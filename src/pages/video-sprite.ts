import { Channel, convertFileSrc } from "@tauri-apps/api/core";
import {
  cleanupPreparedVideoFile,
  cleanupVideoFrameBatchDir,
  cleanupVideoSpriteTempFiles,
  extractVideoFramesWithFfmpeg,
  logVideoSpriteMessage,
  openVideoFile,
  prepareVideoFileForPlayback,
  probeVideoFile,
  readFileAsBase64,
  saveSpriteSheetDataUrl,
  type SavedImageResult,
  type VideoExtractEvent,
  type VideoExtractRegion,
  type VideoProbeResult,
} from "../api/commands";
import type { GeneratorPage } from "./generator";

type BackgroundMode = "edge" | "firstFrame" | "none";
type VideoSpriteView = "source" | "sheet" | "playback";
type PlaybackMode = "source" | "output";

interface PixelBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

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

interface WorkerFrameInput {
  base64: string;
  time: number;
}

interface ExtractedFrameBatch {
  frames: WorkerFrameInput[];
  cropRegion: PixelBounds;
}

interface WorkerFrameResult {
  blob: Blob;
  time: number;
  width: number;
  height: number;
}

interface WorkerProgressMessage {
  id: number;
  type: "progress";
  done: number;
  total: number;
  message: string;
}

interface WorkerSuccessMessage {
  id: number;
  type: "success";
  frames: WorkerFrameResult[];
  sheetBlob: Blob;
  sheetWidth: number;
  sheetHeight: number;
  cellWidth: number;
  cellHeight: number;
}

interface WorkerErrorMessage {
  id: number;
  type: "error";
  error: string;
}

type WorkerMessage = WorkerProgressMessage | WorkerSuccessMessage | WorkerErrorMessage;

interface CropDragState {
  startX: number;
  startY: number;
}

