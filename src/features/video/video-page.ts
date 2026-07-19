import {
  type RedrawRunManifest,
  type SavedImageResult,
  type VideoProbeResult
} from "../../api/commands";
import { getById } from "../../utils/dom";
import {
  usesBackgroundDescription,
  type GenerationBackgroundMode,
  type GenerationFraming,
  type ImageGenerationConstraints,
  type VideoGenerationConstraints,
} from "../../generation/constraints";
import {
  parseInputNumber
} from "../../utils/number";
import {
  type RedrawPlan
} from "./redraw";
import type {
  BackgroundMode,
  PixelBounds,
  VideoSpriteApiSettings,
  VideoSpriteWorkerFrameInput as WorkerFrameInput
} from "./types";

import { videoSpriteControlsMethods, type VideoSpriteControlsMethods } from "./controller/controls";
import { videoSpriteExtractionMethods, type VideoSpriteExtractionMethods } from "./controller/extraction";
import { videoSpriteGenerationMethods, type VideoSpriteGenerationMethods } from "./controller/generation";
import { videoSpriteOptionsMethods, type VideoSpriteOptionsMethods } from "./controller/options";
import { videoSpritePlaybackMethods, type VideoSpritePlaybackMethods } from "./controller/playback";
import { videoSpriteRedrawRunMethods, type VideoSpriteRedrawRunMethods } from "./controller/redraw-run";
import { videoSpriteRedrawStartMethods, type VideoSpriteRedrawStartMethods } from "./controller/redraw-start";
import { videoSpriteRenderMethods, type VideoSpriteRenderMethods } from "./controller/render";
import { videoSpriteSourceMethods, type VideoSpriteSourceMethods } from "./controller/source";
import { videoSpriteWorkspaceMethods, type VideoSpriteWorkspaceMethods } from "./workspace";
export type VideoSpriteView = "source" | "sheet" | "playback";
export type PlaybackMode = "source" | "output";

export interface ExtractionOptions {
  frameCount: number;
  cols: number;
  start: number;
  end: number;
  maxFrameEdge: number;
  padding: number;
  threshold: number;
  bgMode: BackgroundMode;
  autoTrim: boolean;
  transparent: boolean;
  cropRegion: PixelBounds;
}

export interface ProcessedFrame {
  blob: Blob;
  url: string;
  bitmap: ImageBitmap | null;
  time: number;
  width: number;
  height: number;
}

export interface SourcePreviewFrame {
  bitmap: ImageBitmap;
  time: number;
}

export interface SourcePreviewRequest {
  start: number;
  end: number;
  frameCount: number;
  previewFps: number;
  maxFrames: number;
  cacheKey: string;
  signature: string;
}

export interface ExtractedFrameBatch {
  frames: WorkerFrameInput[];
  cropRegion: PixelBounds;
}

export interface CropDragState {
  startX: number;
  startY: number;
}

