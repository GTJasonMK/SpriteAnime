import { detectFrameBoundsFromImageData } from "./auto-trim-core";
import type { AutoBoundsResult, SplitRegion } from "./types";

interface DetectFrameBoundsOptions {
  sheetImage: HTMLImageElement;
  rows: number;
  cols: number;
  region: SplitRegion;
  bgMode: string;
  threshold: number;
  allowExpand: boolean;
  cellRects?: SplitRegion[];
  gridSignature?: string;
}

let workerRequestId = 0;

export async function detectFrameBoundsForImageAsync(
  options: DetectFrameBoundsOptions
): Promise<AutoBoundsResult> {
  if (
    typeof Worker === "undefined" ||
    typeof OffscreenCanvas === "undefined" ||
    typeof createImageBitmap === "undefined"
  ) {
    return detectFrameBoundsForImage(options);
  }

  try {
    const bitmap = await createImageBitmap(options.sheetImage);
    return await detectFrameBoundsInWorker(options, bitmap);
  } catch (err) {
    console.warn("[sprite] Worker 自动边界检测失败，回退主线程:", err);
    return detectFrameBoundsForImage(options);
  }
}

export function detectFrameBoundsForImage(options: DetectFrameBoundsOptions): AutoBoundsResult {
  const { sheetImage, rows, cols, region, bgMode, threshold, allowExpand } = options;
  const canvas = document.createElement("canvas");
  canvas.width = sheetImage.naturalWidth;
  canvas.height = sheetImage.naturalHeight;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    throw new Error("无法创建检测画布");
  }
  ctx.drawImage(sheetImage, 0, 0);

  return detectFrameBoundsFromImageData({
    imageWidth: sheetImage.naturalWidth,
    imageHeight: sheetImage.naturalHeight,
    rows,
    cols,
    region,
    bgMode,
    threshold,
    allowExpand,
    reader: ctx,
    cellRects: options.cellRects,
    gridSignature: options.gridSignature,
  });
}

function detectFrameBoundsInWorker(
  options: DetectFrameBoundsOptions,
  bitmap: ImageBitmap
): Promise<AutoBoundsResult> {
  return new Promise((resolve, reject) => {
    const worker = new Worker(new URL("./bounds-worker.ts", import.meta.url), { type: "module" });
    const id = ++workerRequestId;
    const cleanup = (): void => worker.terminate();
    worker.onmessage = (event: MessageEvent<{ id: number; ok: boolean; result?: AutoBoundsResult; error?: string }>) => {
      if (event.data.id !== id) {
        return;
      }
      cleanup();
      if (event.data.ok && event.data.result) {
        resolve(event.data.result);
      } else {
        reject(new Error(event.data.error || "自动边界检测失败"));
      }
    };
    worker.onerror = (event) => {
      cleanup();
      reject(new Error(event.message || "自动边界检测 Worker 失败"));
    };
    worker.postMessage({
      id,
      bitmap,
      rows: options.rows,
      cols: options.cols,
      region: options.region,
      bgMode: options.bgMode,
      threshold: options.threshold,
      allowExpand: options.allowExpand,
      cellRects: options.cellRects,
      gridSignature: options.gridSignature,
    }, [bitmap]);
  });
}
