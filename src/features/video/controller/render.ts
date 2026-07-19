import {
  fitPreviewCanvasSize,
  preparePreviewCanvas
} from "../../../utils/canvas";
import { getErrorMessage } from "../../../utils/errors";

import type { VideoSpritePage, VideoSpriteView } from "../video-page";

export const videoSpriteRenderMethods = {
  setViewMode(mode: VideoSpriteView, render: boolean = true): void {
    if (mode !== "source" && this.processedFrames.length === 0) {
      return;
    }
    if (
      this.isPlaying &&
      ((this.playbackMode === "source" && mode !== "source") ||
        (this.playbackMode === "output" && mode === "source"))
    ) {
      this.stopPlayback();
    }
    this.viewMode = mode;
    this.els.viewSource.classList.toggle("active", mode === "source");
    this.els.viewSheet.classList.toggle("active", mode === "sheet");
    this.els.viewPlayback.classList.toggle("active", mode === "playback");
    if (render) {
      this.renderCurrentView();
    }
    this.syncControls();
  },

  renderCurrentView(): void {
    this.els.canvas.closest(".preview-area")?.classList.toggle(
      "has-image",
      this.viewMode === "source" ? this.hasSourcePreview() : this.processedFrames.length > 0
    );
    this.updateSourceVideoVisibility();
    if (this.viewMode === "sheet") {
      this.renderSheetView();
    } else if (this.viewMode === "playback") {
      this.renderPlaybackFrame();
    } else {
      this.renderSourceView();
    }
  },

  renderSourceView(): void {
    const width = this.getVideoWidth();
    const height = this.getVideoHeight();
    if (!this.hasSourcePreview() || width <= 0 || height <= 0) {
      this.clearCanvas("选择视频后在这里拖拽选择画面区域");
      return;
    }
    const previewSize = fitPreviewCanvasSize(width, height, 1280);
    const ctx = preparePreviewCanvas({
      target: this.previewCanvasTarget(),
      canvasWidth: previewSize.width,
      canvasHeight: previewSize.height,
      aspectWidth: width,
      aspectHeight: height,
      sizeText: `源尺寸: ${width} x ${height}`,
    });
    if (!ctx) return;
    const canvas = this.els.canvas;
    canvas.style.aspectRatio = `${width} / ${height}`;
    this.els.video.style.aspectRatio = `${width} / ${height}`;
    ctx.save();
    ctx.scale(canvas.width / width, canvas.height / height);
    const sourceBitmap = this.sourceVideoReady ? null : this.getSourceDisplayBitmap();
    if (sourceBitmap) {
      ctx.drawImage(sourceBitmap, 0, 0, width, height);
    }
    this.drawCropOverlay(ctx, width, height);
    ctx.restore();
  },

  drawCropOverlay(ctx: CanvasRenderingContext2D, width: number, height: number): void {
    const region = this.getCropRegionOrFull();
    ctx.save();
    ctx.fillStyle = "rgba(0, 0, 0, 0.42)";
    ctx.fillRect(0, 0, width, region.y);
    ctx.fillRect(0, region.y + region.height, width, height - region.y - region.height);
    ctx.fillRect(0, region.y, region.x, region.height);
    ctx.fillRect(region.x + region.width, region.y, width - region.x - region.width, region.height);
    ctx.strokeStyle = "rgba(244, 203, 124, 0.96)";
    ctx.lineWidth = Math.max(2, Math.round(Math.max(width, height) / 700));
    ctx.strokeRect(region.x + 0.5, region.y + 0.5, region.width - 1, region.height - 1);
    ctx.fillStyle = "rgba(244, 203, 124, 0.95)";
    ctx.font = `${Math.max(14, Math.round(height / 56))}px sans-serif`;
    ctx.fillText(
      `${Math.round(region.x)}, ${Math.round(region.y)} · ${Math.round(region.width)}x${Math.round(region.height)}`,
      region.x + 8,
      Math.max(24, region.y + 24)
    );
    ctx.restore();
  },

  renderSheetView(): void {
    this.updateSourceVideoVisibility(false);
    if (!this.spriteSheetBitmap) {
      this.clearCanvas("生成后显示序列帧图");
      return;
    }
    const ctx = preparePreviewCanvas({
      target: this.previewCanvasTarget(),
      canvasWidth: this.spriteSheetBitmap.width,
      canvasHeight: this.spriteSheetBitmap.height,
      sizeText: `尺寸: ${this.spriteSheetBitmap.width} x ${this.spriteSheetBitmap.height}`,
    });
    if (!ctx) return;
    ctx.drawImage(this.spriteSheetBitmap, 0, 0);
  },

  renderPlaybackFrame(): void {
    this.updateSourceVideoVisibility(false);
    if (this.processedFrames.length === 0) {
      this.clearCanvas("生成后播放预览");
      return;
    }
    const frame = this.processedFrames[this.currentFrameIndex % this.processedFrames.length];
    const seq = ++this.playbackRenderSeq;
    const ctx = preparePreviewCanvas({
      target: this.previewCanvasTarget(),
      canvasWidth: frame.width,
      canvasHeight: frame.height,
      sizeText: `帧尺寸: ${frame.width} x ${frame.height}`,
    });
    if (!ctx) return;
    const canvas = this.els.canvas;
    this.syncPlaybackLabels();
    this.updateFrameCurrentState();
    if (frame.bitmap) {
      ctx.drawImage(frame.bitmap, 0, 0);
      return;
    }
    void createImageBitmap(frame.blob)
      .then((bitmap) => {
        if (seq !== this.playbackRenderSeq) {
          bitmap.close();
          return;
        }
        frame.bitmap = bitmap;
        ctx.clearRect(0, 0, canvas.width, canvas.height);
        ctx.drawImage(bitmap, 0, 0);
      })
      .catch((err) => {
        const message = getErrorMessage(err);
        console.error("[video-sprite] 播放帧解码失败:", err);
        this.setStatus(`播放帧解码失败: ${message}`);
      });
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteRenderMethods = typeof videoSpriteRenderMethods;
