import { invoke, type Channel } from "@tauri-apps/api/core";

export interface FrameData {
  index: number;
  path: string;
  width: number;
  height: number;
  anchorX: number;
}

interface ImageSize {
  width: number;
  height: number;
}

export interface SplitResult {
  frames: FrameData[];
  original_size: ImageSize;
}

export interface ExportFrame {
  index: number;
  path: string;
  anchorX: number;
}

export interface FrameCrop {
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  anchorX: number;
}

export interface SpriteRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface SpriteFrameBounds {
  index: number;
  cellX: number;
  cellY: number;
  cellWidth: number;
  cellHeight: number;
  x: number;
  y: number;
  width: number;
  height: number;
  anchorX: number;
  empty: boolean;
}

export interface SpriteLayoutResult {
  rows: number;
  cols: number;
  region: SpriteRegion;
  gridSignature: string;
  allowExpand: boolean;
  expandPixels: number;
  frameBounds: SpriteFrameBounds[];
  fixedOffsetX: number;
  fixedOffsetY: number;
  fixedWidth: number;
  fixedHeight: number;
  emptyCount: number;
}


export interface SavedImageResult {
  file_path: string;
  file_name: string;
}

export interface VideoProbeResult {
  duration_seconds: number;
  width: number;
  height: number;
}

export interface VideoExtractRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

interface VideoFrameFile {
  path: string;
  time_seconds: number;
  width: number;
  height: number;
}

export interface VideoFramesResult {
  frames: VideoFrameFile[];
  duration_seconds: number;
  width: number;
  height: number;
  output_dir: string;
}

export type VideoExtractEvent =
  | { event: "Probing" }
  | {
    event: "ExtractingFrame";
    data: { index: number; total: number; time_seconds: number };
  }
  | { event: "Completed"; data: { frames: number } };


export function extractSpriteFrames(
  imagePath: string,
  crops: FrameCrop[]
): Promise<SplitResult> {
  return invoke<SplitResult>("extract_sprite_frames", {
    imagePath,
    crops,
  });
}

export function detectSpriteLayout(
  imagePath: string,
  rows: number,
  cols: number,
  region: SpriteRegion,
  cellRects: SpriteRegion[],
  gridSignature: string,
  backgroundMode: string,
  threshold: number,
  allowExpand: boolean
): Promise<SpriteLayoutResult> {
  return invoke("detect_sprite_layout", {
    imagePath,
    rows,
    cols,
    region,
    cellRects,
    gridSignature,
    backgroundMode,
    threshold,
    allowExpand,
  });
}

export function exportFrames(
  frames: ExportFrame[],
  prefix: string
): Promise<string[]> {
  return invoke<string[]>("export_frames", {
    frames,
    prefix,
  });
}

export function exportGif(
  frames: ExportFrame[],
  fileName: string,
  fps: number
): Promise<string> {
  return invoke<string>("export_gif", {
    frames,
    fileName,
    fps,
  });
}

export function saveSpriteSheetDataUrl(
  dataUrl: string,
  fileName: string
): Promise<SavedImageResult> {
  return invoke<SavedImageResult>("save_sprite_sheet_data_url", {
    dataUrl,
    fileName,
  });
}

export function probeVideoFile(videoPath: string): Promise<VideoProbeResult> {
  return invoke<VideoProbeResult>("probe_video_file", { videoPath });
}

export function extractVideoFramesWithFfmpeg(
  channel: Channel<VideoExtractEvent>,
  videoPath: string,
  frameCount: number,
  startSeconds: number,
  endSeconds: number,
  cropRegion?: VideoExtractRegion,
  maxExtractEdge?: number
): Promise<VideoFramesResult> {
  return invoke<VideoFramesResult>("extract_video_frames_with_ffmpeg", {
    channel,
    videoPath,
    frameCount,
    startSeconds,
    endSeconds,
    cropRegion: cropRegion ?? null,
    maxExtractEdge: maxExtractEdge ?? null,
  });
}

export function logVideoSpriteMessage(message: string): Promise<void> {
  return invoke("log_video_sprite_message", { message });
}
