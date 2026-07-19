import { invoke } from "@tauri-apps/api/core";
import type { SavedImageResult } from "./sprite";

export interface RedrawExtractionSnapshot {
  startSeconds: number;
  endSeconds: number;
}

export interface RedrawApiSnapshot {
  profileId: string;
  apiBase: string;
  model: string;
  apiMode: string;
}

export interface CreateRedrawRunRequest {
  sourceName: string;
  totalFrames: number;
  finalCols: number;
  groupRows: number;
  groupCols: number;
  prompt: string;
  negativePrompt: string;
  style: string;
  resolution: string;
  api: RedrawApiSnapshot;
  extraction: RedrawExtractionSnapshot;
}

export type RedrawBatchStatus =
  | "pending_input"
  | "pending"
  | "generating"
  | "succeeded"
  | "failed";

export interface RedrawBatchRecord {
  index: number;
  globalStart: number;
  validCount: number;
  status: RedrawBatchStatus;
  inputPath: string;
  outputPath: string;
  framePaths: string[];
  error: string;
}

export type RedrawRunStatus =
  | "preparing"
  | "ready"
  | "running"
  | "paused"
  | "ready_to_finalize"
  | "completed";

export interface RedrawRunManifest {
  id: string;
  status: RedrawRunStatus;
  totalFrames: number;
  finalCols: number;
  finalRows: number;
  groupRows: number;
  groupCols: number;
  prompt: string;
  negativePrompt: string;
  style: string;
  resolution: string;
  api: RedrawApiSnapshot;
  extraction: RedrawExtractionSnapshot;
  batches: RedrawBatchRecord[];
  finalOutputPath: string;
}

export interface RedrawBatchExecution {
  manifest: RedrawRunManifest;
  prompt: string;
  referenceImagePaths: string[];
}

export function createVideoSpriteRedrawRun(
  request: CreateRedrawRunRequest
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("create_video_sprite_redraw_run", { request });
}

export function saveVideoSpriteRedrawBatchInput(
  runId: string,
  batchIndex: number,
  dataUrl: string
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("save_video_sprite_redraw_batch_input", {
    runId,
    batchIndex,
    dataUrl,
  });
}

export function loadActiveVideoSpriteRedrawRun(): Promise<RedrawRunManifest | null> {
  return invoke<RedrawRunManifest | null>("load_active_video_sprite_redraw_run");
}

export function beginVideoSpriteRedrawBatch(
  runId: string,
  batchIndex: number
): Promise<RedrawBatchExecution> {
  return invoke<RedrawBatchExecution>("begin_video_sprite_redraw_batch", {
    runId,
    batchIndex,
  });
}

export function completeVideoSpriteRedrawBatch(
  runId: string,
  batchIndex: number,
  generatedPath: string
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("complete_video_sprite_redraw_batch", {
    runId,
    batchIndex,
    generatedPath,
  });
}

export function failVideoSpriteRedrawBatch(
  runId: string,
  batchIndex: number,
  error: string
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("fail_video_sprite_redraw_batch", {
    runId,
    batchIndex,
    error,
  });
}

export function pauseVideoSpriteRedrawRun(
  runId: string
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("pause_video_sprite_redraw_run", { runId });
}

export function updateVideoSpriteRedrawFinalCols(
  runId: string,
  finalCols: number
): Promise<RedrawRunManifest> {
  return invoke<RedrawRunManifest>("update_video_sprite_redraw_final_cols", {
    runId,
    finalCols,
  });
}

export function finalizeVideoSpriteRedrawRun(
  runId: string
): Promise<SavedImageResult> {
  return invoke<SavedImageResult>("finalize_video_sprite_redraw_run", { runId });
}

export function discardVideoSpriteRedrawRun(runId: string): Promise<void> {
  return invoke("discard_video_sprite_redraw_run", { runId });
}
