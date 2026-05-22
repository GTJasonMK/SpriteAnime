export type BackgroundMode = "edge" | "firstFrame" | "none";

export interface PixelBounds {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface VideoSpriteWorkerFrameInput {
  base64: string;
  time: number;
}

export interface VideoSpriteWorkerOptions {
  cols: number;
  maxFrameEdge: number;
  padding: number;
  threshold: number;
  bgMode: BackgroundMode;
  autoTrim: boolean;
  transparent: boolean;
  cropRegion: PixelBounds;
}

export interface VideoSpriteWorkerRequest {
  id: number;
  frames: VideoSpriteWorkerFrameInput[];
  options: VideoSpriteWorkerOptions;
}

export interface VideoSpriteWorkerFrameResult {
  blob: Blob;
  time: number;
  width: number;
  height: number;
}

export interface VideoSpriteWorkerProgressMessage {
  id: number;
  type: "progress";
  done: number;
  total: number;
  message: string;
}

export interface VideoSpriteWorkerSuccessMessage {
  id: number;
  type: "success";
  frames: VideoSpriteWorkerFrameResult[];
  sheetBlob: Blob;
  sheetWidth: number;
  sheetHeight: number;
  cellWidth: number;
  cellHeight: number;
}

export interface VideoSpriteWorkerErrorMessage {
  id: number;
  type: "error";
  error: string;
}

export type VideoSpriteWorkerMessage =
  | VideoSpriteWorkerProgressMessage
  | VideoSpriteWorkerSuccessMessage
  | VideoSpriteWorkerErrorMessage;
