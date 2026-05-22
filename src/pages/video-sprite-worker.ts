/// <reference lib="webworker" />

import type {
  PixelBounds,
  VideoSpriteWorkerErrorMessage as WorkerFailure,
  VideoSpriteWorkerOptions as WorkerOptions,
  VideoSpriteWorkerProgressMessage as WorkerProgress,
  VideoSpriteWorkerRequest as ProcessMessage,
  VideoSpriteWorkerSuccessMessage as WorkerSuccess,
} from "./video-sprite-types";
import { loadBitmapFromBase64 } from "../utils/bitmap";
import { clampPixelBounds, expandPixelBounds } from "./video-sprite-utils";

interface ProcessedFrameInternal {
  canvas: OffscreenCanvas;
  blob: Blob;
  time: number;
  width: number;
  height: number;
}

self.onmessage = (event: MessageEvent<ProcessMessage>) => {
  void processFrames(event.data);
};

async function processFrames(message: ProcessMessage): Promise<void> {
  try {
    if (message.frames.length === 0) {
      throw new Error("没有可处理的帧");
    }

    const firstFrameData = message.options.bgMode === "firstFrame"
      ? await decodeFrameImageData(message.frames[0].base64, message.options.cropRegion)
      : null;
    const processed: ProcessedFrameInternal[] = [];

    for (let i = 0; i < message.frames.length; i += 1) {
      postProgress(message.id, i, message.frames.length, `处理第 ${i + 1}/${message.frames.length} 帧`);
      const input = message.frames[i];
      const imageData = firstFrameData && i === 0
        ? firstFrameData
        : await decodeFrameImageData(input.base64, message.options.cropRegion);
      const canvas = buildExtractedFrame(imageData, firstFrameData, message.options);
      const blob = await canvas.convertToBlob({ type: "image/png" });
      processed.push({
        canvas,
        blob,
        time: input.time,
        width: canvas.width,
        height: canvas.height,
      });
    }

    postProgress(message.id, message.frames.length, message.frames.length, "合成序列帧图");
    const sheet = await composeSpriteSheet(processed, message.options);
    self.postMessage({
      id: message.id,
      type: "success",
      frames: processed.map((frame) => ({
        blob: frame.blob,
        time: frame.time,
        width: frame.width,
        height: frame.height,
      })),
      sheetBlob: sheet.blob,
      sheetWidth: sheet.width,
      sheetHeight: sheet.height,
      cellWidth: sheet.cellWidth,
      cellHeight: sheet.cellHeight,
    } satisfies WorkerSuccess);
  } catch (err) {
    self.postMessage({
      id: message.id,
      type: "error",
      error: String(err),
    } satisfies WorkerFailure);
  }
}

function postProgress(id: number, done: number, total: number, message: string): void {
  self.postMessage({
    id,
    type: "progress",
    done,
    total,
    message,
  } satisfies WorkerProgress);
}

async function decodeFrameImageData(base64: string, cropRegion: PixelBounds): Promise<ImageData> {
  const bitmap = await loadBitmapFromBase64(base64);
  try {
    const crop = clampPixelBounds(cropRegion, bitmap.width, bitmap.height);
    const canvas = new OffscreenCanvas(crop.width, crop.height);
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) {
      throw new Error("无法创建视频帧处理画布");
    }
    ctx.drawImage(
      bitmap,
      crop.x,
      crop.y,
      crop.width,
      crop.height,
      0,
      0,
      crop.width,
      crop.height
    );
    return ctx.getImageData(0, 0, crop.width, crop.height);
  } finally {
    bitmap.close();
  }
}