export interface VideoSpriteElements {
  reference: HTMLButtonElement;
  videoPrompt: HTMLTextAreaElement;
  videoSize: HTMLSelectElement;
  videoSeconds: HTMLInputElement;
  videoSourceId: HTMLInputElement;
  videoDirection: HTMLInputElement;
  videoReferenceName: HTMLInputElement;
  pickVideoReference: HTMLButtonElement;
  clearVideoReference: HTMLButtonElement;
  videoConstraintsEnabled: HTMLInputElement;
  videoConstraintsBackground: HTMLSelectElement;
  videoConstraintsBackgroundDescription: HTMLInputElement;
  videoConstraintsFraming: HTMLSelectElement;
  videoConstraintsFixedCamera: HTMLInputElement;
  videoConstraintsLoopAction: HTMLInputElement;
  video: HTMLVideoElement;
  canvas: HTMLCanvasElement;
  placeholder: HTMLElement;
  busy: HTMLElement;
  busyText: HTMLElement;
  file: HTMLElement;
  duration: HTMLElement;
  status: HTMLElement;
  size: HTMLElement;
  frameTotal: HTMLElement;
  frameList: HTMLElement;
  frameCount: HTMLInputElement;
  cols: HTMLInputElement;
  start: HTMLInputElement;
  end: HTMLInputElement;
  frameEdge: HTMLInputElement;
  padding: HTMLInputElement;
  sourcePreviewFps: HTMLInputElement;
  sourcePreviewMax: HTMLInputElement;
  cropFull: HTMLButtonElement;
  cropX: HTMLInputElement;
  cropY: HTMLInputElement;
  cropW: HTMLInputElement;
  cropH: HTMLInputElement;
  bgMode: HTMLSelectElement;
  threshold: HTMLInputElement;
  thresholdLabel: HTMLElement;
  autoTrim: HTMLInputElement;
  transparent: HTMLInputElement;
  viewSource: HTMLButtonElement;
  viewSheet: HTMLButtonElement;
  viewPlayback: HTMLButtonElement;
  timeScrub: HTMLInputElement;
  startRange: HTMLInputElement;
  endRange: HTMLInputElement;
  currentTime: HTMLElement;
  setStart: HTMLButtonElement;
  setEnd: HTMLButtonElement;
  prev: HTMLButtonElement;
  play: HTMLButtonElement;
  next: HTMLButtonElement;
  fps: HTMLInputElement;
  fpsLabel: HTMLElement;
  playbackInfo: HTMLElement;
  redrawFinalCols: HTMLInputElement;
  redrawGroupRows: HTMLInputElement;
  redrawGroupCols: HTMLInputElement;
  redrawResolution: HTMLSelectElement;
  redrawStyle: HTMLSelectElement;
  redrawPrompt: HTMLTextAreaElement;
  redrawNegativePrompt: HTMLInputElement;
  redrawConstraintsEnabled: HTMLInputElement;
  redrawConstraintsBackground: HTMLSelectElement;
  redrawConstraintsBackgroundDescription: HTMLInputElement;
  redrawConstraintsFraming: HTMLSelectElement;
  redrawSummary: HTMLElement;
  redrawApiProfile: HTMLElement;
  redrawPause: HTMLButtonElement;
  redrawDiscard: HTMLButtonElement;
  redrawBatches: HTMLElement;
}

export class VideoSpritePage {
  els: VideoSpriteElements;
  apiSettings!: VideoSpriteApiSettings;
  sourcePath = "";
  sourceName = "";
  videoMeta: VideoProbeResult | null = null;
  cropRegion: PixelBounds | null = null;
  sourcePreviewFrames: SourcePreviewFrame[] = [];
  sourcePreviewIndex = 0;
  sourcePreviewSignature = "";
  sourcePreviewCacheKey = "";
  sourcePreviewCacheStart = 0;
  sourcePreviewCacheEnd = 0;
  sourcePreviewPreparePromise: Promise<void> | null = null;
  sourceVideoReady = false;
  isPreparingSourcePreview = false;
  sourcePreviewSeq = 0;
  currentTimeSeconds = 0;
  cropDrag: CropDragState | null = null;
  viewMode: VideoSpriteView = "source";
  processedFrames: ProcessedFrame[] = [];
  processedFramesOrigin: "none" | "source" | "redraw" = "none";
  spriteSheetBlob: Blob | null = null;
  spriteSheetBitmap: ImageBitmap | null = null;
  savedResult: SavedImageResult | null = null;
  currentFrameIndex = 0;
  isPlaying = false;
  playbackHandle: number | null = null;
  lastPlaybackAt = 0;
  playbackRenderSeq = 0;
  playbackMode: PlaybackMode | null = null;
  isExtracting = false;
  isGeneratingVideo = false;
  videoReferencePath = "";
  workerSeq = 0;
  redrawRun: RedrawRunManifest | null = null;
  redrawPlan: RedrawPlan | null = null;
  isRedrawing = false;
  redrawPauseRequested = false;

  constructor() {
    this.els = this.cacheElements();
  }

  async init(apiSettings: VideoSpriteApiSettings): Promise<void> {
    this.apiSettings = apiSettings;
    this.bindEvents();
    this.syncThresholdLabel();
    this.syncPlaybackLabels();
    this.syncControls();
    this.renderCurrentView();
    await this.cleanupStartupTempFiles();
  }

