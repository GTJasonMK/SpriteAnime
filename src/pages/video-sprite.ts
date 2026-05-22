import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import {
  cleanupPreparedVideoFile,
  cleanupVideoFrameBatchDir,
  cleanupVideoSpriteTempFiles,
  extractVideoFramesWithFfmpeg,
  generateVideo,
  importVideoToLibrary,
  logVideoSpriteMessage,
  openVideoFile,
  prepareVideoFileForPlayback,
  probeVideoFile,
  readFileAsBase64,
  saveSpriteSheetDataUrl,
  type GeneratedVideoResult,
  type SavedImageResult,
  type VideoExtractEvent,
  type VideoExtractRegion,
  type VideoGenerationEvent,
  type VideoProbeResult,
} from "../api/commands";
import { mapWithConcurrency, nextFrame } from "../utils/async";
import { loadBitmapFromBase64 } from "../utils/bitmap";
import {
  clearPreviewCanvas,
  fitPreviewCanvasSize,
  preparePreviewCanvas,
} from "../utils/canvas";
import { getById, queryAll } from "../utils/dom";
import {
  formatInputNumber,
  formatSeconds,
  parseClampedInt as clampInt,
  parseInputNumber,
} from "../utils/number";
import { getFileName, stripFileExtension as stripExtension } from "../utils/path";
import type { GeneratorPage } from "./generator";
import type {
  BackgroundMode,
  PixelBounds,
  VideoSpriteWorkerFrameInput as WorkerFrameInput,
  VideoSpriteWorkerMessage as WorkerMessage,
  VideoSpriteWorkerRequest as WorkerRequest,
  VideoSpriteWorkerSuccessMessage as WorkerSuccessMessage,
} from "./video-sprite-types";
import { clickTab } from "./navigation";
import {
  clampPixelBounds,
  formatPixelBounds,
  parseBackgroundMode,
} from "./video-sprite-utils";
import { renderVideoFrameList } from "./video-frame-list";
import { setBusyState, setButtonState } from "../utils/ui";

type VideoSpriteView = "source" | "sheet" | "playback";
type PlaybackMode = "source" | "output";

interface ExtractionOptions {
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

interface ProcessedFrame {
  blob: Blob;
  url: string;
  bitmap: ImageBitmap | null;
  time: number;
  width: number;
  height: number;
}

interface SourcePreviewFrame {
  bitmap: ImageBitmap;
  time: number;
  width: number;
  height: number;
}

interface SourcePreviewRequest {
  start: number;
  end: number;
  frameCount: number;
  previewFps: number;
  maxFrames: number;
  cacheKey: string;
  signature: string;
}

interface ExtractedFrameBatch {
  frames: WorkerFrameInput[];
  cropRegion: PixelBounds;
}

interface CropDragState {
  startX: number;
  startY: number;
}

interface VideoSpriteElements {
  pickVideo: HTMLButtonElement;
  extract: HTMLButtonElement;
  generateVideo: HTMLButtonElement;
  save: HTMLButtonElement;
  reference: HTMLButtonElement;
  videoPrompt: HTMLTextAreaElement;
  videoSize: HTMLSelectElement;
  videoSeconds: HTMLSelectElement;
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
}

export class VideoSpritePage {
  private els: VideoSpriteElements;
  private generatorPage: GeneratorPage | null = null;
  private sourcePath = "";
  private sourceName = "";
  private videoMeta: VideoProbeResult | null = null;
  private cropRegion: PixelBounds | null = null;
  private sourcePreviewBitmap: ImageBitmap | null = null;
  private sourcePreviewFrames: SourcePreviewFrame[] = [];
  private sourcePreviewIndex = 0;
  private sourcePreviewSignature = "";
  private sourcePreviewCacheKey = "";
  private sourcePreviewCacheStart = 0;
  private sourcePreviewCacheEnd = 0;
  private sourcePreviewPreparePromise: Promise<void> | null = null;
  private sourceVideoReady = false;
  private preparedVideoPath = "";
  private isPreparingSourcePreview = false;
  private sourcePreviewSeq = 0;
  private cropDrag: CropDragState | null = null;
  private viewMode: VideoSpriteView = "source";
  private processedFrames: ProcessedFrame[] = [];
  private spriteSheetBlob: Blob | null = null;
  private spriteSheetBitmap: ImageBitmap | null = null;
  private savedResult: SavedImageResult | null = null;
  private currentFrameIndex = 0;
  private isPlaying = false;
  private playbackHandle: number | null = null;
  private lastPlaybackAt = 0;
  private playbackRenderSeq = 0;
  private playbackMode: PlaybackMode | null = null;
  private isExtracting = false;
  private isGeneratingVideo = false;
  private workerSeq = 0;

  constructor() {
    this.els = this.cacheElements();
  }

  init(generatorPage: GeneratorPage): void {
    this.generatorPage = generatorPage;
    this.bindEvents();
    this.syncThresholdLabel();
    this.syncPlaybackLabels();
    this.syncControls();
    this.renderCurrentView();
    void this.cleanupStartupTempFiles();
  }

  private cacheElements(): VideoSpriteElements {
    return {
      pickVideo: getById<HTMLButtonElement>("btn-video-sprite-pick"),
      extract: getById<HTMLButtonElement>("btn-video-sprite-extract"),
      generateVideo: getById<HTMLButtonElement>("btn-video-generate"),
      save: getById<HTMLButtonElement>("btn-video-sprite-save"),
      reference: getById<HTMLButtonElement>("btn-video-sprite-reference"),
      videoPrompt: getById<HTMLTextAreaElement>("video-generation-prompt"),
      videoSize: getById<HTMLSelectElement>("video-generation-size"),
      videoSeconds: getById<HTMLSelectElement>("video-generation-seconds"),
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
    };
  }