function buildExtractedFrame(
  imageData: ImageData,
  firstFrameData: ImageData | null,
  options: WorkerOptions
): OffscreenCanvas {
  const bgRgb = options.bgMode === "edge"
    ? estimateEdgeBackgroundRgb(imageData)
    : null;
  const foreground = options.autoTrim && options.transparent && options.bgMode !== "none"
    ? analyzeForeground(imageData, firstFrameData, bgRgb, options)
    : null;
  const detected = options.autoTrim && options.bgMode !== "none"
    ? foreground?.bounds || detectForegroundBounds(imageData, firstFrameData, bgRgb, options)
    : null;
  const crop = expandPixelBounds(
    detected || {
      x: 0,
      y: 0,
      width: imageData.width,
      height: imageData.height,
    },
    options.padding,
    imageData.width,
    imageData.height
  );
  const cropped = createCroppedFrameCanvas(
    imageData,
    crop,
    firstFrameData,
    bgRgb,
    options,
    foreground?.mask || null
  );
  return scaleFrameCanvas(cropped, options.maxFrameEdge);
}

function analyzeForeground(
  imageData: ImageData,
  firstFrameData: ImageData | null,
  bgRgb: [number, number, number] | null,
  options: WorkerOptions
): { mask: Uint8Array; bounds: PixelBounds | null } {
  const mask = new Uint8Array(imageData.width * imageData.height);
  let left = imageData.width;
  let top = imageData.height;
  let right = -1;
  let bottom = -1;

  for (let y = 0; y < imageData.height; y += 1) {
    for (let x = 0; x < imageData.width; x += 1) {
      if (!isForegroundPixel(imageData, x, y, firstFrameData, bgRgb, options)) {
        continue;
      }
      mask[y * imageData.width + x] = 1;
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x);
      bottom = Math.max(bottom, y);
    }
  }

  return {
    mask,
    bounds: right < left || bottom < top
      ? null
      : {
          x: left,
          y: top,
          width: right - left + 1,
          height: bottom - top + 1,
        },
  };
}

function detectForegroundBounds(
  imageData: ImageData,
  firstFrameData: ImageData | null,
  bgRgb: [number, number, number] | null,
  options: WorkerOptions
): PixelBounds | null {
  let left = imageData.width;
  let top = imageData.height;
  let right = -1;
  let bottom = -1;

  for (let y = 0; y < imageData.height; y += 1) {
    for (let x = 0; x < imageData.width; x += 1) {
      if (!isForegroundPixel(imageData, x, y, firstFrameData, bgRgb, options)) {
        continue;
      }
      left = Math.min(left, x);
      top = Math.min(top, y);
      right = Math.max(right, x);
      bottom = Math.max(bottom, y);
    }
  }

  if (right < left || bottom < top) {
    return null;
  }
  return {
    x: left,
    y: top,
    width: right - left + 1,
    height: bottom - top + 1,
  };
}

function createCroppedFrameCanvas(
  imageData: ImageData,
  crop: PixelBounds,
  firstFrameData: ImageData | null,
  bgRgb: [number, number, number] | null,
  options: WorkerOptions,
  foregroundMask: Uint8Array | null
): OffscreenCanvas {
  const canvas = new OffscreenCanvas(crop.width, crop.height);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    throw new Error("无法创建单帧处理画布");
  }

  const output = ctx.createImageData(crop.width, crop.height);
  for (let y = 0; y < crop.height; y += 1) {
    for (let x = 0; x < crop.width; x += 1) {
      const srcX = crop.x + x;
      const srcY = crop.y + y;
      const srcIndex = (srcY * imageData.width + srcX) * 4;
      const dstIndex = (y * crop.width + x) * 4;
      const shouldClear = options.transparent &&
        options.bgMode !== "none" &&
        (foregroundMask
          ? foregroundMask[srcY * imageData.width + srcX] === 0
          : !isForegroundPixel(imageData, srcX, srcY, firstFrameData, bgRgb, options));

      if (shouldClear) {
        output.data[dstIndex] = 0;
        output.data[dstIndex + 1] = 0;
        output.data[dstIndex + 2] = 0;
        output.data[dstIndex + 3] = 0;
      } else {
        output.data[dstIndex] = imageData.data[srcIndex];
        output.data[dstIndex + 1] = imageData.data[srcIndex + 1];
        output.data[dstIndex + 2] = imageData.data[srcIndex + 2];
        output.data[dstIndex + 3] = imageData.data[srcIndex + 3];
      }
    }
  }

  ctx.putImageData(output, 0, 0);
  return canvas;
}

