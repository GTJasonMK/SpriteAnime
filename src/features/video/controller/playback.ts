import { queryAll } from "../../../utils/dom";
import { getErrorMessage } from "../../../utils/errors";
import {
  parseClampedInt as clampInt,
  formatSeconds
} from "../../../utils/number";

import type { PlaybackMode, VideoSpritePage } from "../video-page";
import { clampNumber } from "../../../utils/number";

export const videoSpritePlaybackMethods = {
  async togglePlayback(): Promise<void> {
    const mode: PlaybackMode = this.viewMode === "source" ? "source" : "output";
    if (this.isPlaying) {
      this.stopPlayback();
      return;
    }
    if (mode === "source" && this.sourceVideoReady) {
      await this.startSourceVideoPlayback();
      return;
    }
    if (mode === "source") {
      await this.ensureSourcePreviewReadyForPlayback();
      if (!this.hasPreparedSourcePreview()) return;
      if (this.getSourcePreviewPlaybackIndices().length === 0) {
        this.setStatus("所选起止范围内没有可播放的源预览帧");
        return;
      }
    }
    if (mode === "output" && this.processedFrames.length === 0) return;
    this.startPlayback(mode);
  },

  async ensureSourcePreviewReadyForPlayback(): Promise<void> {
    this.clearSourcePlaybackFramesIfStale();
    if (!this.hasPreparedSourcePreview()) {
      await this.prepareSourcePreviewFrames();
    }
  },

  startPlayback(mode: PlaybackMode): void {
    if (mode === "source" && !this.hasPreparedSourcePreview()) return;
    if (mode === "output" && this.processedFrames.length === 0) return;
    if (mode === "output") {
      const indices = this.getOutputPlaybackIndices();
      if (indices.length === 0) {
        this.setStatus("所选起止范围内没有可播放的抽取帧");
        return;
      }
      if (!indices.includes(this.currentFrameIndex)) {
        this.currentFrameIndex = indices[0];
      }
      this.setViewMode("playback", false);
      this.syncTimelineToCurrentOutputFrame();
    } else {
      const indices = this.getSourcePreviewPlaybackIndices();
      if (indices.length === 0) {
        this.setStatus("所选起止范围内没有可播放的源预览帧");
        return;
      }
      if (!indices.includes(this.sourcePreviewIndex)) {
        this.sourcePreviewIndex = indices[0];
        this.currentTimeSeconds = this.sourcePreviewFrames[this.sourcePreviewIndex].time;
        this.syncTimeControls();
      }
      this.setViewMode("source", false);
    }
    this.playbackMode = mode;
    this.isPlaying = true;
    this.lastPlaybackAt = performance.now();
    this.els.play.textContent = "暂停";
    this.syncControls();
    this.playbackHandle = window.requestAnimationFrame((time) => this.playbackTick(time));
  },

  stopPlayback(): void {
    this.isPlaying = false;
    this.playbackMode = null;
    this.els.play.textContent = "播放";
    if (this.sourceVideoReady) {
      this.els.video.pause();
    }
    if (this.playbackHandle !== null) {
      window.cancelAnimationFrame(this.playbackHandle);
      this.playbackHandle = null;
    }
    this.syncControls();
  },

  async startSourceVideoPlayback(): Promise<void> {
    if (!this.sourceVideoReady) return;
    this.setViewMode("source", false);
    let start: number;
    let end: number;
    try {
      ({ start, end } = this.readValidatedTimeRangeInputs());
    } catch (err) {
      await this.handleInputValidationError("源视频播放范围无效", err);
      return;
    }
    const current = this.getSourceCurrentTime();
    if (current < start || current >= end) {
      this.seekSourceVideo(start);
    }
    this.playbackMode = "source";
    this.isPlaying = true;
    this.els.play.textContent = "暂停";
    this.syncSourceVideoPlaybackRate();
    this.syncControls();
    try {
      await this.els.video.play();
    } catch (err) {
      const message = getErrorMessage(err);
      this.stopPlayback();
      this.setStatus(`源视频播放失败: ${message}`);
      await this.log(`direct video play failed | ${message}`);
    }
  },

  handleSourceVideoTimeUpdate(): void {
    if (!this.sourceVideoReady) return;
    const { start, end } = this.getSelectedTimeRange();
    const current = this.els.video.currentTime;
    if (this.isPlaying && this.playbackMode === "source" && end > start && current >= end) {
      this.restartSourceVideoLoop(start);
      return;
    }
    this.currentTimeSeconds = clampNumber(current, 0, Math.max(this.getVideoDuration(), 0));
    this.syncTimeControls();
    this.syncPlaybackLabels();
  },

  handleSourceVideoEnded(): void {
    if (!this.sourceVideoReady || this.playbackMode !== "source") return;
    const { start } = this.getSelectedTimeRange();
    this.restartSourceVideoLoop(start);
  },

  restartSourceVideoLoop(start: number): void {
    this.seekSourceVideo(start);
    void this.els.video.play().catch((err) => {
      const message = getErrorMessage(err);
      console.error("[video-sprite] 源视频循环播放失败:", err);
      this.stopPlayback();
      this.setStatus(`源视频循环播放失败: ${message}`);
    });
  },

  seekSourceVideo(time: number): void {
    const clamped = clampNumber(time, 0, Math.max(this.getVideoDuration(), 0));
    this.currentTimeSeconds = clamped;
    if (!this.sourceVideoReady) {
      this.syncTimeControls();
      return;
    }
    this.els.video.currentTime = clamped;
    this.syncTimeControls();
  },

  playbackTick(time: number): void {
    if (!this.isPlaying) return;
    const fps = clampInt(this.els.fps.value, 12, 1, 60);
    const interval = 1000 / fps;
    if (time - this.lastPlaybackAt >= interval) {
      if (this.playbackMode === "source") {
        const nextIndex = this.getNextSourcePreviewIndex(1);
        if (nextIndex === null) {
          this.stopPlayback();
          this.setStatus("所选起止范围内没有可播放的源预览帧");
          return;
        }
        this.sourcePreviewIndex = nextIndex;
        this.currentTimeSeconds = this.sourcePreviewFrames[this.sourcePreviewIndex]?.time ?? this.currentTimeSeconds;
        this.syncTimeControls();
        this.renderSourceView();
      } else {
        const nextIndex = this.getNextOutputFrameIndex(1);
        if (nextIndex === null) {
          this.stopPlayback();
          this.setStatus("所选起止范围内没有可播放的抽取帧");
          return;
        }
        this.currentFrameIndex = nextIndex;
        this.syncTimelineToCurrentOutputFrame();
        this.renderPlaybackFrame();
      }
      this.lastPlaybackAt = time;
    }
    this.playbackHandle = window.requestAnimationFrame((nextTime) => this.playbackTick(nextTime));
  },

  getNextSourcePreviewIndex(delta: number): number | null {
    const indices = this.getSourcePreviewPlaybackIndices();
    if (indices.length === 0) return null;
    return this.getNextPlaybackIndex(indices, this.sourcePreviewIndex, delta);
  },

  getSourcePreviewPlaybackIndices(): number[] {
    if (this.sourcePreviewFrames.length === 0) return [];
    const { start, end } = this.getSelectedTimeRange();
    const epsilon = 0.011;
    const indices: number[] = [];
    for (let i = 0; i < this.sourcePreviewFrames.length; i += 1) {
      const time = this.sourcePreviewFrames[i].time;
      if (time >= start - epsilon && time <= end + epsilon) {
        indices.push(i);
      }
    }
    return indices;
  },

  getOutputPlaybackIndices(): number[] {
    const { start, end } = this.getSelectedTimeRange();
    const epsilon = 0.011;
    const indices: number[] = [];
    for (let index = 0; index < this.processedFrames.length; index += 1) {
      const time = this.processedFrames[index].time;
      if (time >= start - epsilon && time <= end + epsilon) {
        indices.push(index);
      }
    }
    return indices;
  },

  getNextOutputFrameIndex(delta: number): number | null {
    const indices = this.getOutputPlaybackIndices();
    if (indices.length === 0) return null;
    return this.getNextPlaybackIndex(indices, this.currentFrameIndex, delta);
  },

  getNextPlaybackIndex(indices: number[], currentIndex: number, delta: number): number {
    const currentPosition = indices.indexOf(currentIndex);
    if (currentPosition < 0) {
      return delta < 0 ? indices[indices.length - 1] : indices[0];
    }
    return indices[(currentPosition + delta + indices.length) % indices.length];
  },

  stepPlayback(delta: number): void {
    this.stopPlayback();
    if (this.viewMode === "source") {
      if (this.sourceVideoReady) {
        const step = 1 / clampInt(this.els.fps.value, 12, 1, 60);
        this.seekSourceVideo(this.getSourceCurrentTime() + delta * step);
        this.syncTimeControls();
        this.renderSourceView();
        return;
      }
      if (!this.hasPreparedSourcePreview()) return;
      const nextIndex = this.getNextSourcePreviewIndex(delta);
      if (nextIndex === null) {
        this.setStatus("所选起止范围内没有可播放的源预览帧");
        return;
      }
      this.sourcePreviewIndex = nextIndex;
      this.syncTimeControls();
      this.renderSourceView();
      return;
    }
    if (this.processedFrames.length === 0) return;
    const nextIndex = this.getNextOutputFrameIndex(delta);
    if (nextIndex === null) {
      this.setStatus("所选起止范围内没有可播放的抽取帧");
      return;
    }
    this.currentFrameIndex = nextIndex;
    this.syncTimelineToCurrentOutputFrame();
    this.setViewMode("playback", false);
    this.renderPlaybackFrame();
  },

  syncTimelineToCurrentOutputFrame(): void {
    const frame = this.processedFrames[this.currentFrameIndex];
    if (!frame) return;
    this.currentTimeSeconds = frame.time;
    this.syncTimeControls();
  },

  updateFrameCurrentState(): void {
    queryAll<HTMLElement>(".frame-thumb", this.els.frameList).forEach((item, index) => {
      item.classList.toggle("current", index === this.currentFrameIndex);
      item.setAttribute("aria-current", index === this.currentFrameIndex ? "true" : "false");
    });
  },

  syncPlaybackLabels(): void {
    const fps = clampInt(this.els.fps.value, 12, 1, 60);
    this.els.fps.value = String(fps);
    this.els.fpsLabel.textContent = String(fps);
    if (this.viewMode === "source") {
      if (this.sourceVideoReady) {
        this.els.playbackInfo.textContent =
          `源视频: ${formatSeconds(this.getSourceCurrentTime())} / ${formatSeconds(this.getVideoDuration())}`;
      } else {
        const indices = this.getSourcePreviewPlaybackIndices();
        const currentPos = Math.max(0, indices.indexOf(this.sourcePreviewIndex));
        this.els.playbackInfo.textContent =
          `预览: ${indices.length === 0 ? 0 : currentPos + 1}/${indices.length}`;
      }
    } else {
      const indices = this.getOutputPlaybackIndices();
      const currentPosition = indices.indexOf(this.currentFrameIndex);
      this.els.playbackInfo.textContent =
        `范围帧: ${currentPosition < 0 ? 0 : currentPosition + 1}/${indices.length}`;
    }
  },

  syncThresholdLabel(): void {
    this.els.thresholdLabel.textContent = this.els.threshold.value;
  },

  handlePlaybackFpsChanged(): void {
    this.syncPlaybackLabels();
    if (this.sourceVideoReady) {
      this.syncSourceVideoPlaybackRate();
    }
  },

  syncSourceVideoPlaybackRate(): void {
    if (!this.sourceVideoReady) return;
    const fps = clampInt(this.els.fps.value, 12, 1, 60);
    this.els.video.playbackRate = clampNumber(fps / 24, 0.25, 4);
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpritePlaybackMethods = typeof videoSpritePlaybackMethods;
