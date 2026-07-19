import { invoke, type Channel } from "@tauri-apps/api/core";
import type {
  ImageGenerationConstraints,
  VideoGenerationConstraints,
} from "../generation/constraints";

export type GenerateEvent =
  | { event: "SendingRequest" }
  | { event: "ExtractingUrls"; data: { found: number } }
  | { event: "ProcessingImage"; data: { index: number; step: string } }
  | { event: "Completed"; data: { total_images: number } };

export type VideoGenerationEvent =
  | { event: "Submitting" }
  | { event: "Saving" }
  | { event: "Completed" };

export interface GenerationResult {
  image_urls: string[];
  duration_seconds: number;
}

export interface GeneratedVideoResult {
  file_path: string;
  file_name: string;
  duration_seconds: number;
}

export interface GenerateVideoRequest {
  apiKey: string;
  apiBase: string;
  proxyUrl: string;
  prompt: string;
  model: string;
  apiMode: string;
  size: string;
  seconds: number;
  sourceVideoId: string;
  extensionDirection: string;
  referenceImagePath: string;
}

export interface WorkbenchRecord {
  id: string;
  path: string;
  label: string;
  prompt: string;
  model: string;
  durationSeconds?: number;
  createdAt: string;
  updatedAt: string;
}

export interface TransparentBackgroundResult {
  file_path: string;
  file_name: string;
  transparent_pixels: number;
}

export interface TransparentBackgroundCanvasResult {
  base64_data: string;
  background_color: string;
  transparent_pixels: number;
}

export interface ConnectedEraseCanvasResult {
  base64_data: string;
  erased_pixels: number;
  operations: Array<{
    erasedPixels: number;
    reason: "erased" | "outside" | "no_seed";
  }>;
}

export interface PromptOptimizationResult {
  prompt: string;
  negativePrompt: string;
  gridRows: number;
  gridCols: number;
}


export function generateImage(
  channel: Channel<GenerateEvent>,
  apiKey: string,
  apiBase: string,
  prompt: string,
  negPrompt: string,
  model: string,
  style: string,
  ratio: string,
  resolution: string,
  count: number,
  apiMode: string,
  referenceImagePaths: string[],
  proxyUrl: string
): Promise<GenerationResult> {
  return invoke<GenerationResult>("generate_image", {
    channel,
    apiKey,
    apiBase,
    prompt,
    negPrompt,
    model,
    style,
    ratio,
    resolution,
    count,
    apiMode,
    referenceImagePaths,
    proxyUrl,
  });
}

export function buildSpriteImagePrompt(
  prompt: string,
  constraints: ImageGenerationConstraints,
  rows: number,
  cols: number,
  hasReference: boolean
): Promise<string> {
  return invoke<string>("build_sprite_image_prompt", {
    prompt,
    constraints,
    rows,
    cols,
    hasReference,
  });
}

export function buildRedrawConstraintPrompt(
  prompt: string,
  constraints: ImageGenerationConstraints
): Promise<string> {
  return invoke<string>("build_redraw_constraint_prompt", { prompt, constraints });
}

export function buildVideoPrompt(
  prompt: string,
  constraints: VideoGenerationConstraints,
  hasReference: boolean
): Promise<string> {
  return invoke<string>("build_video_prompt", { prompt, constraints, hasReference });
}

export function generateVideo(
  channel: Channel<VideoGenerationEvent>,
  request: GenerateVideoRequest
): Promise<GeneratedVideoResult> {
  return invoke<GeneratedVideoResult>("generate_video", {
    channel,
    request,
  });
}

export function addPromptHistory(prompt: string): Promise<string[]> {
  return invoke<string[]>("add_prompt_history", { prompt });
}

export function readWorkbenchRecords(limit: number): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("read_workbench_records", { limit });
}

export function upsertWorkbenchRecords(records: WorkbenchRecord[]): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("upsert_workbench_records", { records });
}

export function deleteWorkbenchRecord(id: string): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("delete_workbench_record", { id });
}

export function clearWorkbenchRecords(): Promise<void> {
  return invoke("clear_workbench_records");
}

export function applyCanvasBackgroundTransparent(
  dataUrl: string,
  tolerance: number,
  featherRadius: number,
  colorKeyMode: string
): Promise<TransparentBackgroundCanvasResult> {
  return invoke<TransparentBackgroundCanvasResult>("apply_canvas_background_transparent", {
    dataUrl,
    tolerance,
    featherRadius,
    colorKeyMode,
  });
}

export function applyCanvasConnectedErase(
  dataUrl: string,
  operation: { x: number; y: number; tolerance: number; radius: number }
): Promise<ConnectedEraseCanvasResult> {
  return invoke<ConnectedEraseCanvasResult>("apply_canvas_connected_erase", {
    dataUrl,
    operations: {
      schemaVersion: 1,
      operations: [operation],
    },
  });
}

export function saveMattedImageDataUrl(
  sourcePath: string,
  dataUrl: string
): Promise<TransparentBackgroundResult> {
  return invoke<TransparentBackgroundResult>("save_matted_image_data_url", {
    sourcePath,
    dataUrl,
  });
}

export function optimizePrompt(
  apiKey: string,
  apiBase: string,
  apiMode: string,
  prompt: string,
  negPrompt: string,
  model: string,
  style: string,
  ratio: string,
  resolution: string,
  gridRows: number,
  gridCols: number,
  referenceImagePath: string,
  useReferenceImageUnderstanding: boolean,
  proxyUrl: string
): Promise<PromptOptimizationResult> {
  return invoke<PromptOptimizationResult>("optimize_prompt", {
    apiKey,
    apiBase,
    apiMode,
    prompt,
    negPrompt,
    model,
    style,
    ratio,
    resolution,
    gridRows,
    gridCols,
    referenceImagePath,
    useReferenceImageUnderstanding,
    proxyUrl,
  });
}