interface VideoSpriteElements {
  pickVideo: HTMLButtonElement;
  extract: HTMLButtonElement;
  save: HTMLButtonElement;
  reference: HTMLButtonElement;
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
    const g = (id: string) => document.getElementById(id) as HTMLElement;
    return {
      pickVideo: g("btn-video-sprite-pick") as HTMLButtonElement,
      extract: g("btn-video-sprite-extract") as HTMLButtonElement,
      save: g("btn-video-sprite-save") as HTMLButtonElement,
      reference: g("btn-video-sprite-reference") as HTMLButtonElement,
      video: g("video-sprite-video") as HTMLVideoElement,
      canvas: g("video-sprite-canvas") as HTMLCanvasElement,
      placeholder: g("video-sprite-placeholder"),
      busy: g("video-sprite-busy"),
      busyText: g("video-sprite-busy-text"),
      file: g("video-sprite-file"),
      duration: g("video-sprite-duration"),
      status: g("video-sprite-status"),
      size: g("video-sprite-size"),
      frameTotal: g("video-sprite-frame-total"),
      frameList: g("video-sprite-frame-list"),
      frameCount: g("video-sprite-frame-count") as HTMLInputElement,
      cols: g("video-sprite-cols") as HTMLInputElement,
      start: g("video-sprite-start") as HTMLInputElement,
      end: g("video-sprite-end") as HTMLInputElement,
      frameEdge: g("video-sprite-frame-edge") as HTMLInputElement,
      padding: g("video-sprite-padding") as HTMLInputElement,
      cropFull: g("btn-video-sprite-crop-full") as HTMLButtonElement,
      cropX: g("video-sprite-crop-x") as HTMLInputElement,
      cropY: g("video-sprite-crop-y") as HTMLInputElement,
      cropW: g("video-sprite-crop-w") as HTMLInputElement,
      cropH: g("video-sprite-crop-h") as HTMLInputElement,
      bgMode: g("video-sprite-bg-mode") as HTMLSelectElement,
      threshold: g("video-sprite-threshold") as HTMLInputElement,
      thresholdLabel: g("video-sprite-threshold-label"),
      autoTrim: g("video-sprite-auto-trim") as HTMLInputElement,
      transparent: g("video-sprite-transparent") as HTMLInputElement,
      viewSource: g("btn-video-sprite-view-source") as HTMLButtonElement,
      viewSheet: g("btn-video-sprite-view-sheet") as HTMLButtonElement,
      viewPlayback: g("btn-video-sprite-view-playback") as HTMLButtonElement,
      timeScrub: g("video-sprite-time-scrub") as HTMLInputElement,
      startRange: g("video-sprite-start-range") as HTMLInputElement,
      endRange: g("video-sprite-end-range") as HTMLInputElement,
      currentTime: g("video-sprite-current-time"),
      setStart: g("btn-video-sprite-set-start") as HTMLButtonElement,
      setEnd: g("btn-video-sprite-set-end") as HTMLButtonElement,
      prev: g("btn-video-sprite-prev") as HTMLButtonElement,
      play: g("btn-video-sprite-play") as HTMLButtonElement,
      next: g("btn-video-sprite-next") as HTMLButtonElement,
      fps: g("video-sprite-fps") as HTMLInputElement,
      fpsLabel: g("video-sprite-fps-label"),
      playbackInfo: g("video-sprite-playback-info"),
    };
  }

  private bindEvents(): void {
    this.els.pickVideo.addEventListener("click", () => this.handlePickVideo());
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
    this.els.fps.addEventListener("input", () => this.syncPlaybackLabels());
    this.els.video.addEventListener("timeupdate", () => this.handleSourceVideoTimeUpdate());
    this.els.video.addEventListener("ended", () => this.handleSourceVideoEnded());
    this.els.canvas.addEventListener("pointerdown", (event) => this.handleCropPointerDown(event));
    this.els.canvas.addEventListener("pointermove", (event) => this.handleCropPointerMove(event));
    this.els.canvas.addEventListener("pointerup", (event) => this.handleCropPointerUp(event));
    this.els.canvas.addEventListener("pointercancel", (event) => this.handleCropPointerUp(event));
  }

  private async handlePickVideo(): Promise<void> {
    if (this.isExtracting) return;

    try {
      const file = await openVideoFile();
      this.stopPlayback();
      this.clearSourceVideo();
      await this.releasePreparedVideoFile("replace source");
      this.sourcePath = file.file_path;
      this.sourceName = file.file_name || getFileName(file.file_path) || "animation";
      this.videoMeta = null;
      this.savedResult = null;
      this.cropRegion = null;
      this.clearGeneratedOutput();
      this.clearSourcePreview();
      this.setViewMode("source", false);
      this.showBusy(false);

      this.setStatus("正在用 ffmpeg 读取视频...");
      await nextFrame();
      await this.log(`pick video | ffmpeg probe | path=${file.file_path}`);
      this.videoMeta = await probeVideoFile(file.file_path);

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
    } catch (err) {
      if (!String(err).includes("用户取消")) {
        console.error("[video-sprite] 选择视频失败:", err);
        await this.log(`pick video failed | ${String(err)}`);
        this.setStatus(`选择视频失败: ${String(err)}`);
      }
    }
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
    if (this.hasPreparedSourcePreview(request.signature)) {
      await this.log(`source preview reuse | signature=${request.signature}`);
      this.sourcePreviewIndex = findNearestSourceFrameIndex(this.sourcePreviewFrames, request.start);
      this.syncTimeControls();
      this.renderCurrentView();
      return;
    }
    if (this.sourcePreviewPreparePromise) {
      await this.log(`source preview await existing prepare | signature=${request.signature}`);
      await this.sourcePreviewPreparePromise;
      if (this.hasPreparedSourcePreview(request.signature)) {
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

  private async prepareSourcePreviewFramesForRequest(request: {
    start: number;
    end: number;
    frameCount: number;
    signature: string;
  }): Promise<void> {
    const seq = ++this.sourcePreviewSeq;
    this.isPreparingSourcePreview = true;
    this.syncControls();
    const { start, end, frameCount, signature } = request;
    await this.log(`source preview prepare start | signature=${signature}`);
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
    const duration = this.getVideoDuration();
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, Math.max(duration, 0));
    const end = clampNumber(parseInputNumber(this.els.end.value, duration), start, Math.max(duration, start));
    this.els.start.value = formatInputNumber(start);
    this.els.end.value = formatInputNumber(end);
    this.clearSourcePlaybackFramesIfStale();
    this.syncTimeControls();
    if (refreshPreview) {
      await this.showSourceFrameAtTime(start, true).catch((err) => this.handleSourcePreviewError(err));
    }
  }

  private handleEndTimeChanged(): void {
    const duration = this.getVideoDuration();
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, Math.max(duration, 0));
    const end = clampNumber(parseInputNumber(this.els.end.value, duration), start, Math.max(duration, start));
    this.els.start.value = formatInputNumber(start);
    this.els.end.value = formatInputNumber(end);
    this.clearSourcePlaybackFramesIfStale();
    this.syncTimeControls();
  }

  private handleSourceScrubInput(): void {
    void this.showSourceFrameAtTime(parseInputNumber(this.els.timeScrub.value, 0), false);
  }

  private async handleStartRangeInput(): Promise<void> {
    const duration = this.getVideoDuration();
    const start = clampNumber(parseInputNumber(this.els.startRange.value, 0), 0, Math.max(duration, 0));
    const end = Math.max(start, parseInputNumber(this.els.end.value, duration));
    this.els.start.value = formatInputNumber(start);
    this.els.end.value = formatInputNumber(end);
    this.clearSourcePlaybackFramesIfStale();
    this.syncTimeControls();
    await this.showSourceFrameAtTime(start, false);
  }

  private handleEndRangeInput(): void {
    const duration = this.getVideoDuration();
    const start = parseInputNumber(this.els.start.value, 0);
    const end = clampNumber(parseInputNumber(this.els.endRange.value, duration), start, Math.max(duration, start));
    this.els.end.value = formatInputNumber(end);
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
    if (this.isExtracting || !this.sourcePath || !this.cropRegion) return;

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
        `generate start | path=${this.sourcePath} crop=${formatRegion(options.cropRegion)} start=${options.start.toFixed(3)} end=${options.end.toFixed(3)} frames=${options.frameCount}`
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
      worker.postMessage({
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
      });
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
    this.els.frameList.innerHTML = "";
    if (this.processedFrames.length === 0) {
      this.els.frameList.innerHTML = '<div class="placeholder-text">选择视频后生成序列帧图</div>';
      return;
    }

    this.processedFrames.forEach((frame, index) => {
      const item = document.createElement("div");
      item.className = "frame-thumb video-frame-thumb";
      item.classList.toggle("current", index === this.currentFrameIndex);
      item.addEventListener("click", () => {
        this.currentFrameIndex = index;
        this.setViewMode("playback", false);
        this.renderCurrentView();
        this.syncPlaybackLabels();
        this.updateFrameCurrentState();
      });

      const img = document.createElement("img");
      img.src = frame.url;
      img.alt = `抽取帧 ${index + 1}`;

      const label = document.createElement("span");
      label.className = "frame-index";
      label.textContent = `${index + 1} · ${formatSeconds(frame.time)}`;

      item.appendChild(img);
      item.appendChild(label);
      this.els.frameList.appendChild(item);
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
    const generatorTab = document.querySelector<HTMLButtonElement>(
      '.tab-button[data-tab="generator"]'
    );
    generatorTab?.click();
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
    const duration = this.getVideoDuration();
    const max = Math.max(duration, 0);
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, max);
    const end = clampNumber(parseInputNumber(this.els.end.value, max), start, Math.max(max, start));
    return { start, end };
  }

  private getSourcePreviewRequest(): {
    start: number;
    end: number;
    frameCount: number;
    signature: string;
  } {
    const { start, end } = this.getSelectedTimeRange();
    const frameCount = getSourcePreviewFrameCount(Math.max(0, end - start));
    const signature = [
      this.sourcePath,
      start.toFixed(3),
      end.toFixed(3),
      frameCount,
    ].join("|");
    return { start, end, frameCount, signature };
  }

  private hasPreparedSourcePreview(signature: string = this.getSourcePreviewRequest().signature): boolean {
    if (this.sourcePreviewFrames.length === 0) return false;
    if (this.sourcePreviewSignature === signature) return true;
    const { start, end } = this.getSelectedTimeRange();
    const epsilon = 0.05;
    return start >= this.sourcePreviewCacheStart - epsilon &&
      end <= this.sourcePreviewCacheEnd + epsilon;
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
    return clampRegion(this.cropRegion || full, full.width, full.height);
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
    if (this.isExtracting || this.viewMode !== "source") return;
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
    this.els.canvas.releasePointerCapture(event.pointerId);
    this.cropDrag = null;
    this.handleCropPointerMove(event);
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
    const canvas = this.els.canvas;
    const previewSize = fitPreviewCanvasSize(width, height, 1280);
    canvas.width = previewSize.width;
    canvas.height = previewSize.height;
    canvas.style.aspectRatio = `${width} / ${height}`;
    this.els.video.style.aspectRatio = `${width} / ${height}`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.save();
    ctx.scale(canvas.width / width, canvas.height / height);
    const sourceBitmap = this.sourceVideoReady ? null : this.getSourceDisplayBitmap();
    if (sourceBitmap) {
      ctx.drawImage(sourceBitmap, 0, 0, width, height);
    }
    this.drawCropOverlay(ctx, width, height);
    ctx.restore();
    this.els.placeholder.style.display = "none";
    this.els.size.textContent = `源尺寸: ${width} x ${height}`;
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
    const canvas = this.els.canvas;
    canvas.width = this.spriteSheetBitmap.width;
    canvas.height = this.spriteSheetBitmap.height;
    canvas.style.aspectRatio = `${canvas.width} / ${canvas.height}`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.drawImage(this.spriteSheetBitmap, 0, 0);
    this.els.placeholder.style.display = "none";
    this.els.size.textContent = `尺寸: ${canvas.width} x ${canvas.height}`;
  }

  private renderPlaybackFrame(): void {
    this.updateSourceVideoVisibility(false);
    if (this.processedFrames.length === 0) {
      this.clearCanvas("生成后播放预览");
      return;
    }
    const frame = this.processedFrames[this.currentFrameIndex % this.processedFrames.length];
    const seq = ++this.playbackRenderSeq;
    const canvas = this.els.canvas;
    canvas.width = frame.width;
    canvas.height = frame.height;
    canvas.style.aspectRatio = `${canvas.width} / ${canvas.height}`;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    this.els.placeholder.style.display = "none";
    this.els.size.textContent = `帧尺寸: ${frame.width} x ${frame.height}`;
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
    if (mode === "source" && !this.hasPreparedSourcePreview()) {
      await this.prepareSourcePreviewFrames();
      if (!this.hasPreparedSourcePreview()) return;
    }
    if (mode === "output" && this.processedFrames.length === 0) return;
    this.startPlayback(mode);
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
    const interval = this.playbackMode === "source"
      ? this.getSourcePlaybackInterval()
      : 1000 / fps;
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

  private getSourcePlaybackInterval(): number {
    const indices = this.getSourcePreviewPlaybackIndices();
    if (indices.length < 2) {
      return 250;
    }
    const currentIndex = indices.includes(this.sourcePreviewIndex) ? this.sourcePreviewIndex : indices[0];
    const currentPos = indices.indexOf(currentIndex);
    const nextIndex = indices[(currentPos + 1) % indices.length];
    const current = this.sourcePreviewFrames[currentIndex];
    const next = this.sourcePreviewFrames[nextIndex];
    const { start, end } = this.getSelectedTimeRange();
    const loopDelta = Math.max(0.05, end - current.time + next.time - start);
    const delta = next.time > current.time ? next.time - current.time : loopDelta;
    return clampNumber(delta * 1000, 33, 1000);
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
    this.els.frameList.querySelectorAll<HTMLElement>(".frame-thumb").forEach((item, index) => {
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

  private syncControls(): void {
    const hasVideo = Boolean(this.sourcePath && this.getVideoWidth() > 0 && this.getVideoHeight() > 0);
    const hasOutput = this.processedFrames.length > 0 && Boolean(this.spriteSheetBlob);
    const hasSourcePlayback = this.hasPreparedSourcePreview();
    const hasSourceStep = this.sourceVideoReady || hasSourcePlayback;
    const controlsSourcePlayback = this.viewMode === "source";
    const canStartSourcePlayback = hasVideo && !this.isExtracting && !this.isPreparingSourcePreview;
    const hasActivePlayback = controlsSourcePlayback ? canStartSourcePlayback : hasOutput;
    this.els.pickVideo.disabled = this.isExtracting;
    this.els.extract.disabled = this.isExtracting || !hasVideo;
    this.els.save.disabled = this.isExtracting || !hasOutput;
    this.els.reference.disabled = this.isExtracting || !hasOutput;
    this.els.viewSheet.disabled = !hasOutput;
    this.els.viewPlayback.disabled = !hasOutput;
    this.els.prev.disabled = controlsSourcePlayback ? !hasSourceStep : !hasOutput;
    this.els.play.disabled = !hasActivePlayback;
    this.els.next.disabled = controlsSourcePlayback ? !hasSourceStep : !hasOutput;
    this.els.setStart.disabled = this.isExtracting || !hasVideo;
    this.els.setEnd.disabled = this.isExtracting || !hasVideo;
    this.els.timeScrub.disabled = this.isExtracting || !hasVideo;
    this.els.startRange.disabled = this.isExtracting || !hasVideo;
    this.els.endRange.disabled = this.isExtracting || !hasVideo;
    this.els.play.textContent = this.isPlaying ? "暂停" : "播放";
    this.els.extract.classList.toggle("is-loading", this.isExtracting);
    this.els.extract.textContent = this.isExtracting ? "生成中" : "生成序列帧图";
    [this.els.cropX, this.els.cropY, this.els.cropW, this.els.cropH].forEach((input) => {
      input.disabled = this.isExtracting || !hasVideo;
    });
    this.syncPlaybackLabels();
  }

  private showBusy(show: boolean, text: string = "正在生成..."): void {
    this.els.busy.hidden = !show;
    this.els.busyText.textContent = text;
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
    this.sourcePreviewCacheStart = 0;
    this.sourcePreviewCacheEnd = 0;
    this.sourcePreviewIndex = 0;
    this.showBusy(false);
    this.stopPlayback();
    this.syncTimeControls();
    this.syncControls();
  }

  private clearSourcePlaybackFramesIfStale(): void {
    if (!this.sourcePreviewSignature && !this.isPreparingSourcePreview) return;
    const currentSignature = this.getSourcePreviewRequest().signature;
    if (this.hasPreparedSourcePreview(currentSignature)) return;
    void this.log(
      `source preview cache stale | old=${this.sourcePreviewSignature || "(preparing)"} new=${currentSignature}`
    );
    this.clearSourcePlaybackFrames();
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
    const duration = this.getVideoDuration();
    const max = Math.max(duration, 0);
    const start = clampNumber(parseInputNumber(this.els.start.value, 0), 0, max);
    const end = clampNumber(parseInputNumber(this.els.end.value, max), start, max);
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
    this.els.canvas.width = 1;
    this.els.canvas.height = 1;
    this.els.canvas.style.aspectRatio = "";
    const ctx = this.els.canvas.getContext("2d");
    ctx?.clearRect(0, 0, 1, 1);
    this.els.canvas.closest(".preview-area")?.classList.remove("has-image");
    this.els.placeholder.textContent = message;
    this.els.placeholder.style.display = "";
    this.els.size.textContent = "尺寸: -";
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
  const response = await fetch(`data:image/png;base64,${base64}`);
  const blob = await response.blob();
  return createImageBitmap(blob);
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

function getSourcePreviewFrameCount(duration: number): number {
  if (!Number.isFinite(duration) || duration <= 0) {
    return 24;
  }
  return clampNumber(Math.ceil(duration * 6), 24, 72);
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

async function mapWithConcurrency<T, R>(
  items: readonly T[],
  concurrency: number,
  mapper: (item: T, index: number) => Promise<R>
): Promise<R[]> {
  const limit = Math.max(1, Math.min(concurrency, items.length || 1));
  const results = new Array<R>(items.length);
  let nextIndex = 0;

  async function worker(): Promise<void> {
    while (true) {
      const index = nextIndex;
      nextIndex += 1;
      if (index >= items.length) return;
      results[index] = await mapper(items[index], index);
    }
  }

  await Promise.all(Array.from({ length: limit }, () => worker()));
  return results;
}

function fitPreviewCanvasSize(width: number, height: number, maxEdge: number): { width: number; height: number } {
  const edge = Math.max(width, height);
  if (edge <= maxEdge) {
    return {
      width: Math.max(1, Math.round(width)),
      height: Math.max(1, Math.round(height)),
    };
  }
  const scale = maxEdge / edge;
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
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

function clampRegion(region: PixelBounds, width: number, height: number): PixelBounds {
  const regionWidth = clampNumber(Math.round(region.width), 1, width);
  const regionHeight = clampNumber(Math.round(region.height), 1, height);
  return {
    x: clampNumber(Math.round(region.x), 0, Math.max(0, width - regionWidth)),
    y: clampNumber(Math.round(region.y), 0, Math.max(0, height - regionHeight)),
    width: regionWidth,
    height: regionHeight,
  };
}

function nextFrame(): Promise<void> {
  return new Promise((resolve) => window.requestAnimationFrame(() => resolve()));
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

function parseBackgroundMode(value: string): BackgroundMode {
  if (value === "firstFrame" || value === "none") {
    return value;
  }
  return "edge";
}

function clampInt(value: string, fallback: number, min: number, max: number): number {
  const parsed = Number.parseInt(value, 10);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(max, Math.max(min, parsed));
}

function parseInputNumber(value: string, fallback: number): number {
  const parsed = Number.parseFloat(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function clampNumber(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

function formatSeconds(value: number): string {
  if (!Number.isFinite(value)) {
    return "-";
  }
  return `${value.toFixed(2)}s`;
}

function formatInputNumber(value: number): string {
  return Number.isFinite(value) ? value.toFixed(2) : "0";
}

function getOutputFileName(sourceName: string): string {
  const stem = stripExtension(sourceName || "video");
  return `${stem}_sprite_sheet.png`;
}

function getFileName(path: string): string {
  return path.split(/[\\/]/).pop() || "";
}

function stripExtension(fileName: string): string {
  return fileName.replace(/\.[^/.]+$/, "");
}

function formatRegion(region: PixelBounds): string {
  return `${Math.round(region.x)},${Math.round(region.y)},${Math.round(region.width)}x${Math.round(region.height)}`;
}
