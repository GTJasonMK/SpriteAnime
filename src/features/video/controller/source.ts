import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import {
  extractVideoFramesWithFfmpeg,
  probeVideoFile,
  type VideoExtractEvent
} from "../../../api/commands";
import { mapWithConcurrency, nextFrame } from "../../../utils/async";
import { getErrorMessage } from "../../../utils/errors";
import {
  formatInputNumber,
  formatSeconds,
  parseInputNumber
} from "../../../utils/number";

import type { SourcePreviewRequest, VideoSpritePage } from "../video-page";
import { clampNumber } from "../../../utils/number";
import { closeSourcePreviewFrames, findNearestSourceFrameIndex, getMediaErrorMessage, loadBitmapFromPath, readRequiredNumberInput, requiredVideoFileName, waitForVideoReady } from "./helpers";

export const videoSpriteSourceMethods = {
  resetSource(): void {
    this.stopPlayback();
    this.clearSourcePreview();
    this.clearGeneratedOutput();
    this.sourcePath = "";
    this.sourceName = "";
    this.videoMeta = null;
    this.cropRegion = null;
    this.savedResult = null;
    this.currentTimeSeconds = 0;
    this.els.file.textContent = "未选择视频";
    this.els.duration.textContent = "时长: -";
    this.els.size.textContent = "尺寸: -";
    this.setStatus("请选择视频来源");
    this.renderSourceView();
    this.syncControls();
  },

  async loadSourceVideo(
    filePath: string,
    fileName: string,
    context: string
  ): Promise<void> {
    const sourceName = requiredVideoFileName(fileName, filePath, context);
    this.stopPlayback();
    this.clearSourceVideo();
    this.sourcePath = filePath;
    this.sourceName = sourceName;
    this.currentTimeSeconds = 0;
    this.videoMeta = null;
    this.savedResult = null;
    this.cropRegion = null;
    this.clearGeneratedOutput();
    this.clearSourcePreview();
    this.setViewMode("source", false);
    this.showBusy(false);
    this.syncControls();

    this.setStatus("正在用 ffmpeg 读取视频...");
    await nextFrame();
    await this.log(`${context} | ffmpeg probe | path=${filePath}`);
    this.videoMeta = await probeVideoFile(filePath);

    const duration = this.getVideoDuration();
    this.els.file.textContent = this.sourceName;
    this.els.duration.textContent = `时长: ${formatSeconds(duration)}`;
    this.els.start.value = "0";
    this.els.end.value = duration > 0 ? formatInputNumber(duration) : "0";
    this.syncTimeControls();
    this.setFullCropRegion(false);
    this.syncControls();

    const previewError = await this.loadSourceVideoForPreview();
    this.renderSourceView();
    if (previewError) {
      this.setStatus(`视频已加载；${previewError}`);
    } else {
      this.setStatus("已加载源视频，可播放并拖拽选择画面区域");
    }
    this.syncControls();
  },

  async loadSourceVideoForPreview(): Promise<string | null> {
    this.clearSourceVideo();
    const directError = await this.tryLoadVideoPath(this.sourcePath, "direct");
    if (!directError) {
      await this.log("direct video playback ok");
      return null;
    }
    await this.log(`direct video playback failed | ${directError}`);
    this.clearSourceVideo();
    return this.buildSourcePlaybackError(directError);
  },

  buildSourcePlaybackError(reason: string): string {
    return `当前 WebView 无法直接播放源视频：${reason}。仍可点击“生成序列帧图”；如需预览，请点击“播放”使用 FFmpeg 帧预览，或将视频转码为 H.264/AAC MP4。`;
  },

  async tryLoadVideoPath(path: string, label: string): Promise<string | null> {
    const url = convertFileSrc(path);
    this.els.video.pause();
    this.els.video.removeAttribute("src");
    this.els.video.load();
    this.els.video.src = url;
    this.els.video.muted = true;
    this.els.video.preload = "metadata";

    try {
      await waitForVideoReady(this.els.video, 4500);
      this.sourceVideoReady = true;
      const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, Math.max(this.getVideoDuration(), 0));
      this.seekSourceVideo(start);
      this.updateSourceVideoVisibility();
      return null;
    } catch (err) {
      const message = getMediaErrorMessage(this.els.video, err);
      this.els.video.pause();
      this.els.video.removeAttribute("src");
      this.els.video.load();
      this.sourceVideoReady = false;
      this.updateSourceVideoVisibility();
      return `${label}: ${message}`;
    }
  },

  async prepareSourcePreviewFrames(): Promise<void> {
    if (!this.sourcePath || !this.videoMeta) return;
    let request: SourcePreviewRequest;
    try {
      request = this.getSourcePreviewRequest();
    } catch (err) {
      await this.handleInputValidationError("源视频预览参数无效", err);
      return;
    }
    if (this.hasPreparedSourcePreview(request)) {
      await this.log(`source preview reuse | signature=${request.signature}`);
      this.sourcePreviewIndex = findNearestSourceFrameIndex(
        this.sourcePreviewFrames,
        this.currentTimeSeconds
      );
      this.syncTimeControls();
      this.renderCurrentView();
      return;
    }
    if (this.sourcePreviewPreparePromise) {
      await this.log(`source preview await existing prepare | signature=${request.signature}`);
      await this.sourcePreviewPreparePromise;
      if (this.hasPreparedSourcePreview(request)) {
        this.sourcePreviewIndex = findNearestSourceFrameIndex(
          this.sourcePreviewFrames,
          this.currentTimeSeconds
        );
        this.syncTimeControls();
        this.renderCurrentView();
      }
      return;
    }
    const preparePromise = this.prepareSourcePreviewFramesForRequest(request);
    this.sourcePreviewPreparePromise = preparePromise;
    try {
      await preparePromise;
    } finally {
      if (this.sourcePreviewPreparePromise === preparePromise) {
        this.sourcePreviewPreparePromise = null;
      }
    }
  },

  async prepareSourcePreviewFramesForRequest(request: SourcePreviewRequest): Promise<void> {
    const seq = ++this.sourcePreviewSeq;
    this.isPreparingSourcePreview = true;
    this.syncControls();
    const { start, end, frameCount, previewFps, maxFrames, cacheKey, signature } = request;
    await this.log(
      `source preview prepare start | signature=${signature} fps=${previewFps} max=${maxFrames}`
    );
    this.setStatus(`正在准备源视频播放预览 ${frameCount} 帧...`);
    this.showBusy(true, "正在准备源视频播放预览...");
    await nextFrame();

    let outputDir = "";
    try {
      const channel = new Channel<VideoExtractEvent>();
      channel.onmessage = (event: VideoExtractEvent) => {
        if (event.event === "ExtractingFrame") {
          const text =
            `正在准备源视频播放预览 ${event.data.index}/${event.data.total}`;
          this.setStatus(text);
          this.showBusy(true, text);
        }
      };
      const result = await extractVideoFramesWithFfmpeg(
        channel,
        this.sourcePath,
        frameCount,
        start,
        end,
        undefined,
        640
      );
      outputDir = result.output_dir;
      let decoded = 0;
      const frames = await mapWithConcurrency(result.frames, 4, async (frame) => {
        const bitmap = await loadBitmapFromPath(frame.path);
        decoded += 1;
        if (seq === this.sourcePreviewSeq) {
          const text = `正在解码源视频播放预览 ${decoded}/${result.frames.length}`;
          this.setStatus(text);
          this.showBusy(true, text);
        }
        return {
          bitmap,
          time: frame.time_seconds,
        };
      });
      if (seq !== this.sourcePreviewSeq) {
        closeSourcePreviewFrames(frames);
        return;
      }
      closeSourcePreviewFrames(this.sourcePreviewFrames);
      this.sourcePreviewFrames = frames;
      this.sourcePreviewSignature = signature;
      this.sourcePreviewCacheKey = cacheKey;
      this.sourcePreviewCacheStart = start;
      this.sourcePreviewCacheEnd = end;
      const selectedRange = this.getSelectedTimeRange();
      const usesDefaultFullRange =
        selectedRange.start <= 0.011 &&
        Math.abs(selectedRange.end - this.getVideoDuration()) <= 0.011;
      const firstFrame = frames[0];
      const lastFrame = frames[frames.length - 1];
      if (usesDefaultFullRange && firstFrame && lastFrame && lastFrame.time > firstFrame.time) {
        this.setTimeRangeInputs(firstFrame.time, lastFrame.time);
      }
      this.sourcePreviewIndex = findNearestSourceFrameIndex(frames, this.currentTimeSeconds);
      this.syncTimeControls();
      this.renderCurrentView();
      this.setStatus("源视频播放预览已就绪");
      await this.log(`source preview prepare ok | signature=${signature} frames=${frames.length}`);
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[video-sprite] 源视频播放预览准备失败:", err);
      await this.log(`source preview frames failed | ${message}`);
      this.setStatus(`源视频播放预览准备失败: ${message}`);
    } finally {
      await this.cleanupVideoFrameOutputDir(outputDir, "source playback preview");
      if (seq === this.sourcePreviewSeq) {
        this.isPreparingSourcePreview = false;
        this.showBusy(false);
        this.syncControls();
      }
    }
  },

  showSourceFrameAtTime(time: number): void {
    const duration = this.getVideoDuration();
    const clampedTime = clampNumber(time, 0, Math.max(duration, 0));
    this.currentTimeSeconds = clampedTime;
    if (this.sourceVideoReady) {
      this.seekSourceVideo(clampedTime);
      this.syncTimeControls();
      this.renderCurrentView();
      return;
    }
    if (this.hasPreparedSourcePreview()) {
      this.sourcePreviewIndex = findNearestSourceFrameIndex(this.sourcePreviewFrames, clampedTime);
      this.syncTimeControls();
      this.renderCurrentView();
      return;
    }
    this.els.timeScrub.value = String(clampedTime);
    this.syncCurrentTimeLabel(clampedTime);
    this.renderCurrentView();
  },

  async handleStartTimeChanged(refreshPreview: boolean): Promise<void> {
    try {
      const { start, end } = this.readValidatedTimeRangeInputs();
      this.setTimeRangeInputs(start, end);
      this.syncTimeRangeInputChange();
      if (refreshPreview) {
        this.showSourceFrameAtTime(start);
      }
    } catch (err) {
      await this.handleInputValidationError("起始时间无效", err);
    }
  },

  handleEndTimeChanged(): void {
    try {
      const { start, end } = this.readValidatedTimeRangeInputs();
      this.setTimeRangeInputs(start, end);
      this.syncTimeRangeInputChange();
    } catch (err) {
      void this.handleInputValidationError("结束时间无效", err);
    }
  },

  handleSourceScrubInput(): void {
    const time = parseInputNumber(this.els.timeScrub.value, 0);
    this.showSourceFrameAtTime(time);
  },

  handleStartRangeInput(): void {
    const duration = this.getVideoDuration();
    const start = clampNumber(parseInputNumber(this.els.startRange.value, 0), 0, Math.max(duration, 0));
    const end = Math.max(start, parseInputNumber(this.els.end.value, duration));
    this.setTimeRangeInputs(start, end);
    this.syncTimeRangeInputChange();
    this.showSourceFrameAtTime(start);
  },

  handleEndRangeInput(): void {
    const duration = this.getVideoDuration();
    const start = parseInputNumber(this.els.start.value, 0);
    const end = clampNumber(parseInputNumber(this.els.endRange.value, duration), start, Math.max(duration, start));
    this.els.end.value = formatInputNumber(end);
    this.syncTimeRangeInputChange();
    this.showSourceFrameAtTime(end);
  },

  readTimeRangeInputs(): { start: number; end: number; max: number; } {
    const max = Math.max(this.getVideoDuration(), 0);
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, max);
    const end = clampNumber(parseInputNumber(this.els.end.value, max), start, max);
    return { start, end, max };
  },

  readValidatedTimeRangeInputs(): { start: number; end: number; max: number; } {
    const max = Math.max(this.getVideoDuration(), 0);
    if (max <= 0) {
      throw new Error("视频时长无效。解决方法：请重新导入带有有效 duration 元数据的视频，或先用 ffmpeg -i input -c copy output.mp4 重新封装。");
    }
    const start = readRequiredNumberInput(this.els.start, "起始时间", 0, max);
    const end = readRequiredNumberInput(this.els.end, "结束时间", 0, max);
    if (end <= start) {
      throw new Error("结束时间必须大于起始时间。解决方法：请调整起始/结束时间，选择一个非零长度的视频片段。");
    }
    return { start, end, max };
  },

  setTimeRangeInputs(start: number, end: number): void {
    this.els.start.value = formatInputNumber(start);
    this.els.end.value = formatInputNumber(end);
  },

  syncTimeRangeInputChange(): void {
    this.clearSourcePlaybackFramesIfStale();
    this.syncTimeControls();
  },

  async setCurrentTimeAsRangePoint(point: "start" | "end"): Promise<void> {
    const time = this.getSourceCurrentTime();
    if (point === "start") {
      this.els.start.value = formatInputNumber(Math.min(time, parseInputNumber(this.els.end.value, time)));
      await this.handleStartTimeChanged(false);
    } else {
      this.els.end.value = formatInputNumber(Math.max(time, parseInputNumber(this.els.start.value, 0)));
      this.handleEndTimeChanged();
    }
  },

  async handleInputValidationError(context: string, err: unknown): Promise<void> {
    const message = getErrorMessage(err);
    await this.log(`${context} | ${message}`);
    this.setStatus(`${context}: ${message}`);
  },

  handleBackendExtractProgress(event: VideoExtractEvent): void {
    switch (event.event) {
      case "Probing":
        this.setStatus("正在读取视频元数据...");
        this.showBusy(true, "正在读取视频元数据...");
        break;
      case "ExtractingFrame": {
        const text =
          `ffmpeg 抽取第 ${event.data.index}/${event.data.total} 帧 @ ${event.data.time_seconds.toFixed(2)}s`;
        this.setStatus(text);
        this.showBusy(true, text);
        break;
      }
      case "Completed": {
        const text = `ffmpeg 已抽取 ${event.data.frames} 帧`;
        this.setStatus(text);
        this.showBusy(true, text);
        break;
      }
    }
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteSourceMethods = typeof videoSpriteSourceMethods;
