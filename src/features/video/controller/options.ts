import {
  parseClampedInt as clampInt
} from "../../../utils/number";
import type {
  PixelBounds
} from "../types";
import {
  clampPixelBounds,
  parseBackgroundMode
} from "../utils";

import type { CropDragState, ExtractionOptions, SourcePreviewRequest, VideoSpritePage } from "../video-page";
import { clampNumber } from "../../../utils/number";
import { getSourcePreviewFrameCount, readRequiredIntegerInput, regionFromPoints } from "./helpers";

export const videoSpriteOptionsMethods = {
  readOptions(): ExtractionOptions {
    const frameCount = readRequiredIntegerInput(this.els.frameCount, "抽帧数量", 2, 64);
    const cols = readRequiredIntegerInput(this.els.cols, "每行列数", 1, 12);
    const maxFrameEdge = readRequiredIntegerInput(this.els.frameEdge, "单帧最大边长", 64, 768);
    const padding = readRequiredIntegerInput(this.els.padding, "透明裁边距", 0, 96);
    const threshold = readRequiredIntegerInput(this.els.threshold, "背景阈值", 1, 160);
    const { start, end } = this.readValidatedTimeRangeInputs();
    const bgMode = parseBackgroundMode(this.els.bgMode.value);
    const cropRegion = this.getCropRegionOrFull();

    this.els.frameCount.value = String(frameCount);
    this.els.cols.value = String(cols);
    this.els.frameEdge.value = String(maxFrameEdge);
    this.els.padding.value = String(padding);
    this.els.threshold.value = String(threshold);
    this.syncThresholdLabel();

    return {
      frameCount,
      cols,
      start,
      end,
      maxFrameEdge,
      padding,
      threshold,
      bgMode,
      autoTrim: this.els.autoTrim.checked,
      transparent: this.els.transparent.checked,
      cropRegion,
    };
  },

  getSelectedTimeRange(): { start: number; end: number; } {
    const { start, end } = this.readTimeRangeInputs();
    return { start, end };
  },

  getSourcePreviewRequest(): SourcePreviewRequest {
    const { start, end } = this.readValidatedTimeRangeInputs();
    const { previewFps, maxFrames } = this.readSourcePreviewSettings();
    return this.buildSourcePreviewRequest(start, end, previewFps, maxFrames);
  },

  getCurrentSourcePreviewRequest(): SourcePreviewRequest {
    const { start, end } = this.getSelectedTimeRange();
    const { previewFps, maxFrames } = this.readSafeSourcePreviewSettings();
    return this.buildSourcePreviewRequest(start, end, previewFps, maxFrames);
  },

  buildSourcePreviewRequest(
    start: number,
    end: number,
    previewFps: number,
    maxFrames: number
  ): SourcePreviewRequest {
    const frameCount = getSourcePreviewFrameCount(Math.max(0, end - start), previewFps, maxFrames);
    const cacheKey = [
      this.sourcePath,
      previewFps,
      maxFrames,
    ].join("|");
    const signature = [
      cacheKey,
      start.toFixed(3),
      end.toFixed(3),
      frameCount,
    ].join("|");
    return { start, end, frameCount, previewFps, maxFrames, cacheKey, signature };
  },

  readSafeSourcePreviewSettings(): { previewFps: number; maxFrames: number; } {
    return {
      previewFps: clampInt(this.els.sourcePreviewFps.value, 6, 1, 30),
      maxFrames: clampInt(this.els.sourcePreviewMax.value, 72, 2, 240),
    };
  },

  readSourcePreviewSettings(): { previewFps: number; maxFrames: number; } {
    const previewFps = readRequiredIntegerInput(this.els.sourcePreviewFps, "预览 FPS", 1, 30);
    const maxFrames = readRequiredIntegerInput(this.els.sourcePreviewMax, "预览上限", 2, 240);
    this.els.sourcePreviewFps.value = String(previewFps);
    this.els.sourcePreviewMax.value = String(maxFrames);
    return { previewFps, maxFrames };
  },

  handleSourcePreviewSettingChanged(): void {
    try {
      this.readSourcePreviewSettings();
      this.syncControls();
    } catch (err) {
      void this.handleInputValidationError("源视频预览参数无效", err);
    }
  },

  hasPreparedSourcePreview(request?: SourcePreviewRequest): boolean {
    if (this.sourcePreviewFrames.length === 0) return false;
    const currentRequest = request || this.getCurrentSourcePreviewRequest();
    if (this.sourcePreviewSignature === currentRequest.signature) return true;
    if (this.sourcePreviewCacheKey !== currentRequest.cacheKey) return false;
    const epsilon = 0.05;
    return currentRequest.start >= this.sourcePreviewCacheStart - epsilon &&
      currentRequest.end <= this.sourcePreviewCacheEnd + epsilon;
  },

  handleCropInput(): void {
    const full = this.getFullRegion();
    if (!full) return;
    const width = clampInt(this.els.cropW.value, full.width, 1, full.width);
    const height = clampInt(this.els.cropH.value, full.height, 1, full.height);
    const x = clampInt(this.els.cropX.value, 0, 0, Math.max(0, full.width - width));
    const y = clampInt(this.els.cropY.value, 0, 0, Math.max(0, full.height - height));
    this.cropRegion = { x, y, width, height };
    this.syncCropInputs();
    this.renderCurrentView();
  },

  setFullCropRegion(render: boolean = true): void {
    const full = this.getFullRegion();
    if (!full) return;
    this.cropRegion = full;
    this.syncCropInputs();
    if (render) {
      this.renderCurrentView();
    }
  },

  getCropRegionOrFull(): PixelBounds {
    const full = this.getFullRegion();
    if (!full || !this.cropRegion) {
      throw new Error("视频裁切区域尚未初始化");
    }
    return clampPixelBounds(this.cropRegion, full.width, full.height);
  },

  getFullRegion(): PixelBounds | null {
    const width = this.getVideoWidth();
    const height = this.getVideoHeight();
    if (width <= 0 || height <= 0) {
      return null;
    }
    return { x: 0, y: 0, width, height };
  },

  syncCropInputs(): void {
    const region = this.cropRegion;
    if (!region) return;
    this.els.cropX.value = String(Math.round(region.x));
    this.els.cropY.value = String(Math.round(region.y));
    this.els.cropW.value = String(Math.round(region.width));
    this.els.cropH.value = String(Math.round(region.height));
  },

  handleCropPointerDown(event: PointerEvent): void {
    if (this.isBusy() || this.viewMode !== "source") return;
    const point = this.getCanvasPoint(event);
    if (!point) return;
    this.cropDrag = point;
    this.els.canvas.setPointerCapture(event.pointerId);
    this.cropRegion = {
      x: point.startX,
      y: point.startY,
      width: 1,
      height: 1,
    };
    this.renderCurrentView();
  },

  handleCropPointerMove(event: PointerEvent): void {
    if (!this.cropDrag || this.viewMode !== "source") return;
    const point = this.getCanvasPoint(event);
    if (!point) return;
    this.cropRegion = regionFromPoints(this.cropDrag, point, this.getVideoWidth(), this.getVideoHeight());
    this.syncCropInputs();
    this.renderCurrentView();
  },

  handleCropPointerUp(event: PointerEvent): void {
    if (!this.cropDrag) return;
    const drag = this.cropDrag;
    const point = this.viewMode === "source" ? this.getCanvasPoint(event) : null;
    if (point) {
      this.cropRegion = regionFromPoints(drag, point, this.getVideoWidth(), this.getVideoHeight());
    }
    if (this.els.canvas.hasPointerCapture(event.pointerId)) {
      this.els.canvas.releasePointerCapture(event.pointerId);
    }
    this.cropDrag = null;
    this.syncCropInputs();
    this.renderCurrentView();
  },

  getCanvasPoint(event: PointerEvent): CropDragState | null {
    const width = this.getVideoWidth();
    const height = this.getVideoHeight();
    if (width <= 0 || height <= 0) return null;
    const rect = this.els.canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return null;
    return {
      startX: clampNumber(((event.clientX - rect.left) / rect.width) * width, 0, width),
      startY: clampNumber(((event.clientY - rect.top) / rect.height) * height, 0, height),
    };
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteOptionsMethods = typeof videoSpriteOptionsMethods;