  private bindEvents(): void {
    this.els.pickVideo.addEventListener("click", () => this.handlePickVideo());
    this.els.generateVideo.addEventListener("click", () => {
      void this.handleGenerateVideo();
    });
    this.els.extract.addEventListener("click", () => this.handleExtract());
    this.els.save.addEventListener("click", () => this.handleSave());
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
      void this.showSourceFrameAtTime(parseInputNumber(this.els.timeScrub.value, 0), true)
        .catch((err) => this.handleSourcePreviewError(err));
    });
    this.els.startRange.addEventListener("input", () => {
      void this.handleStartRangeInput();
    });
    this.els.startRange.addEventListener("change", () => {
      void this.showSourceFrameAtTime(parseInputNumber(this.els.startRange.value, 0), true)
        .catch((err) => this.handleSourcePreviewError(err));
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
  }

  private async handlePickVideo(): Promise<void> {
    if (this.isBusy()) return;

    try {
      const file = await openVideoFile();
      const imported = await importVideoToLibrary(file.file_path);
      await this.loadSourceVideo(
        imported.file_path,
        imported.file_name || file.file_name || getFileName(imported.file_path) || "animation",
        "pick video"
      );
    } catch (err) {
      if (!String(err).includes("用户取消")) {
        console.error("[video-sprite] 选择视频失败:", err);
        await this.log(`pick video failed | ${String(err)}`);
        this.setStatus(`选择视频失败: ${String(err)}`);
      }
    }
  }

  private async handleGenerateVideo(): Promise<void> {
    if (this.isBusy()) return;

    const prompt = this.els.videoPrompt.value.trim();
    if (!prompt) {
      this.setStatus("请输入视频生成提示词");
      this.els.videoPrompt.focus();
      return;
    }

    const settings = this.generatorPage?.getActiveApiSettings();
    const model = settings?.videoModel?.trim() || "sora-2";
    const apiMode = settings?.videoApiMode || "chat_completions";
    const size = this.els.videoSize.value.trim() || "1280x720";
    const seconds = this.els.videoSeconds.value.trim() || "4";

    this.stopPlayback();
    this.isGeneratingVideo = true;
    this.savedResult = null;
    this.clearGeneratedOutput();
    this.syncControls();
    this.showBusy(true, "正在调用视频生成模型...");
    this.setStatus("正在调用视频生成模型...");
    await nextFrame();

    try {
      await this.log(
        `ai video generate start | mode=${apiMode} model=${model} size=${size} seconds=${seconds} profile=${settings?.profileName || ""}`
      );
      const channel = new Channel<VideoGenerationEvent>();
      channel.onmessage = (event: VideoGenerationEvent) => this.handleVideoGenerationProgress(event);
      const result = await generateVideo(
        channel,
        settings?.videoApiKey || settings?.apiKey || "",
        settings?.videoApiBase || settings?.apiBase || "",
        settings?.videoProxyUrl || settings?.proxyUrl || "",
        prompt,
        model,
        apiMode,
        size,
        seconds
      );
      await this.applyGeneratedVideo(result);
      await this.log(`ai video generate ok | path=${result.file_path}`);
    } catch (err) {
      console.error("[video-sprite] 视频生成失败:", err);
      await this.log(`ai video generate failed | ${String(err)}`);
      this.setStatus(`视频生成失败: ${String(err)}`);
    } finally {
      this.isGeneratingVideo = false;
      this.showBusy(false);
      this.syncControls();
      this.renderCurrentView();
    }
  }

  private handleVideoGenerationProgress(event: VideoGenerationEvent): void {
    switch (event.event) {
      case "Started":
      case "Submitting":
        this.setStatus("正在调用视频生成模型...");
        this.showBusy(true, "正在调用视频生成模型...");
        break;
      case "Downloading":
        this.setStatus("正在读取生成视频...");
        this.showBusy(true, "正在读取生成视频...");
        break;
      case "Saving":
        this.setStatus("正在保存生成视频...");
        this.showBusy(true, "正在保存生成视频...");
        break;
      case "Completed":
        this.setStatus("视频生成完成，正在加载到抽帧工作台...");
        this.showBusy(true, "正在加载生成视频...");
        break;
      case "Error":
        this.setStatus(`视频生成失败: ${String(event.data?.message || "")}`);
        break;
      default:
        break;
    }
  }

  private async applyGeneratedVideo(result: GeneratedVideoResult): Promise<void> {
    await this.loadSourceVideo(
      result.file_path,
      result.file_name || getFileName(result.file_path) || "generated_video.mp4",
      "ai video"
    );
    const elapsed = Number(result.duration_seconds);
    const suffix = Number.isFinite(elapsed) ? `，耗时 ${elapsed.toFixed(2)}s` : "";
    this.setStatus(`已生成并加载视频: ${this.sourceName}${suffix}`);
  }

  private async loadSourceVideo(
    filePath: string,
    fileName: string,
    context: string
  ): Promise<void> {
    this.stopPlayback();
    this.clearSourceVideo();
    await this.releasePreparedVideoFile("replace source");
    this.sourcePath = filePath;
    this.sourceName = fileName || getFileName(filePath) || "animation";
    this.videoMeta = null;
    this.savedResult = null;
    this.cropRegion = null;
    this.clearGeneratedOutput();
    this.clearSourcePreview();
    this.setViewMode("source", false);
    this.showBusy(false);

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
    if (await this.tryLoadSourceVideo()) {
      this.renderSourceView();
      this.setStatus("已直接加载源视频，可播放并拖拽选择画面区域");
    } else {
      await this.refreshSourcePreviewAtStart();
      this.setStatus("当前 WebView 不支持直接播放该视频，已使用 ffmpeg 单帧预览");
    }
    this.syncControls();
  }

  private async refreshSourcePreviewAtStart(): Promise<void> {
    if (!this.sourcePath) return;

    const start = clampNumber(
      parseInputNumber(this.els.start.value, 0),
      0,
      Math.max(this.getVideoDuration(), 0)
    );
    await this.showSourceFrameAtTime(start, true);
  }

  private async tryLoadSourceVideo(): Promise<boolean> {
    this.clearSourceVideo();
    const directError = await this.tryLoadVideoPath(this.sourcePath, "direct");
    if (!directError) {
      await this.log("direct video playback ok");
      return true;
    }
    await this.log(`direct video playback failed | ${directError}`);

    try {
      const copied = await prepareVideoFileForPlayback(this.sourcePath);
      const copiedError = await this.tryLoadVideoPath(copied.file_path, "copied");
      if (!copiedError) {
        this.preparedVideoPath = copied.file_path;
        await this.log(`copied video playback ok | path=${copied.file_path}`);
        return true;
      }
      await this.log(`copied video playback failed | ${copiedError}`);
      await this.cleanupPreparedVideoPath(copied.file_path, "copied playback failed");
    } catch (err) {
      await this.log(`copy video for playback failed | ${String(err)}`);
    }

    this.clearSourceVideo();
    return false;
  }

  private async tryLoadVideoPath(path: string, label: string): Promise<string | null> {
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
  }

  private async loadBackendSourcePreview(time: number): Promise<void> {
    this.clearSourceVideo();
    this.setStatus("正在用 ffmpeg 读取预览帧...");
    await nextFrame();
    const channel = new Channel<VideoExtractEvent>();
    channel.onmessage = (event: VideoExtractEvent) => {
      if (event.event === "Probing") {
        this.setStatus("正在读取视频元数据...");
      } else if (event.event === "ExtractingFrame") {
        this.setStatus("正在用 ffmpeg 读取预览帧...");
      }
    };
    let outputDir = "";
    try {
      const result = await extractVideoFramesWithFfmpeg(
        channel,
        this.sourcePath,
        1,
        time,
        time,
        undefined,
        1280
      );
      outputDir = result.output_dir;
      const first = result.frames[0];
      if (!first) {
        throw new Error("无法抽取视频预览帧");
      }
      const bitmap = await loadBitmapFromPath(first.path);
      this.setSourcePreviewBitmap(bitmap);
      this.videoMeta = {
        duration_seconds: result.duration_seconds,
        width: result.width,
        height: result.height,
      };
      this.renderSourceView();
    } finally {
      await this.cleanupVideoFrameOutputDir(outputDir, "source single preview");
    }
  }

  private async prepareSourcePreviewFrames(): Promise<void> {
    if (!this.sourcePath || !this.videoMeta) return;
    const request = this.getSourcePreviewRequest();
    if (this.hasPreparedSourcePreview(request)) {
      await this.log(`source preview reuse | signature=${request.signature}`);
      this.sourcePreviewIndex = findNearestSourceFrameIndex(this.sourcePreviewFrames, request.start);
      this.syncTimeControls();
      this.renderCurrentView();
      return;
    }
    if (this.sourcePreviewPreparePromise) {
      await this.log(`source preview await existing prepare | signature=${request.signature}`);
      await this.sourcePreviewPreparePromise;
      if (this.hasPreparedSourcePreview(request)) {
        this.sourcePreviewIndex = findNearestSourceFrameIndex(this.sourcePreviewFrames, request.start);
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
  }

  private async prepareSourcePreviewFramesForRequest(request: SourcePreviewRequest): Promise<void> {
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
          const index = Number(event.data?.index) || 0;
          const total = Number(event.data?.total) || frameCount;
          const text = `正在准备源视频播放预览 ${index}/${total}`;
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
          width: frame.width,
          height: frame.height,
        };
      });
      if (seq !== this.sourcePreviewSeq) {
        closeSourcePreviewFrames(frames);
        return;
      }
      closeSourcePreviewFrames(this.sourcePreviewFrames);
      if (this.sourcePreviewBitmap) {
        this.sourcePreviewBitmap.close();
        this.sourcePreviewBitmap = null;
      }
      this.sourcePreviewFrames = frames;
      this.sourcePreviewSignature = signature;
      this.sourcePreviewCacheKey = cacheKey;
      this.sourcePreviewCacheStart = start;
      this.sourcePreviewCacheEnd = end;
      this.sourcePreviewIndex = findNearestSourceFrameIndex(frames, start);
      this.syncTimeControls();
      this.renderCurrentView();
      this.setStatus("源视频播放预览已就绪");
      await this.log(`source preview prepare ok | signature=${signature} frames=${frames.length}`);
    } catch (err) {
      console.error("[video-sprite] 源视频播放预览准备失败:", err);
      await this.log(`source preview frames failed | ${String(err)}`);
      this.setStatus(`源视频播放预览准备失败: ${String(err)}`);
    } finally {
      await this.cleanupVideoFrameOutputDir(outputDir, "source playback preview");
      if (seq === this.sourcePreviewSeq) {
        this.isPreparingSourcePreview = false;
        this.showBusy(false);
        this.syncControls();
      }
    }
  }

  private async showSourceFrameAtTime(time: number, allowBackendFallback: boolean): Promise<void> {
    const duration = this.getVideoDuration();
    const clampedTime = clampNumber(time, 0, Math.max(duration, 0));
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
    if (allowBackendFallback) {
      await this.loadBackendSourcePreview(clampedTime);
    } else {
      this.renderCurrentView();
    }
  }

  private async handleStartTimeChanged(refreshPreview: boolean): Promise<void> {
    const { start, end } = this.readTimeRangeInputs();
    this.setTimeRangeInputs(start, end);
    this.syncTimeRangeInputChange();
    if (refreshPreview) {
      await this.showSourceFrameAtTime(start, true).catch((err) => this.handleSourcePreviewError(err));
    }
  }

  private handleEndTimeChanged(): void {
    const { start, end } = this.readTimeRangeInputs();
    this.setTimeRangeInputs(start, end);
    this.syncTimeRangeInputChange();
  }

  private handleSourceScrubInput(): void {
    void this.showSourceFrameAtTime(parseInputNumber(this.els.timeScrub.value, 0), false);
  }

  private async handleStartRangeInput(): Promise<void> {
    const duration = this.getVideoDuration();
    const start = clampNumber(parseInputNumber(this.els.startRange.value, 0), 0, Math.max(duration, 0));
    const end = Math.max(start, parseInputNumber(this.els.end.value, duration));
    this.setTimeRangeInputs(start, end);
    this.syncTimeRangeInputChange();
    await this.showSourceFrameAtTime(start, false);
  }

  private handleEndRangeInput(): void {
    const duration = this.getVideoDuration();
    const start = parseInputNumber(this.els.start.value, 0);
    const end = clampNumber(parseInputNumber(this.els.endRange.value, duration), start, Math.max(duration, start));
    this.els.end.value = formatInputNumber(end);
    this.syncTimeRangeInputChange();
  }

  private readTimeRangeInputs(): { start: number; end: number; max: number } {
    const max = Math.max(this.getVideoDuration(), 0);
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, max);
    const end = clampNumber(parseInputNumber(this.els.end.value, max), start, max);
    return { start, end, max };
  }

  private setTimeRangeInputs(start: number, end: number): void {
    this.els.start.value = formatInputNumber(start);
    this.els.end.value = formatInputNumber(end);
  }

  private syncTimeRangeInputChange(): void {
    this.clearSourcePlaybackFramesIfStale();
    this.syncTimeControls();
  }

  private async setCurrentTimeAsRangePoint(point: "start" | "end"): Promise<void> {
    const time = this.getSourceCurrentTime();
    if (point === "start") {
      this.els.start.value = formatInputNumber(Math.min(time, parseInputNumber(this.els.end.value, time)));
      await this.handleStartTimeChanged(false);
    } else {
      this.els.end.value = formatInputNumber(Math.max(time, parseInputNumber(this.els.start.value, 0)));
      this.handleEndTimeChanged();
    }
  }

  private async handleSourcePreviewError(err: unknown): Promise<void> {
    console.error("[video-sprite] 预览帧读取失败:", err);
    await this.log(`preview failed | ${String(err)}`);
    this.setStatus(`预览帧读取失败: ${String(err)}；详见 logs/video-sprite.log`);
  }

  private handleBackendExtractProgress(event: VideoExtractEvent): void {
    switch (event.event) {
      case "Started":
        this.setStatus("ffmpeg 抽帧任务已启动...");
        this.showBusy(true, "ffmpeg 抽帧任务已启动...");
        break;
      case "Probing":
        this.setStatus("正在读取视频元数据...");
        this.showBusy(true, "正在读取视频元数据...");
        break;
      case "ExtractingFrame": {
        const data = event.data || {};
        const index = Number(data.index) || 0;
        const total = Number(data.total) || 0;
        const time = Number(data.time_seconds);
        const suffix = Number.isFinite(time) ? ` @ ${time.toFixed(2)}s` : "";
        const text = `ffmpeg 抽取第 ${index}/${total} 帧${suffix}`;
        this.setStatus(text);
        this.showBusy(true, text);
        break;
      }
      case "Completed": {
        const frames = Number(event.data?.frames) || 0;
        const text = `ffmpeg 已抽取 ${frames} 帧`;
        this.setStatus(text);
        this.showBusy(true, text);
        break;
      }
      case "Error":
        this.setStatus(`ffmpeg 抽帧失败: ${String(event.data?.message || "")}`);
        break;
      default:
        break;
    }
  }

  private async handleExtract(): Promise<void> {
    if (this.isBusy() || !this.sourcePath || !this.cropRegion) return;

    this.stopPlayback();
    const options = this.readOptions();
    this.isExtracting = true;
    this.savedResult = null;
    this.clearGeneratedOutput();
    this.syncControls();
    this.showBusy(true, "正在抽取视频帧...");
    this.setStatus("准备异步生成...");
    await nextFrame();

    try {
      await this.log(
        `generate start | path=${this.sourcePath} crop=${formatPixelBounds(options.cropRegion)} start=${options.start.toFixed(3)} end=${options.end.toFixed(3)} frames=${options.frameCount}`
      );
      const frameBatch = await this.extractFrameInputs(options);
      this.showBusy(true, "正在处理帧并合成 PNG...");
      await nextFrame();
      const result = await this.processFramesInWorker(frameBatch.frames, {
        ...options,
        cropRegion: frameBatch.cropRegion,
      });
      await this.applyWorkerResult(result);
      this.setViewMode("sheet", false);
      this.setStatus(`已生成 ${this.processedFrames.length} 帧序列图`);
      await this.log(`generate ok | frames=${this.processedFrames.length}`);
    } catch (err) {
      console.error("[video-sprite] 抽帧失败:", err);
      await this.log(`generate failed | ${String(err)}`);
      this.setStatus(`抽帧失败: ${String(err)}；详见 logs/video-sprite.log`);
    } finally {
      this.isExtracting = false;
      this.showBusy(false);
      this.syncControls();
      this.renderCurrentView();
    }
  }

  private async extractFrameInputs(options: ExtractionOptions): Promise<ExtractedFrameBatch> {
    this.setStatus("正在用 ffmpeg 异步抽取视频帧...");
    this.showBusy(true, "正在用 ffmpeg 异步抽取视频帧...");
    await nextFrame();
    const channel = new Channel<VideoExtractEvent>();
    channel.onmessage = (event: VideoExtractEvent) => this.handleBackendExtractProgress(event);
    let outputDir = "";
    try {
      const result = await extractVideoFramesWithFfmpeg(
        channel,
        this.sourcePath,
        options.frameCount,
        options.start,
        options.end,
        toVideoExtractRegion(options.cropRegion),
        getBackendMaxExtractEdge(options)
      );
      outputDir = result.output_dir;
      this.videoMeta = {
        duration_seconds: result.duration_seconds,
        width: result.width,
        height: result.height,
      };

      let readCount = 0;
      const frames = await mapWithConcurrency(result.frames, 4, async (frame) => {
        const base64 = await readFileAsBase64(frame.path);
        readCount += 1;
        const text = `读取抽取帧 ${readCount}/${result.frames.length}...`;
        this.setStatus(text);
        this.showBusy(true, text);
        return {
          base64,
          time: frame.time_seconds,
        };
      });
      const firstFrame = result.frames[0];
      return {
        frames,
        cropRegion: {
          x: 0,
          y: 0,
          width: firstFrame?.width || options.cropRegion.width,
          height: firstFrame?.height || options.cropRegion.height,
        },
      };
    } finally {
      await this.cleanupVideoFrameOutputDir(outputDir, "final frame extraction");
    }
  }

  private processFramesInWorker(
    frames: WorkerFrameInput[],
    options: ExtractionOptions
  ): Promise<WorkerSuccessMessage> {
    const id = ++this.workerSeq;
    const worker = new Worker(new URL("./video-sprite-worker.ts", import.meta.url), {
      type: "module",
    });

    return new Promise((resolve, reject) => {
      worker.onmessage = (event: MessageEvent<WorkerMessage>) => {
        const message = event.data;
        if (message.id !== id) return;
        if (message.type === "progress") {
          this.setStatus(message.message);
          this.showBusy(true, `${message.message} (${message.done}/${message.total})`);
          return;
        }
        worker.terminate();
        if (message.type === "success") {
          resolve(message);
        } else {
          reject(new Error(message.error));
        }
      };
      worker.onerror = (event) => {
        worker.terminate();
        reject(new Error(event.message || "序列帧 worker 执行失败"));
      };
      const request: WorkerRequest = {
        id,
        frames,
        options: {
          cols: options.cols,
          maxFrameEdge: options.maxFrameEdge,
          padding: options.padding,
          threshold: options.threshold,
          bgMode: options.bgMode,
          autoTrim: options.autoTrim,
          transparent: options.transparent,
          cropRegion: options.cropRegion,
        },
      };
      worker.postMessage(request);
    });
  }

  private async applyWorkerResult(result: WorkerSuccessMessage): Promise<void> {
    this.clearGeneratedOutput();
    this.spriteSheetBlob = result.sheetBlob;
    this.spriteSheetBitmap = await createImageBitmap(result.sheetBlob);
    this.els.size.textContent = `尺寸: ${result.sheetWidth} x ${result.sheetHeight}`;
    this.els.frameTotal.textContent = `${result.frames.length} 帧`;

    for (let i = 0; i < result.frames.length; i += 1) {
      const frame = result.frames[i];
      this.processedFrames.push({
        blob: frame.blob,
        url: URL.createObjectURL(frame.blob),
        bitmap: null,
        time: frame.time,
        width: frame.width,
        height: frame.height,
      });
    }
    this.currentFrameIndex = 0;
    this.renderFrameList();
    this.syncPlaybackLabels();
  }

  private renderFrameList(): void {
    renderVideoFrameList({
      container: this.els.frameList,
      frames: this.processedFrames,
      currentIndex: this.currentFrameIndex,
      formatTime: formatSeconds,
      onSelect: (index) => {
        this.currentFrameIndex = index;
        this.setViewMode("playback", false);
        this.renderCurrentView();
        this.syncPlaybackLabels();
        this.updateFrameCurrentState();
      },
    });
  }

  private async handleSave(): Promise<SavedImageResult | null> {
    if (!this.spriteSheetBlob) {
      this.setStatus("请先生成序列帧图");
      return null;
    }

    this.setStatus("正在保存序列帧图...");
    this.els.save.disabled = true;

    try {
      const dataUrl = await blobToDataUrl(this.spriteSheetBlob);
      const result = await saveSpriteSheetDataUrl(dataUrl, getOutputFileName(this.sourceName));
      this.savedResult = result;
      this.setStatus(`已保存: ${result.file_name}`);
      this.syncControls();
      return result;
    } catch (err) {
      console.error("[video-sprite] 保存失败:", err);
      this.setStatus(`保存失败: ${String(err)}`);
      this.syncControls();
      return null;
    }
  }

  private async handleUseAsReference(): Promise<void> {
    if (!this.generatorPage) return;

    const result = this.savedResult || await this.handleSave();
    if (!result) return;

    this.generatorPage.setExternalReferenceImage(result.file_path, result.file_name);
    clickTab("generator");
  }

  private readOptions(): ExtractionOptions {
    const duration = this.getVideoDuration();
    const frameCount = clampInt(this.els.frameCount.value, 12, 2, 64);
    const cols = clampInt(this.els.cols.value, 6, 1, 12);
    const maxFrameEdge = clampInt(this.els.frameEdge.value, 256, 64, 768);
    const padding = clampInt(this.els.padding.value, 12, 0, 96);
    const threshold = clampInt(this.els.threshold.value, 36, 1, 160);
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, Math.max(duration, 0));
    const fallbackEnd = duration > 0 ? duration : start;
    let end = clampNumber(parseInputNumber(this.els.end.value, fallbackEnd), start, fallbackEnd);
    if (duration > 0) {
      end = Math.min(end, Math.max(start, duration - 0.03));
    }
    if (end <= start) {
      end = start;
    }
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
  }

  private getSelectedTimeRange(): { start: number; end: number } {
    const { start, end } = this.readTimeRangeInputs();
    return { start, end };
  }

  private getSourcePreviewRequest(): SourcePreviewRequest {
    const { start, end } = this.getSelectedTimeRange();
    const { previewFps, maxFrames } = this.readSourcePreviewSettings();
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
  }

  private readSourcePreviewSettings(): { previewFps: number; maxFrames: number } {
    const previewFps = clampInt(this.els.sourcePreviewFps.value, 6, 1, 30);
    const maxFrames = clampInt(this.els.sourcePreviewMax.value, 72, 2, 240);
    this.els.sourcePreviewFps.value = String(previewFps);
    this.els.sourcePreviewMax.value = String(maxFrames);
    return { previewFps, maxFrames };
  }

  private handleSourcePreviewSettingChanged(): void {
    this.readSourcePreviewSettings();
    this.syncControls();
  }

  private hasPreparedSourcePreview(request: SourcePreviewRequest = this.getSourcePreviewRequest()): boolean {
    if (this.sourcePreviewFrames.length === 0) return false;
    if (this.sourcePreviewSignature === request.signature) return true;
    if (this.sourcePreviewCacheKey !== request.cacheKey) return false;
    const epsilon = 0.05;
    return request.start >= this.sourcePreviewCacheStart - epsilon &&
      request.end <= this.sourcePreviewCacheEnd + epsilon;
  }

  private handleCropInput(): void {
    const full = this.getFullRegion();
    if (!full) return;
    const width = clampInt(this.els.cropW.value, full.width, 1, full.width);
    const height = clampInt(this.els.cropH.value, full.height, 1, full.height);
    const x = clampInt(this.els.cropX.value, 0, 0, Math.max(0, full.width - width));
    const y = clampInt(this.els.cropY.value, 0, 0, Math.max(0, full.height - height));
    this.cropRegion = { x, y, width, height };
    this.syncCropInputs();
    this.renderCurrentView();
  }

  private setFullCropRegion(render: boolean = true): void {
    const full = this.getFullRegion();
    if (!full) return;
    this.cropRegion = full;
    this.syncCropInputs();
    if (render) {
      this.renderCurrentView();
    }
  }

  private getCropRegionOrFull(): PixelBounds {
    const full = this.getFullRegion() || { x: 0, y: 0, width: 1, height: 1 };
    return clampPixelBounds(this.cropRegion || full, full.width, full.height);
  }

  private getFullRegion(): PixelBounds | null {
    const width = this.getVideoWidth();
    const height = this.getVideoHeight();
    if (width <= 0 || height <= 0) {
      return null;
    }
    return { x: 0, y: 0, width, height };
  }

  private syncCropInputs(): void {
    const region = this.cropRegion;
    if (!region) return;
    this.els.cropX.value = String(Math.round(region.x));
    this.els.cropY.value = String(Math.round(region.y));
    this.els.cropW.value = String(Math.round(region.width));
    this.els.cropH.value = String(Math.round(region.height));
  }

  private handleCropPointerDown(event: PointerEvent): void {
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
  }

  private handleCropPointerMove(event: PointerEvent): void {
    if (!this.cropDrag || this.viewMode !== "source") return;
    const point = this.getCanvasPoint(event);
    if (!point) return;
    this.cropRegion = regionFromPoints(this.cropDrag, point, this.getVideoWidth(), this.getVideoHeight());
    this.syncCropInputs();
    this.renderCurrentView();
  }

  private handleCropPointerUp(event: PointerEvent): void {
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
  }

  private getCanvasPoint(event: PointerEvent): CropDragState | null {
    const width = this.getVideoWidth();
    const height = this.getVideoHeight();
    if (width <= 0 || height <= 0) return null;
    const rect = this.els.canvas.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return null;
    return {
      startX: clampNumber(((event.clientX - rect.left) / rect.width) * width, 0, width),
      startY: clampNumber(((event.clientY - rect.top) / rect.height) * height, 0, height),
    };
  }

  private setViewMode(mode: VideoSpriteView, render: boolean = true): void {
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
  }

  private renderCurrentView(): void {
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
  }

  private renderSourceView(): void {
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
  }

  private drawCropOverlay(ctx: CanvasRenderingContext2D, width: number, height: number): void {
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
  }

  private renderSheetView(): void {
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
  }

  private renderPlaybackFrame(): void {
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
        console.error("[video-sprite] 播放帧解码失败:", err);
        this.setStatus(`播放帧解码失败: ${String(err)}`);
      });
  }

  private async togglePlayback(): Promise<void> {
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
    }
    if (mode === "output" && this.processedFrames.length === 0) return;
    this.startPlayback(mode);
  }

  private async ensureSourcePreviewReadyForPlayback(): Promise<void> {
    this.clearSourcePlaybackFramesIfStale();
    if (!this.hasPreparedSourcePreview()) {
      await this.prepareSourcePreviewFrames();
    }
  }

  private startPlayback(mode: PlaybackMode): void {
    if (mode === "source" && !this.hasPreparedSourcePreview()) return;
    if (mode === "output" && this.processedFrames.length === 0) return;
    if (mode === "output") {
      this.setViewMode("playback", false);
    } else {
      this.setViewMode("source", false);
    }
    this.playbackMode = mode;
    this.isPlaying = true;
    this.lastPlaybackAt = performance.now();
    this.els.play.textContent = "暂停";
    this.syncControls();
    this.playbackHandle = window.requestAnimationFrame((time) => this.playbackTick(time));
  }

  private stopPlayback(): void {
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
  }

  private async startSourceVideoPlayback(): Promise<void> {
    if (!this.sourceVideoReady) return;
    this.setViewMode("source", false);
    const { start, end } = this.getSelectedTimeRange();
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
      this.stopPlayback();
      this.setStatus(`源视频播放失败: ${String(err)}`);
      await this.log(`direct video play failed | ${String(err)}`);
    }
  }

  private handleSourceVideoTimeUpdate(): void {
    if (!this.sourceVideoReady) return;
    const { start, end } = this.getSelectedTimeRange();
    const current = this.els.video.currentTime || 0;
    if (this.isPlaying && this.playbackMode === "source" && end > start && current >= end) {
      this.seekSourceVideo(start);
      void this.els.video.play().catch((err) => {
        this.stopPlayback();
        this.setStatus(`源视频循环播放失败: ${String(err)}`);
      });
      return;
    }
    this.els.timeScrub.value = String(clampNumber(current, 0, Math.max(this.getVideoDuration(), 0)));
    this.syncCurrentTimeLabel(current);
    this.syncPlaybackLabels();
  }

  private handleSourceVideoEnded(): void {
    if (!this.sourceVideoReady || this.playbackMode !== "source") return;
    const { start } = this.getSelectedTimeRange();
    this.seekSourceVideo(start);
    void this.els.video.play().catch(() => this.stopPlayback());
  }

  private seekSourceVideo(time: number): void {
    if (!this.sourceVideoReady) return;
    const clamped = clampNumber(time, 0, Math.max(this.getVideoDuration(), 0));
    if (Number.isFinite(clamped)) {
      try {
        this.els.video.currentTime = clamped;
      } catch (err) {
        console.warn("[video-sprite] 设置视频时间失败:", err);
      }
      this.els.timeScrub.value = String(clamped);
      this.syncCurrentTimeLabel(clamped);
    }
  }

  private playbackTick(time: number): void {
    if (!this.isPlaying) return;
    const fps = clampInt(this.els.fps.value, 12, 1, 60);
    const interval = 1000 / fps;
    if (time - this.lastPlaybackAt >= interval) {
      if (this.playbackMode === "source") {
        this.sourcePreviewIndex = this.getNextSourcePreviewIndex(1);
        this.syncTimeControls();
        this.renderSourceView();
      } else {
        this.currentFrameIndex = (this.currentFrameIndex + 1) % this.processedFrames.length;
        this.renderPlaybackFrame();
      }
      this.lastPlaybackAt = time;
    }
    this.playbackHandle = window.requestAnimationFrame((nextTime) => this.playbackTick(nextTime));
  }

  private getNextSourcePreviewIndex(delta: number): number {
    const indices = this.getSourcePreviewPlaybackIndices();
    if (indices.length === 0) return this.sourcePreviewIndex;
    const currentPos = indices.indexOf(this.sourcePreviewIndex);
    const basePos = currentPos >= 0 ? currentPos : 0;
    return indices[(basePos + delta + indices.length) % indices.length];
  }

  private getSourcePreviewPlaybackIndices(): number[] {
    if (this.sourcePreviewFrames.length === 0) return [];
    const { start, end } = this.getSelectedTimeRange();
    const epsilon = 0.05;
    const indices: number[] = [];
    for (let i = 0; i < this.sourcePreviewFrames.length; i += 1) {
      const time = this.sourcePreviewFrames[i].time;
      if (time >= start - epsilon && time <= end + epsilon) {
        indices.push(i);
      }
    }
    if (indices.length > 0) {
      return indices;
    }
    return [findNearestSourceFrameIndex(this.sourcePreviewFrames, start)];
  }

  private stepPlayback(delta: number): void {
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
      this.sourcePreviewIndex = this.getNextSourcePreviewIndex(delta);
      this.syncTimeControls();
      this.renderSourceView();
      return;
    }
    if (this.processedFrames.length === 0) return;
    this.currentFrameIndex =
      (this.currentFrameIndex + delta + this.processedFrames.length) % this.processedFrames.length;
    this.setViewMode("playback", false);
    this.renderPlaybackFrame();
  }

  private updateFrameCurrentState(): void {
    queryAll<HTMLElement>(".frame-thumb", this.els.frameList).forEach((item, index) => {
      item.classList.toggle("current", index === this.currentFrameIndex);
      item.setAttribute("aria-current", index === this.currentFrameIndex ? "true" : "false");
    });
  }

  private syncPlaybackLabels(): void {
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
      this.els.playbackInfo.textContent =
        `帧: ${this.processedFrames.length === 0 ? 0 : this.currentFrameIndex + 1}/${this.processedFrames.length}`;
    }
  }

  private syncThresholdLabel(): void {
    this.els.thresholdLabel.textContent = this.els.threshold.value;
  }

  private handlePlaybackFpsChanged(): void {
    this.syncPlaybackLabels();
    if (this.sourceVideoReady) {
      this.syncSourceVideoPlaybackRate();
    }
  }

  private syncSourceVideoPlaybackRate(): void {
    if (!this.sourceVideoReady) return;
    const fps = clampInt(this.els.fps.value, 12, 1, 60);
    this.els.video.playbackRate = clampNumber(fps / 24, 0.25, 4);
  }

  private syncControls(): void {
    const hasVideo = Boolean(this.sourcePath && this.getVideoWidth() > 0 && this.getVideoHeight() > 0);
    const hasOutput = this.processedFrames.length > 0 && Boolean(this.spriteSheetBlob);
    const hasSourcePlayback = this.hasPreparedSourcePreview();
    const hasSourceStep = this.sourceVideoReady || hasSourcePlayback;
    const controlsSourcePlayback = this.viewMode === "source";
    const busy = this.isBusy();
    const canStartSourcePlayback = hasVideo && !busy && !this.isPreparingSourcePreview;
    const hasActivePlayback = controlsSourcePlayback ? canStartSourcePlayback : hasOutput;
    setButtonState(this.els.pickVideo, { disabled: busy });
    setButtonState(this.els.generateVideo, {
      disabled: busy,
      loading: this.isGeneratingVideo,
      text: this.isGeneratingVideo ? "生成中" : "AI 生成",
    });
    setButtonState(this.els.extract, {
      disabled: busy || !hasVideo,
      loading: this.isExtracting,
      text: this.isExtracting ? "生成中" : "生成序列帧图",
    });
    setButtonState(this.els.save, { disabled: busy || !hasOutput });
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
    [this.els.videoPrompt, this.els.videoSize, this.els.videoSeconds].forEach((input) => {
      input.disabled = busy;
    });
    this.syncPlaybackLabels();
  }

  private isBusy(): boolean {
    return this.isExtracting || this.isGeneratingVideo;
  }

  private showBusy(show: boolean, text: string = "正在生成..."): void {
    setBusyState(this.els.busy, this.els.busyText, show, text);
  }

  private async cleanupStartupTempFiles(): Promise<void> {
    try {
      const result = await cleanupVideoSpriteTempFiles();
      if (result.removed_files > 0 || result.removed_dirs > 0) {
        await this.log(
          `temp cleanup startup ok | files=${result.removed_files} dirs=${result.removed_dirs}`
        );
      }
    } catch (err) {
      await this.log(`temp cleanup startup failed | ${String(err)}`);
    }
  }

  private async cleanupVideoFrameOutputDir(outputDir: string, context: string): Promise<void> {
    if (!outputDir) return;
    try {
      const result = await cleanupVideoFrameBatchDir(outputDir);
      await this.log(
        `temp cleanup video frames ok | context=${context} dir=${outputDir} files=${result.removed_files} dirs=${result.removed_dirs}`
      );
    } catch (err) {
      await this.log(
        `temp cleanup video frames failed | context=${context} dir=${outputDir} error=${String(err)}`
      );
    }
  }

  private async cleanupPreparedVideoPath(path: string, context: string): Promise<void> {
    if (!path) return;
    try {
      const result = await cleanupPreparedVideoFile(path);
      await this.log(
        `temp cleanup prepared video ok | context=${context} path=${path} files=${result.removed_files}`
      );
    } catch (err) {
      await this.log(
        `temp cleanup prepared video failed | context=${context} path=${path} error=${String(err)}`
      );
    }
  }

  private async releasePreparedVideoFile(context: string): Promise<void> {
    const path = this.preparedVideoPath;
    this.preparedVideoPath = "";
    await this.cleanupPreparedVideoPath(path, context);
  }

  private clearGeneratedOutput(): void {
    this.stopPlayback();
    this.processedFrames.forEach((frame) => {
      URL.revokeObjectURL(frame.url);
      frame.bitmap?.close();
    });
    this.processedFrames = [];
    if (this.spriteSheetBitmap) {
      this.spriteSheetBitmap.close();
    }
    this.spriteSheetBitmap = null;
    this.spriteSheetBlob = null;
    this.currentFrameIndex = 0;
    this.els.frameTotal.textContent = "0 帧";
    this.els.frameList.innerHTML = '<div class="placeholder-text">选择视频后生成序列帧图</div>';
    this.syncPlaybackLabels();
  }

  private clearSourcePreview(): void {
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
    if (this.sourcePreviewBitmap) {
      this.sourcePreviewBitmap.close();
      this.sourcePreviewBitmap = null;
    }
    this.syncTimeControls();
  }

  private clearSourcePlaybackFrames(): void {
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
  }

  private clearSourcePlaybackFramesIfStale(): boolean {
    if (!this.sourcePreviewSignature && !this.isPreparingSourcePreview) return false;
    const currentRequest = this.getSourcePreviewRequest();
    if (this.hasPreparedSourcePreview(currentRequest)) return false;
    void this.log(
      `source preview cache stale | old=${this.sourcePreviewSignature || "(preparing)"} new=${currentRequest.signature}`
    );
    this.clearSourcePlaybackFrames();
    return true;
  }

  private clearSourceVideo(): void {
    this.sourceVideoReady = false;
    this.els.video.pause();
    this.els.video.removeAttribute("src");
    this.els.video.load();
    this.updateSourceVideoVisibility(false);
  }

  private setSourcePreviewBitmap(bitmap: ImageBitmap): void {
    this.clearSourcePreview();
    this.sourcePreviewBitmap = bitmap;
  }

  private getSourceDisplayBitmap(): ImageBitmap | null {
    return this.sourcePreviewFrames[this.sourcePreviewIndex]?.bitmap || this.sourcePreviewBitmap;
  }

  private getSourceCurrentTime(): number {
    if (this.sourceVideoReady) {
      return this.els.video.currentTime || 0;
    }
    return this.sourcePreviewFrames[this.sourcePreviewIndex]?.time ||
      parseInputNumber(this.els.timeScrub.value, parseInputNumber(this.els.start.value, 0));
  }

  private syncTimeControls(): void {
    const { start, end, max } = this.readTimeRangeInputs();
    const current = clampNumber(this.getSourceCurrentTime(), 0, max);

    [this.els.timeScrub, this.els.startRange, this.els.endRange].forEach((range) => {
      range.min = "0";
      range.max = String(max);
      range.step = "0.01";
    });
    this.els.startRange.value = String(start);
    this.els.endRange.value = String(end);
    this.els.timeScrub.value = String(current);
    this.syncCurrentTimeLabel(current);
  }

  private syncCurrentTimeLabel(time: number): void {
    this.els.currentTime.textContent =
      `当前: ${formatSeconds(time)} / ${formatSeconds(this.getVideoDuration())}`;
  }

  private hasSourcePreview(): boolean {
    return Boolean(this.sourceVideoReady || this.sourcePreviewBitmap || this.sourcePreviewFrames.length > 0);
  }

  private updateSourceVideoVisibility(forceVisible?: boolean): void {
    const visible = forceVisible ?? (this.viewMode === "source" && this.sourceVideoReady);
    const previewArea = this.els.canvas.closest(".preview-area");
    previewArea?.classList.toggle("uses-video", visible);
    this.els.video.style.display = visible ? "block" : "none";
  }

  private clearCanvas(message: string): void {
    this.updateSourceVideoVisibility(false);
    clearPreviewCanvas(this.previewCanvasTarget(), message);
  }

  private previewCanvasTarget() {
    return {
      canvas: this.els.canvas,
      placeholder: this.els.placeholder,
      sizeLabel: this.els.size,
    };
  }

  private setStatus(text: string): void {
    this.els.status.textContent = text;
  }

  private async log(message: string): Promise<void> {
    try {
      await logVideoSpriteMessage(message);
    } catch (err) {
      console.warn("[video-sprite] 写入视频日志失败:", err);
    }
  }

  private getVideoDuration(): number {
    return this.videoMeta?.duration_seconds || 0;
  }

  private getVideoWidth(): number {
    return this.videoMeta?.width || this.sourcePreviewBitmap?.width || 0;
  }

  private getVideoHeight(): number {
    return this.videoMeta?.height || this.sourcePreviewBitmap?.height || 0;
  }
}