  cacheElements(): VideoSpriteElements {
    return {
      reference: getById<HTMLButtonElement>("btn-video-sprite-reference"),
      videoPrompt: getById<HTMLTextAreaElement>("video-generation-prompt"),
      videoSize: getById<HTMLSelectElement>("video-generation-size"),
      videoSeconds: getById<HTMLInputElement>("video-generation-seconds"),
      videoSourceId: getById<HTMLInputElement>("video-generation-source-id"),
      videoDirection: getById<HTMLInputElement>("video-generation-direction"),
      videoReferenceName: getById<HTMLInputElement>("video-generation-reference-name"),
      pickVideoReference: getById<HTMLButtonElement>("btn-video-reference-pick"),
      clearVideoReference: getById<HTMLButtonElement>("btn-video-reference-clear"),
      videoConstraintsEnabled: getById<HTMLInputElement>("video-constraints-enabled"),
      videoConstraintsBackground: getById<HTMLSelectElement>("video-constraints-background"),
      videoConstraintsBackgroundDescription: getById<HTMLInputElement>("video-constraints-background-description"),
      videoConstraintsFraming: getById<HTMLSelectElement>("video-constraints-framing"),
      videoConstraintsFixedCamera: getById<HTMLInputElement>("video-constraints-fixed-camera"),
      videoConstraintsLoopAction: getById<HTMLInputElement>("video-constraints-loop-action"),
      video: getById<HTMLVideoElement>("video-sprite-video"),
      canvas: getById<HTMLCanvasElement>("video-sprite-canvas"),
      placeholder: getById("video-sprite-placeholder"),
      busy: getById("video-sprite-busy"),
      busyText: getById("video-sprite-busy-text"),
      file: getById("video-sprite-file"),
      duration: getById("video-sprite-duration"),
      status: getById("video-sprite-status"),
      size: getById("video-sprite-size"),
      frameTotal: getById("video-sprite-frame-total"),
      frameList: getById("video-sprite-frame-list"),
      frameCount: getById<HTMLInputElement>("video-sprite-frame-count"),
      cols: getById<HTMLInputElement>("video-sprite-cols"),
      start: getById<HTMLInputElement>("video-sprite-start"),
      end: getById<HTMLInputElement>("video-sprite-end"),
      frameEdge: getById<HTMLInputElement>("video-sprite-frame-edge"),
      padding: getById<HTMLInputElement>("video-sprite-padding"),
      sourcePreviewFps: getById<HTMLInputElement>("video-sprite-source-preview-fps"),
      sourcePreviewMax: getById<HTMLInputElement>("video-sprite-source-preview-max"),
      cropFull: getById<HTMLButtonElement>("btn-video-sprite-crop-full"),
      cropX: getById<HTMLInputElement>("video-sprite-crop-x"),
      cropY: getById<HTMLInputElement>("video-sprite-crop-y"),
      cropW: getById<HTMLInputElement>("video-sprite-crop-w"),
      cropH: getById<HTMLInputElement>("video-sprite-crop-h"),
      bgMode: getById<HTMLSelectElement>("video-sprite-bg-mode"),
      threshold: getById<HTMLInputElement>("video-sprite-threshold"),
      thresholdLabel: getById("video-sprite-threshold-label"),
      autoTrim: getById<HTMLInputElement>("video-sprite-auto-trim"),
      transparent: getById<HTMLInputElement>("video-sprite-transparent"),
      viewSource: getById<HTMLButtonElement>("btn-video-sprite-view-source"),
      viewSheet: getById<HTMLButtonElement>("btn-video-sprite-view-sheet"),
      viewPlayback: getById<HTMLButtonElement>("btn-video-sprite-view-playback"),
      timeScrub: getById<HTMLInputElement>("video-sprite-time-scrub"),
      startRange: getById<HTMLInputElement>("video-sprite-start-range"),
      endRange: getById<HTMLInputElement>("video-sprite-end-range"),
      currentTime: getById("video-sprite-current-time"),
      setStart: getById<HTMLButtonElement>("btn-video-sprite-set-start"),
      setEnd: getById<HTMLButtonElement>("btn-video-sprite-set-end"),
      prev: getById<HTMLButtonElement>("btn-video-sprite-prev"),
      play: getById<HTMLButtonElement>("btn-video-sprite-play"),
      next: getById<HTMLButtonElement>("btn-video-sprite-next"),
      fps: getById<HTMLInputElement>("video-sprite-fps"),
      fpsLabel: getById("video-sprite-fps-label"),
      playbackInfo: getById("video-sprite-playback-info"),
      redrawFinalCols: getById<HTMLInputElement>("video-redraw-final-cols"),
      redrawGroupRows: getById<HTMLInputElement>("video-redraw-group-rows"),
      redrawGroupCols: getById<HTMLInputElement>("video-redraw-group-cols"),
      redrawResolution: getById<HTMLSelectElement>("video-redraw-resolution"),
      redrawStyle: getById<HTMLSelectElement>("video-redraw-style"),
      redrawPrompt: getById<HTMLTextAreaElement>("video-redraw-prompt"),
      redrawNegativePrompt: getById<HTMLInputElement>("video-redraw-negative-prompt"),
      redrawConstraintsEnabled: getById<HTMLInputElement>("redraw-constraints-enabled"),
      redrawConstraintsBackground: getById<HTMLSelectElement>("redraw-constraints-background"),
      redrawConstraintsBackgroundDescription: getById<HTMLInputElement>("redraw-constraints-background-description"),
      redrawConstraintsFraming: getById<HTMLSelectElement>("redraw-constraints-framing"),
      redrawSummary: getById("video-redraw-summary"),
      redrawApiProfile: getById("video-redraw-api-profile"),
      redrawPause: getById<HTMLButtonElement>("btn-video-redraw-pause"),
      redrawDiscard: getById<HTMLButtonElement>("btn-video-redraw-discard"),
      redrawBatches: getById("video-redraw-batches"),
    };
  }

