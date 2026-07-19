import { convertFileSrc } from "@tauri-apps/api/core";
import { nextFrame } from "../utils/async";

interface FrameSource {
  path: string;
  anchorX: number;
}

/// Canvas动画播放器，使用ImageBitmap预解码实现高性能帧动画
export class CanvasPlayer {
  private canvas: HTMLCanvasElement;
  private ctx: CanvasRenderingContext2D;
  private frames: ImageBitmap[] = [];
  private anchorsX: number[] = [];
  private scale: number = 1.0;
  private frameWidth: number = 0;
  private frameHeight: number = 0;
  private anchorCanvasX: number = 0;
  private canvasWidth: number = 0;
  private canvasHeight: number = 0;

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas;
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("无法获取Canvas 2D上下文");
    this.ctx = ctx;
  }

  /// 从临时帧路径预解码所有帧
  async loadFrames(frameDataList: FrameSource[]): Promise<void> {
    this.releaseFrames();

    this.frames = await decodeFrameBitmapsInBatches(frameDataList);

    if (this.frames.length > 0) {
      this.anchorsX = this.frames.map((frame, index) => {
        const anchor = frameDataList[index].anchorX;
        if (!Number.isFinite(anchor)) {
          throw new Error(`第 ${index + 1} 帧定位针无效`);
        }
        return Math.max(0, Math.min(frame.width, anchor));
      });
      const leftSpan = Math.max(...this.anchorsX);
      const rightSpan = Math.max(
        ...this.frames.map((frame, index) => frame.width - this.anchorsX[index])
      );
      this.anchorCanvasX = leftSpan;
      this.frameWidth = Math.max(1, Math.ceil(leftSpan + rightSpan));
      this.frameHeight = Math.max(...this.frames.map((frame) => frame.height));
    }
  }

  /// 绘制指定帧到Canvas
  renderFrame(index: number): void {
    if (this.frames.length === 0) return;
    const frame = this.frames[index % this.frames.length];
    const w = Math.max(1, Math.round(this.frameWidth * this.scale));
    const h = Math.max(1, Math.round(this.frameHeight * this.scale));

    if (this.canvasWidth !== w || this.canvasHeight !== h) {
      this.canvas.width = w;
      this.canvas.height = h;
      this.canvasWidth = w;
      this.canvasHeight = h;
    }

    this.ctx.clearRect(0, 0, w, h);
    this.ctx.imageSmoothingEnabled = this.scale > 1.5;
    const drawW = Math.max(1, Math.round(frame.width * this.scale));
    const drawH = Math.max(1, Math.round(frame.height * this.scale));
    const anchorX = this.anchorsX[index % this.frames.length];
    const x = Math.round((this.anchorCanvasX - anchorX) * this.scale);
    const y = h - drawH;
    this.ctx.drawImage(frame, x, y, drawW, drawH);
  }

  clear(): void {
    this.canvas.width = 1;
    this.canvas.height = 1;
    this.canvasWidth = 1;
    this.canvasHeight = 1;
  }

  setScale(s: number): void {
    this.scale = Math.max(0.1, Math.min(5.0, s));
  }

  destroy(): void {
    this.releaseFrames();
  }

  private releaseFrames(): void {
    this.frames.forEach((f) => f.close());
    this.frames = [];
    this.anchorsX = [];
    this.frameWidth = 0;
    this.frameHeight = 0;
    this.anchorCanvasX = 0;
    this.canvasWidth = 0;
    this.canvasHeight = 0;
  }
}

async function loadFrameBitmap(frame: FrameSource): Promise<ImageBitmap> {
  const path = frame.path.trim();
  if (!path) {
    throw new Error("帧缺少临时路径，请重新拆分后再预览。");
  }

  const image = new Image();
  image.decoding = "async";
  image.crossOrigin = "anonymous";
  image.src = convertFileSrc(path);
  await image.decode();
  return createImageBitmap(image);
}

async function decodeFrameBitmapsInBatches(
  frames: FrameSource[]
): Promise<ImageBitmap[]> {
  const decoded: ImageBitmap[] = [];
  try {
    const batchSize = 4;
    for (let start = 0; start < frames.length; start += batchSize) {
      const batch = frames.slice(start, start + batchSize);
      decoded.push(...await decodeFrameBitmapBatch(batch));
      await nextFrame();
    }
    return decoded;
  } catch (err) {
    decoded.forEach((frame) => frame.close());
    throw err;
  }
}

async function decodeFrameBitmapBatch(
  frames: FrameSource[]
): Promise<ImageBitmap[]> {
  const results = await Promise.allSettled(frames.map(loadFrameBitmap));
  const decoded: ImageBitmap[] = [];
  let failure: unknown = null;

  results.forEach((result) => {
    if (result.status === "fulfilled") {
      decoded.push(result.value);
    } else if (failure === null) {
      failure = result.reason;
    }
  });

  if (failure !== null) {
    decoded.forEach((frame) => frame.close());
    throw failure;
  }

  return decoded;
}
