import {
  cleanupVideoFrameBatchDir,
  cleanupVideoSpriteTempFiles,
  logVideoSpriteMessage
} from "../../../api/commands";
import {
  clearPreviewCanvas
} from "../../../utils/canvas";
import { getErrorMessage } from "../../../utils/errors";
import {
  formatSeconds
} from "../../../utils/number";
import { setBusyState, setButtonState } from "../../../utils/ui";

import type { VideoSpritePage } from "../video-page";
import { clampNumber } from "../../../utils/number";
import { closeSourcePreviewFrames } from "./helpers";
import { notifyTaskStateChanged } from "../../../workflows/events";

export const videoSpriteControlsMethods = {
  syncControls(): void {
    const hasVideo = Boolean(this.sourcePath && this.getVideoWidth() > 0 && this.getVideoHeight() > 0);
    const hasOutput = this.processedFrames.length > 0 && Boolean(this.spriteSheetBlob);
    const hasSourcePlayback = this.hasPreparedSourcePreview();
    const hasSourceStep = this.sourceVideoReady || hasSourcePlayback;
    const controlsSourcePlayback = this.viewMode === "source";
    const busy = this.isBusy();
    const canStartSourcePlayback = hasVideo && !busy && !this.isPreparingSourcePreview;
    const hasActivePlayback = controlsSourcePlayback ? canStartSourcePlayback : hasOutput;
    setButtonState(this.els.reference, { disabled: busy || !hasOutput });
    this.els.viewSheet.disabled = !hasOutput;
    this.els.viewPlayback.disabled = !hasOutput;
    this.els.prev.disabled = controlsSourcePlayback ? !hasSourceStep : !hasOutput;
    setButtonState(this.els.play, {
      disabled: !hasActivePlayback,
      text: this.isPlaying ? "暂停" : "播放",
    });
    this.els.next.disabled = controlsSourcePlayback ? !hasSourceStep : !hasOutput;
    this.els.setStart.disabled = busy || !hasVideo;
    this.els.setEnd.disabled = busy || !hasVideo;
    this.els.timeScrub.disabled = busy || !hasVideo;
    this.els.startRange.disabled = busy || !hasVideo;
    this.els.endRange.disabled = busy || !hasVideo;
    this.els.sourcePreviewFps.disabled = busy || !hasVideo || this.sourceVideoReady;
    this.els.sourcePreviewMax.disabled = busy || !hasVideo || this.sourceVideoReady;
    [this.els.cropX, this.els.cropY, this.els.cropW, this.els.cropH].forEach((input) => {
      input.disabled = busy || !hasVideo;
    });
    [
      this.els.videoPrompt,
      this.els.videoSize,
      this.els.videoSeconds,
      this.els.videoSourceId,
      this.els.videoDirection,
    ].forEach((input) => {
      input.disabled = busy;
    });
    this.els.pickVideoReference.disabled = busy;
    this.els.clearVideoReference.disabled = busy || !this.videoReferencePath;
    setButtonState(this.els.redrawPause, {
      disabled: !this.isRedrawing || this.redrawPauseRequested,
      text: this.redrawPauseRequested ? "等待暂停" : "暂停",
    });
    setButtonState(this.els.redrawDiscard, {
      disabled: busy || !this.redrawRun,
    });
    this.els.redrawFinalCols.disabled = this.isRedrawing;
    [
      this.els.redrawGroupRows,
      this.els.redrawGroupCols,
      this.els.redrawResolution,
      this.els.redrawStyle,
      this.els.redrawPrompt,
      this.els.redrawNegativePrompt,
    ].forEach((input) => {
      input.disabled = busy || Boolean(this.redrawRun);
    });
    this.els.redrawApiProfile.textContent = `API: ${this.apiSettings.getActiveProfileName()}`;
    this.syncGenerationConstraintControls();
    this.syncPlaybackLabels();
    notifyTaskStateChanged();
  },

  isBusy(): boolean {
    return this.isExtracting || this.isGeneratingVideo || this.isRedrawing;
  },

  showBusy(show: boolean, text: string = "正在生成..."): void {
    setBusyState(this.els.busy, this.els.busyText, show, text);
  },

  async cleanupStartupTempFiles(): Promise<void> {
    try {
      const result = await cleanupVideoSpriteTempFiles();
      if (result.removed_dirs > 0) {
        await this.log(
          `temp cleanup startup ok | dirs=${result.removed_dirs}`
        );
      }
    } catch (err) {
      await this.log(`temp cleanup startup failed | ${getErrorMessage(err)}`);
    }
  },

  async cleanupVideoFrameOutputDir(outputDir: string, context: string): Promise<void> {
    if (!outputDir) return;
    try {
      const result = await cleanupVideoFrameBatchDir(outputDir);
      await this.log(
        `temp cleanup video frames ok | context=${context} dir=${outputDir} dirs=${result.removed_dirs}`
      );
    } catch (err) {
      await this.log(
        `temp cleanup video frames failed | context=${context} dir=${outputDir} error=${getErrorMessage(err)}`
      );
    }
  },

  clearGeneratedOutput(): void {
    this.stopPlayback();
    this.processedFrames.forEach((frame) => {
      URL.revokeObjectURL(frame.url);
      frame.bitmap?.close();
    });
    this.processedFrames = [];
    this.processedFramesOrigin = "none";
    if (this.spriteSheetBitmap) {
      this.spriteSheetBitmap.close();
    }
    this.spriteSheetBitmap = null;
    this.spriteSheetBlob = null;
    this.currentFrameIndex = 0;
    this.els.frameTotal.textContent = "0 帧";
    this.els.frameList.innerHTML = '<div class="placeholder-text">选择视频后生成序列帧图</div>';
    this.syncPlaybackLabels();
  },

  clearSourcePreview(): void {
    this.sourcePreviewSeq += 1;
    this.isPreparingSourcePreview = false;
    this.sourcePreviewPreparePromise = null;
    this.clearSourceVideo();
    closeSourcePreviewFrames(this.sourcePreviewFrames);
    this.sourcePreviewFrames = [];
    this.sourcePreviewSignature = "";
    this.sourcePreviewCacheKey = "";
    this.sourcePreviewCacheStart = 0;
    this.sourcePreviewCacheEnd = 0;
    this.sourcePreviewIndex = 0;
    this.syncTimeControls();
  },

  clearSourcePlaybackFrames(): void {
    if (this.sourcePreviewFrames.length === 0 && !this.isPreparingSourcePreview && !this.sourcePreviewSignature) return;
    this.sourcePreviewSeq += 1;
    this.isPreparingSourcePreview = false;
    this.sourcePreviewPreparePromise = null;
    closeSourcePreviewFrames(this.sourcePreviewFrames);
    this.sourcePreviewFrames = [];
    this.sourcePreviewSignature = "";
    this.sourcePreviewCacheKey = "";
    this.sourcePreviewCacheStart = 0;
    this.sourcePreviewCacheEnd = 0;
    this.sourcePreviewIndex = 0;
    this.showBusy(false);
    this.stopPlayback();
    this.syncTimeControls();
    this.syncControls();
  },

  clearSourcePlaybackFramesIfStale(): boolean {
    if (!this.sourcePreviewSignature && !this.isPreparingSourcePreview) return false;
    const currentRequest = this.getCurrentSourcePreviewRequest();
    if (this.hasPreparedSourcePreview(currentRequest)) return false;
    this.clearSourcePlaybackFrames();
    return true;
  },

  clearSourceVideo(): void {
    this.sourceVideoReady = false;
    this.els.video.pause();
    this.els.video.removeAttribute("src");
    this.els.video.load();
    this.updateSourceVideoVisibility(false);
  },

  getSourceDisplayBitmap(): ImageBitmap | null {
    return this.sourcePreviewFrames[this.sourcePreviewIndex]?.bitmap || null;
  },

  getSourceCurrentTime(): number {
    return this.currentTimeSeconds;
  },

  syncTimeControls(): void {
    const { start, end, max } = this.readTimeRangeInputs();
    const current = clampNumber(this.currentTimeSeconds, 0, max);

    [this.els.timeScrub, this.els.startRange, this.els.endRange].forEach((range) => {
      range.min = "0";
      range.max = String(max);
      range.step = "0.01";
    });
    this.els.startRange.value = String(start);
    this.els.endRange.value = String(end);
    this.els.timeScrub.value = String(current);
    this.syncCurrentTimeLabel(current);
  },

  syncCurrentTimeLabel(time: number): void {
    this.els.currentTime.textContent =
      `当前: ${formatSeconds(time)} / ${formatSeconds(this.getVideoDuration())}`;
  },

  hasSourcePreview(): boolean {
    return Boolean(this.sourceVideoReady || this.sourcePreviewFrames.length > 0);
  },

  updateSourceVideoVisibility(forceVisible?: boolean): void {
    const visible = forceVisible ?? (this.viewMode === "source" && this.sourceVideoReady);
    const previewArea = this.els.canvas.closest(".preview-area");
    previewArea?.classList.toggle("uses-video", visible);
    this.els.video.style.display = visible ? "block" : "none";
  },

  clearCanvas(message: string): void {
    this.updateSourceVideoVisibility(false);
    clearPreviewCanvas(this.previewCanvasTarget(), message);
  },

  previewCanvasTarget() {
    return {
      canvas: this.els.canvas,
      placeholder: this.els.placeholder,
      sizeLabel: this.els.size,
    };
  },

  setStatus(text: string): void {
    this.els.status.textContent = text;
  },

  async log(message: string): Promise<void> {
    await logVideoSpriteMessage(message);
  },

  getVideoDuration(): number {
    return this.videoMeta?.duration_seconds || 0;
  },

  getVideoWidth(): number {
    return this.videoMeta?.width || 0;
  },

  getVideoHeight(): number {
    return this.videoMeta?.height || 0;
  },

} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteControlsMethods = typeof videoSpriteControlsMethods;