  bindEvents(): void {
    [
      this.els.videoConstraintsEnabled,
      this.els.videoConstraintsBackground,
      this.els.redrawConstraintsEnabled,
      this.els.redrawConstraintsBackground,
    ].forEach((input) => {
      input.addEventListener("change", () => this.syncGenerationConstraintControls());
    });
    this.els.pickVideoReference.addEventListener("click", () => {
      void this.handlePickVideoReference();
    });
    this.els.clearVideoReference.addEventListener("click", () => {
      this.clearVideoReference();
    });
    this.els.reference.addEventListener("click", () => this.handleUseAsReference());
    this.els.threshold.addEventListener("input", () => this.syncThresholdLabel());
    this.els.cropFull.addEventListener("click", () => this.setFullCropRegion());
    [this.els.cropX, this.els.cropY, this.els.cropW, this.els.cropH].forEach((input) => {
      input.addEventListener("change", () => this.handleCropInput());
    });
    this.els.start.addEventListener("change", () => {
      void this.handleStartTimeChanged(true);
    });
    this.els.end.addEventListener("change", () => {
      this.handleEndTimeChanged();
    });
    [this.els.sourcePreviewFps, this.els.sourcePreviewMax].forEach((input) => {
      input.addEventListener("input", () => this.handleSourcePreviewSettingChanged());
      input.addEventListener("change", () => this.handleSourcePreviewSettingChanged());
    });
    this.els.timeScrub.addEventListener("input", () => this.handleSourceScrubInput());
    this.els.timeScrub.addEventListener("change", () => {
      const time = parseInputNumber(this.els.timeScrub.value, 0);
      this.showSourceFrameAtTime(time);
    });
    this.els.startRange.addEventListener("input", () => {
      this.handleStartRangeInput();
    });
    this.els.startRange.addEventListener("change", () => {
      const time = parseInputNumber(this.els.startRange.value, 0);
      this.showSourceFrameAtTime(time);
    });
    this.els.endRange.addEventListener("input", () => this.handleEndRangeInput());
    this.els.setStart.addEventListener("click", () => {
      void this.setCurrentTimeAsRangePoint("start");
    });
    this.els.setEnd.addEventListener("click", () => {
      void this.setCurrentTimeAsRangePoint("end");
    });
    this.els.viewSource.addEventListener("click", () => this.setViewMode("source"));
    this.els.viewSheet.addEventListener("click", () => this.setViewMode("sheet"));
    this.els.viewPlayback.addEventListener("click", () => this.setViewMode("playback"));
    this.els.prev.addEventListener("click", () => this.stepPlayback(-1));
    this.els.next.addEventListener("click", () => this.stepPlayback(1));
    this.els.play.addEventListener("click", () => {
      void this.togglePlayback();
    });
    this.els.fps.addEventListener("input", () => this.handlePlaybackFpsChanged());
    this.els.video.addEventListener("timeupdate", () => this.handleSourceVideoTimeUpdate());
    this.els.video.addEventListener("ended", () => this.handleSourceVideoEnded());
    this.els.canvas.addEventListener("pointerdown", (event) => this.handleCropPointerDown(event));
    this.els.canvas.addEventListener("pointermove", (event) => this.handleCropPointerMove(event));
    this.els.canvas.addEventListener("pointerup", (event) => this.handleCropPointerUp(event));
    this.els.canvas.addEventListener("pointercancel", (event) => this.handleCropPointerUp(event));
    [
      this.els.frameCount,
      this.els.redrawFinalCols,
      this.els.redrawGroupRows,
      this.els.redrawGroupCols,
    ].forEach((input) => {
      input.addEventListener("input", () => this.refreshRedrawPlanSummary());
    });
    this.els.redrawResolution.addEventListener("change", () => {
      this.refreshRedrawPlanSummary();
    });
    this.els.redrawFinalCols.addEventListener("change", () => {
      void this.handleRedrawFinalColsChanged();
    });
    this.els.redrawPause.addEventListener("click", () => {
      void this.handlePauseRedraw();
    });
    this.els.redrawDiscard.addEventListener("click", () => {
      void this.handleDiscardRedraw();
    });
  }