async function loadBitmapFromPath(path: string): Promise<ImageBitmap> {
  const base64 = await readFileAsBase64(path);
  return loadBitmapFromBase64(base64);
}

function toVideoExtractRegion(region: PixelBounds): VideoExtractRegion {
  return {
    x: Math.round(region.x),
    y: Math.round(region.y),
    width: Math.round(region.width),
    height: Math.round(region.height),
  };
}

function getBackendMaxExtractEdge(options: ExtractionOptions): number {
  if (!options.autoTrim && options.bgMode === "none") {
    return options.maxFrameEdge;
  }
  const qualityEdge = Math.max(options.maxFrameEdge * 2, options.maxFrameEdge + options.padding * 4);
  return clampNumber(Math.round(qualityEdge), options.maxFrameEdge, 1536);
}

function getSourcePreviewFrameCount(duration: number, previewFps: number, maxFrames: number): number {
  const safeFps = clampNumber(Math.round(previewFps), 1, 30);
  const safeMaxFrames = clampNumber(Math.round(maxFrames), 2, 240);
  if (!Number.isFinite(duration) || duration <= 0) {
    return clampNumber(safeFps, 2, safeMaxFrames);
  }
  return clampNumber(Math.ceil(duration * safeFps), 2, safeMaxFrames);
}

