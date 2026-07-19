import { invoke } from "@tauri-apps/api/core";

export interface FileOpenResult {
  file_path: string;
  file_name: string;
}

export interface TempCleanupResult {
  removed_dirs: number;
}


export function readImageAsBase64(path: string): Promise<string> {
  return invoke<string>("read_image_as_base64", { path });
}

export function readFileAsBase64(path: string): Promise<string> {
  return invoke<string>("read_file_as_base64", { path });
}

export function openImageFile(): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("open_image_file");
}

export function openVideoFile(): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("open_video_file");
}

export function importImageToLibrary(sourcePath: string): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("import_image_to_library", { sourcePath });
}

export function importVideoToLibrary(sourcePath: string): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("import_video_to_library", { sourcePath });
}

export function cleanupVideoFrameBatchDir(outputDir: string): Promise<TempCleanupResult> {
  return invoke<TempCleanupResult>("cleanup_video_frame_batch_dir", { outputDir });
}

export function cleanupVideoSpriteTempFiles(): Promise<TempCleanupResult> {
  return invoke<TempCleanupResult>("cleanup_video_sprite_temp_files");
}

export function revealInExplorer(path: string): Promise<void> {
  return invoke("reveal_in_explorer", { path });
}

export function openImageFilePath(path: string): Promise<void> {
  return invoke("open_image_file_path", { path });
}