  readVideoGenerationConstraints(): VideoGenerationConstraints {
    return {
      enabled: this.els.videoConstraintsEnabled.checked,
      backgroundMode: this.els.videoConstraintsBackground.value as VideoGenerationConstraints["backgroundMode"],
      backgroundDescription: this.els.videoConstraintsBackgroundDescription.value,
      framing: this.els.videoConstraintsFraming.value as GenerationFraming,
      fixedCamera: this.els.videoConstraintsFixedCamera.checked,
      loopAction: this.els.videoConstraintsLoopAction.checked,
    };
  }

  readRedrawGenerationConstraints(): ImageGenerationConstraints {
    return {
      enabled: this.els.redrawConstraintsEnabled.checked,
      backgroundMode: this.els.redrawConstraintsBackground.value as GenerationBackgroundMode,
      backgroundDescription: this.els.redrawConstraintsBackgroundDescription.value,
      framing: this.els.redrawConstraintsFraming.value as GenerationFraming,
    };
  }

  syncGenerationConstraintControls(): void {
    const busy = this.isBusy();
    const videoEnabled = this.els.videoConstraintsEnabled.checked;
    this.els.videoConstraintsEnabled.disabled = busy;
    this.els.videoConstraintsBackground.disabled = busy || !videoEnabled;
    this.els.videoConstraintsFraming.disabled = busy || !videoEnabled;
    this.els.videoConstraintsFixedCamera.disabled = busy || !videoEnabled;
    this.els.videoConstraintsLoopAction.disabled = busy || !videoEnabled;
    this.els.videoConstraintsBackgroundDescription.disabled =
      busy || !videoEnabled || !usesBackgroundDescription(
        this.els.videoConstraintsBackground.value as GenerationBackgroundMode
      );

    const redrawLocked = busy || Boolean(this.redrawRun);
    const redrawEnabled = this.els.redrawConstraintsEnabled.checked;
    this.els.redrawConstraintsEnabled.disabled = redrawLocked;
    this.els.redrawConstraintsBackground.disabled = redrawLocked || !redrawEnabled;
    this.els.redrawConstraintsFraming.disabled = redrawLocked || !redrawEnabled;
    this.els.redrawConstraintsBackgroundDescription.disabled =
      redrawLocked || !redrawEnabled || !usesBackgroundDescription(
        this.els.redrawConstraintsBackground.value as GenerationBackgroundMode
      );
  }


}

export interface VideoSpritePage extends VideoSpriteGenerationMethods, VideoSpriteSourceMethods, VideoSpriteExtractionMethods, VideoSpriteRedrawStartMethods, VideoSpriteRedrawRunMethods, VideoSpriteOptionsMethods, VideoSpriteRenderMethods, VideoSpritePlaybackMethods, VideoSpriteControlsMethods, VideoSpriteWorkspaceMethods { }

Object.assign(
  VideoSpritePage.prototype,
  videoSpriteGenerationMethods,
  videoSpriteSourceMethods,
  videoSpriteExtractionMethods,
  videoSpriteRedrawStartMethods,
  videoSpriteRedrawRunMethods,
  videoSpriteOptionsMethods,
  videoSpriteRenderMethods,
  videoSpritePlaybackMethods,
  videoSpriteControlsMethods,
  videoSpriteWorkspaceMethods
);