function findNearestSourceFrameIndex(frames: SourcePreviewFrame[], time: number): number {
  if (frames.length === 0) return 0;
  let bestIndex = 0;
  let bestDistance = Number.POSITIVE_INFINITY;
  for (let i = 0; i < frames.length; i += 1) {
    const distance = Math.abs(frames[i].time - time);
    if (distance < bestDistance) {
      bestDistance = distance;
      bestIndex = i;
    }
  }
  return bestIndex;
}

function closeSourcePreviewFrames(frames: SourcePreviewFrame[]): void {
  frames.forEach((frame) => frame.bitmap.close());
}

function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result || ""));
    reader.onerror = () => reject(reader.error || new Error("读取 PNG 数据失败"));
    reader.readAsDataURL(blob);
  });
}

function regionFromPoints(
  start: CropDragState,
  end: CropDragState,
  width: number,
  height: number
): PixelBounds {
  const left = clampNumber(Math.min(start.startX, end.startX), 0, width);
  const top = clampNumber(Math.min(start.startY, end.startY), 0, height);
  const right = clampNumber(Math.max(start.startX, end.startX), left + 1, width);
  const bottom = clampNumber(Math.max(start.startY, end.startY), top + 1, height);
  return {
    x: Math.round(left),
    y: Math.round(top),
    width: Math.max(1, Math.round(right - left)),
    height: Math.max(1, Math.round(bottom - top)),
  };
}

