/// <reference lib="webworker" />

import { detectFrameBoundsFromImageData } from "./auto-trim-core";
import type { AutoBoundsResult, SplitRegion } from "./types";

interface DetectBoundsMessage {
  id: number;
  bitmap: ImageBitmap;
  rows: number;
  cols: number;
  region: SplitRegion;
  bgMode: string;
  threshold: number;
  allowExpand: boolean;
  cellRects?: SplitRegion[];
  gridSignature?: string;
}

interface WorkerSuccess {
  id: number;
  ok: true;
  result: AutoBoundsResult;
}

interface WorkerFailure {
  id: number;
  ok: false;
  error: string;
}

self.onmessage = (event: MessageEvent<DetectBoundsMessage>) => {
  const message = event.data;
  try {
    const result = detectFrameBounds(message);
    message.bitmap.close();
    self.postMessage({ id: message.id, ok: true, result } satisfies WorkerSuccess);
  } catch (err) {
    message.bitmap.close();
    self.postMessage({
      id: message.id,
      ok: false,
      error: String(err),
    } satisfies WorkerFailure);
  }
};

function detectFrameBounds(options: DetectBoundsMessage): AutoBoundsResult {
  const { bitmap, rows, cols, region, bgMode, threshold, allowExpand } = options;
  const canvas = new OffscreenCanvas(bitmap.width, bitmap.height);
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx) {
    throw new Error("无法创建检测画布");
  }
  ctx.drawImage(bitmap, 0, 0);

  return detectFrameBoundsFromImageData({
    imageWidth: bitmap.width,
    imageHeight: bitmap.height,
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