function scaleFrameCanvas(source: OffscreenCanvas, maxEdge: number): OffscreenCanvas {
  const edge = Math.max(source.width, source.height);
  if (edge <= maxEdge) {
    return source;
  }

  const scale = maxEdge / edge;
  const canvas = new OffscreenCanvas(
    Math.max(1, Math.round(source.width * scale)),
    Math.max(1, Math.round(source.height * scale))
  );
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("无法缩放单帧");
  }
  ctx.imageSmoothingEnabled = true;
  ctx.drawImage(source, 0, 0, canvas.width, canvas.height);
  return canvas;
}

async function composeSpriteSheet(
  frames: ProcessedFrameInternal[],
  options: WorkerOptions
): Promise<{ blob: Blob; width: number; height: number; cellWidth: number; cellHeight: number }> {
  const cols = Math.max(1, Math.min(options.cols, frames.length));
  const rows = Math.ceil(frames.length / cols);
  const cellWidth = frames.reduce((max, frame) => Math.max(max, frame.width), 1);
  const cellHeight = frames.reduce((max, frame) => Math.max(max, frame.height), 1);
  const canvas = new OffscreenCanvas(cols * cellWidth, rows * cellHeight);
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    throw new Error("无法创建序列帧合成画布");
  }
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (!options.transparent) {
    ctx.fillStyle = "#f4efe8";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
  }

  frames.forEach((frame, index) => {
    const col = index % cols;
    const row = Math.floor(index / cols);
    const x = col * cellWidth + Math.round((cellWidth - frame.width) / 2);
    const y = row * cellHeight + (cellHeight - frame.height);
    ctx.drawImage(frame.canvas, x, y);
  });

  return {
    blob: await canvas.convertToBlob({ type: "image/png" }),
    width: canvas.width,
    height: canvas.height,
    cellWidth,
    cellHeight,
  };
}

function isForegroundPixel(
  imageData: ImageData,
  x: number,
  y: number,
  firstFrameData: ImageData | null,
  bgRgb: [number, number, number] | null,
  options: WorkerOptions
): boolean {
  const index = (y * imageData.width + x) * 4;
  const alpha = imageData.data[index + 3];
  if (alpha <= 10) {
    return false;
  }
  if (options.bgMode === "none") {
    return true;
  }

  const thresholdSq = options.threshold * options.threshold;
  if (options.bgMode === "firstFrame" && firstFrameData) {
    const dr = imageData.data[index] - firstFrameData.data[index];
    const dg = imageData.data[index + 1] - firstFrameData.data[index + 1];
    const db = imageData.data[index + 2] - firstFrameData.data[index + 2];
    return dr * dr + dg * dg + db * db > thresholdSq;
  }

  if (!bgRgb) {
    return true;
  }
  const dr = imageData.data[index] - bgRgb[0];
  const dg = imageData.data[index + 1] - bgRgb[1];
  const db = imageData.data[index + 2] - bgRgb[2];
  return dr * dr + dg * dg + db * db > thresholdSq;
}

function estimateEdgeBackgroundRgb(imageData: ImageData): [number, number, number] {
  const { width, height, data } = imageData;
  const step = Math.max(1, Math.floor(Math.max(width, height) / 180));
  let r = 0;
  let g = 0;
  let b = 0;
  let count = 0;

  const sample = (x: number, y: number) => {
    const index = (y * width + x) * 4;
    if (data[index + 3] <= 10) return;
    r += data[index];
    g += data[index + 1];
    b += data[index + 2];
    count += 1;
  };

  for (let x = 0; x < width; x += step) {
    sample(x, 0);
    sample(x, height - 1);
  }
  for (let y = 0; y < height; y += step) {
    sample(0, y);
    sample(width - 1, y);
  }

  if (count === 0) {
    return [255, 255, 255];
  }
  return [
    Math.round(r / count),
    Math.round(g / count),
    Math.round(b / count),
  ];
}