function waitForVideoReady(video: HTMLVideoElement, timeoutMs: number): Promise<void> {
  return new Promise((resolve, reject) => {
    let settled = false;
    const cleanup = () => {
      window.clearTimeout(timer);
      video.removeEventListener("loadeddata", handleReady);
      video.removeEventListener("canplay", handleReady);
      video.removeEventListener("error", handleError);
    };
    const finish = (callback: () => void) => {
      if (settled) return;
      settled = true;
      cleanup();
      callback();
    };
    const handleReady = () => finish(resolve);
    const handleError = () => {
      finish(() => reject(new Error("视频格式或编码不受当前 WebView 支持")));
    };
    const timer = window.setTimeout(() => {
      finish(() => reject(new Error("视频加载超时")));
    }, timeoutMs);

    video.addEventListener("loadeddata", handleReady, { once: true });
    video.addEventListener("canplay", handleReady, { once: true });
    video.addEventListener("error", handleError, { once: true });
    video.load();
  });
}

function getMediaErrorMessage(video: HTMLVideoElement, err: unknown): string {
  const mediaError = video.error;
  if (!mediaError) {
    return err instanceof Error ? err.message : String(err);
  }
  const codeText = (() => {
    switch (mediaError.code) {
      case MediaError.MEDIA_ERR_ABORTED:
        return "加载被中止";
      case MediaError.MEDIA_ERR_NETWORK:
        return "网络或文件读取失败";
      case MediaError.MEDIA_ERR_DECODE:
        return "视频解码失败";
      case MediaError.MEDIA_ERR_SRC_NOT_SUPPORTED:
        return "视频格式或编码不受当前 WebView 支持";
      default:
        return "视频加载失败";
    }
  })();
  return mediaError.message ? `${codeText}: ${mediaError.message}` : codeText;
}

function clampNumber(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function getOutputFileName(sourceName: string): string {
  const stem = stripExtension(sourceName || "video");
  return `${stem}_sprite_sheet.png`;
}
