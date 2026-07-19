import { convertFileSrc } from "@tauri-apps/api/core";
import {
  readFileAsBase64,
  type GenerateEvent,
  type VideoExtractRegion
} from "../../../api/commands";
import { loadBitmapFromBase64 } from "../../../utils/bitmap";
import { getErrorMessage } from "../../../utils/errors";
import { clampNumber } from "../../../utils/number";
import { stripFileExtension as stripExtension } from "../../../utils/path";
import type {
  PixelBounds
} from "../types";

import type { CropDragState, ExtractionOptions, SourcePreviewFrame } from "../video-page";

export function createRedrawBatchImage(path: string, label: string): HTMLImageElement {
  const image = document.createElement("img");
  image.alt = `${label}缩略图`;
  image.loading = "lazy";
  if (path) {
    image.src = convertFileSrc(path);
  }
  return image;
}

export function redrawBatchStatusLabel(status: string): string {
  switch (status) {
    case "pending_input":
      return "准备输入图";
    case "pending":
      return "等待生成";
    case "generating":
      return "生成中";
    case "succeeded":
      return "已完成";
    case "failed":
      return "失败，可重试";
    default:
      return status;
  }
}

export function describeGenerateEvent(event: GenerateEvent): string {
  switch (event.event) {
    case "SendingRequest":
      return "正在发送";
    case "ExtractingUrls":
      return "正在读取结果";
    case "ProcessingImage":
      return event.data.step;
    case "Completed":
      return "生成完成";
  }
}

export function base64PngToBlob(base64: string): Blob {
  const binary = window.atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return new Blob([bytes], { type: "image/png" });
}

export function requiredPathFileName(path: string, context: string): string {
  const normalized = path.trim().replaceAll("\\", "/");
  const fileName = normalized.slice(normalized.lastIndexOf("/") + 1);
  if (!fileName) {
    throw new Error(`${context}路径缺少文件名`);
  }
  return fileName;
}

export async function loadBitmapFromPath(path: string): Promise<ImageBitmap> {
  const base64 = await readFileAsBase64(path);
  return loadBitmapFromBase64(base64);
}

export function toVideoExtractRegion(region: PixelBounds): VideoExtractRegion {
  return {
    x: Math.round(region.x),
    y: Math.round(region.y),
    width: Math.round(region.width),
    height: Math.round(region.height),
  };
}

export function getBackendMaxExtractEdge(options: ExtractionOptions): number {
  if (!options.autoTrim && options.bgMode === "none") {
    return options.maxFrameEdge;
  }
  const qualityEdge = Math.max(options.maxFrameEdge * 2, options.maxFrameEdge + options.padding * 4);
  return clampNumber(Math.round(qualityEdge), options.maxFrameEdge, 1536);
}

export function getSourcePreviewFrameCount(duration: number, previewFps: number, maxFrames: number): number {
  const safeFps = clampNumber(Math.round(previewFps), 1, 30);
  const safeMaxFrames = clampNumber(Math.round(maxFrames), 2, 240);
  if (!Number.isFinite(duration) || duration <= 0) {
    return clampNumber(safeFps, 2, safeMaxFrames);
  }
  return clampNumber(Math.ceil(duration * safeFps), 2, safeMaxFrames);
}

export function findNearestSourceFrameIndex(frames: SourcePreviewFrame[], time: number): number {
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

export function closeSourcePreviewFrames(frames: SourcePreviewFrame[]): void {
  frames.forEach((frame) => frame.bitmap.close());
}

export function blobToDataUrl(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (typeof reader.result !== "string" || !reader.result) {
        reject(new Error("读取 PNG 数据未返回 data URL"));
        return;
      }
      resolve(reader.result);
    };
    reader.onerror = () => reject(reader.error ?? new Error("读取 PNG 数据失败"));
    reader.readAsDataURL(blob);
  });
}

export function regionFromPoints(
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

export function waitForVideoReady(video: HTMLVideoElement, timeoutMs: number): Promise<void> {
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

export function getMediaErrorMessage(video: HTMLVideoElement, err: unknown): string {
  const mediaError = video.error;
  if (!mediaError) {
    return getErrorMessage(err);
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

export function readRequiredIntegerInput(
  input: HTMLInputElement,
  label: string,
  min: number,
  max: number
): number {
  const value = readRequiredNumberInput(input, label, min, max);
  if (!Number.isInteger(value)) {
    throw new Error(`${label}必须是整数。解决方法：请输入 ${min}-${max} 之间的整数。`);
  }
  return value;
}

export function readRequiredNumberInput(
  input: HTMLInputElement,
  label: string,
  min: number,
  max: number
): number {
  const raw = input.value.trim();
  if (!raw) {
    throw new Error(`${label}为空。解决方法：请输入 ${min}-${formatInputLimit(max)} 之间的数字。`);
  }
  const value = Number(raw);
  if (!Number.isFinite(value)) {
    throw new Error(`${label}不是有效数字。解决方法：请输入 ${min}-${formatInputLimit(max)} 之间的数字。`);
  }
  if (value < min || value > max) {
    throw new Error(`${label}超出范围。解决方法：请输入 ${min}-${formatInputLimit(max)} 之间的数字。`);
  }
  return value;
}

export function formatInputLimit(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

export function getOutputFileName(sourceName: string): string {
  const stem = stripExtension(sourceName.trim());
  if (!stem) {
    throw new Error(
      "保存序列帧图缺少视频源文件名。解决方法：请重新选择或生成带文件名的视频后再保存。"
    );
  }
  return `${stem}_sprite_sheet.png`;
}

export function requiredVideoFileName(fileName: string, filePath: string, context: string): string {
  const name = fileName.trim();
  if (name) return name;
  const path = filePath.trim() || "(空路径)";
  throw new Error(
    `${context}缺少视频文件名。解决方法：请重新选择或生成带文件名的视频；如果这是旧数据，请确认后端返回 file_name。路径：${path}`
  );
}
