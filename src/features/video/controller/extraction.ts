import { Channel } from "@tauri-apps/api/core";
import {
  extractVideoFramesWithFfmpeg,
  readFileAsBase64,
  saveSpriteSheetDataUrl,
  type SavedImageResult,
  type VideoExtractEvent
} from "../../../api/commands";
import { mapWithConcurrency, nextFrame } from "../../../utils/async";
import { getErrorMessage } from "../../../utils/errors";
import {
  formatSeconds
} from "../../../utils/number";
import { startImageTaskWithReference } from "../../../workflows/task-coordinator";
import { renderVideoFrameList } from "../frame-list";
import type {
  VideoSpriteWorkerFrameInput as WorkerFrameInput,
  VideoSpriteWorkerMessage as WorkerMessage,
  VideoSpriteWorkerRequest as WorkerRequest,
  VideoSpriteWorkerSuccessMessage as WorkerSuccessMessage
} from "../types";
import {
  formatPixelBounds
} from "../utils";

import type { ExtractedFrameBatch, ExtractionOptions, VideoSpritePage } from "../video-page";
import { blobToDataUrl, getBackendMaxExtractEdge, getOutputFileName, toVideoExtractRegion } from "./helpers";

export const videoSpriteExtractionMethods = {
  async handleExtract(): Promise<void> {
    if (this.isBusy() || !this.sourcePath || !this.cropRegion) return;

    this.stopPlayback();
    let options: ExtractionOptions;
    try {
      options = this.readOptions();
    } catch (err) {
      await this.handleInputValidationError("抽帧参数无效", err);
      return;
    }
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
      const message = getErrorMessage(err);
      console.error("[video-sprite] 抽帧失败:", err);
      await this.log(`generate failed | ${message}`);
      this.setStatus(`抽帧失败: ${message}；详见 logs/video-sprite.log`);
    } finally {
      this.isExtracting = false;
      this.showBusy(false);
      this.syncControls();
      this.renderCurrentView();
    }
  },

  async extractFrameInputs(options: ExtractionOptions): Promise<ExtractedFrameBatch> {
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
          width: firstFrame.width,
          height: firstFrame.height,
        },
      };
    } finally {
      await this.cleanupVideoFrameOutputDir(outputDir, "final frame extraction");
    }
  },

  processFramesInWorker(
    frames: WorkerFrameInput[],
    options: ExtractionOptions
  ): Promise<WorkerSuccessMessage> {
    const id = ++this.workerSeq;
    const worker = new Worker(new URL("../worker.ts", import.meta.url), {
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
  },

  async applyWorkerResult(result: WorkerSuccessMessage): Promise<void> {
    this.clearGeneratedOutput();
    this.spriteSheetBlob = result.sheetBlob;
    this.spriteSheetBitmap = await createImageBitmap(result.sheetBlob);
    this.els.size.textContent =
      `尺寸: ${this.spriteSheetBitmap.width} x ${this.spriteSheetBitmap.height}`;
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
    this.processedFramesOrigin = "source";
    this.currentFrameIndex = 0;
    const firstFrame = this.processedFrames[0];
    const lastFrame = this.processedFrames[this.processedFrames.length - 1];
    if (firstFrame && lastFrame && lastFrame.time > firstFrame.time) {
      this.setTimeRangeInputs(firstFrame.time, lastFrame.time);
    }
    this.syncTimelineToCurrentOutputFrame();
    this.renderFrameList();
    this.syncPlaybackLabels();
  },

  renderFrameList(): void {
    renderVideoFrameList({
      container: this.els.frameList,
      frames: this.processedFrames,
      currentIndex: this.currentFrameIndex,
      formatTime: formatSeconds,
      onSelect: (index) => {
        this.currentFrameIndex = index;
        this.syncTimelineToCurrentOutputFrame();
        this.setViewMode("playback", false);
        this.renderCurrentView();
        this.syncPlaybackLabels();
        this.updateFrameCurrentState();
      },
    });
  },

  async handleSave(): Promise<SavedImageResult | null> {
    if (!this.spriteSheetBlob) {
      this.setStatus("请先生成序列帧图");
      return null;
    }

    this.setStatus("正在保存序列帧图...");

    try {
      const dataUrl = await blobToDataUrl(this.spriteSheetBlob);
      const result = await saveSpriteSheetDataUrl(dataUrl, getOutputFileName(this.sourceName));
      this.savedResult = result;
      this.setStatus(`已保存: ${result.file_name}`);
      this.syncControls();
      return result;
    } catch (err) {
      const message = getErrorMessage(err);
      console.error("[video-sprite] 保存失败:", err);
      this.setStatus(`保存失败: ${message}`);
      this.syncControls();
      return null;
    }
  },

  async handleUseAsReference(): Promise<void> {
    const result = this.savedResult || await this.handleSave();
    if (!result) return;
    await startImageTaskWithReference(result.file_path, result.file_name);
  },


} satisfies ThisType<VideoSpritePage>;

export type VideoSpriteExtractionMethods = typeof videoSpriteExtractionMethods;
