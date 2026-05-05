import { invoke } from "@tauri-apps/api/core";
import { type Channel } from "@tauri-apps/api/core";

// ==================== 类型定义 ====================

export interface StyleOption {
  key: string;
  label: string;
  prompt_suffix: string;
}

export interface RatioOption {
  key: string;
  width: number;
  height: number;
}

export interface PresetsPayload {
  models: string[];
  styles: StyleOption[];
  ratios: RatioOption[];
  resolutions: string[];
}

export interface UserConfig {
  api_key: string;
  api_base: string;
  proxy_url: string;
  last_model: string;
  last_ratio: string;
  last_resolution: string;
  last_style: string;
  last_count: number;
  prompt_optimizer_api_key: string;
  prompt_optimizer_api_base: string;
  prompt_optimizer_model: string;
  prompt_optimizer_vision: boolean;
  save_dir: string;
  ffmpeg_path: string;
  ffprobe_path: string;
  prompt_history: string[];
}

export interface GenerateEvent {
  event: string;
  data: any;
}

export interface GenerationResult {
  images_base64: string[];
  image_urls: string[];
  duration_seconds?: number;
}

export interface FrameData {
  index: number;
  base64?: string;
  path?: string;
  width: number;
  height: number;
  anchorX?: number;
}

export interface ImageSize {
  width: number;
  height: number;
}

export interface SplitResult {
  frames: FrameData[];
  total_frames: number;
  original_size: ImageSize;
}

export interface ExportFrame {
  index: number;
  path?: string;
  base64?: string;
  anchorX?: number;
}

export interface FrameCrop {
  index: number;
  x: number;
  y: number;
  width: number;
  height: number;
  anchorX?: number;
}

export interface FileOpenResult {
  file_path: string;
  file_name: string;
  base64_data: string;
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
  base64_data: string;
  background_color: string;
  transparent_pixels: number;
}

export interface TransparentBackgroundCanvasResult {
  base64_data: string;
  background_color: string;
  transparent_pixels: number;
}

export interface PromptOptimizationResult {
  prompt: string;
  negativePrompt: string;
  gridRows: number;
  gridCols: number;
  warning?: string;
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

export interface VideoFrameFile {
  index: number;
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

export interface VideoExtractEvent {
  event: string;
  data?: any;
}

export interface TempCleanupResult {
  removed_files: number;
  removed_dirs: number;
}

export interface ApiCheckResult {
  ok: boolean;
  status: "ok" | "warning";
  message: string;
  endpoint: string;
  model: string;
  modelFound?: boolean | null;
}

// ==================== 预设与配置 ====================

export async function getPresets(): Promise<PresetsPayload> {
  return invoke<PresetsPayload>("get_presets");
}

export async function loadConfig(): Promise<UserConfig> {
  return invoke<UserConfig>("load_config");
}

export async function saveConfig(config: UserConfig): Promise<void> {
  return invoke("save_config", { config });
}

export async function checkGenerationApi(
  apiKey: string,
  apiBase: string,
  model: string,
  proxyUrl: string
): Promise<ApiCheckResult> {
  return invoke<ApiCheckResult>("check_generation_api", {
    apiKey,
    apiBase,
    model,
    proxyUrl,
  });
}

export async function checkPromptOptimizerApi(
  apiKey: string,
  apiBase: string,
  model: string,
  proxyUrl: string
): Promise<ApiCheckResult> {
  return invoke<ApiCheckResult>("check_prompt_optimizer_api", {
    apiKey,
    apiBase,
    model,
    proxyUrl,
  });
}

// ==================== 生成图片 ====================

export async function generateImage(
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
  referenceImagePath: string
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
    referenceImagePath,
  });
}

// ==================== 提示词历史 ====================

export async function getPromptHistory(limit: number): Promise<string[]> {
  return invoke<string[]>("get_prompt_history", { limit });
}

export async function addPromptHistory(prompt: string): Promise<void> {
  return invoke("add_prompt_history", { prompt });
}

export async function readWorkbenchRecords(limit: number): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("read_workbench_records", { limit });
}

export async function upsertWorkbenchRecords(records: WorkbenchRecord[]): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("upsert_workbench_records", { records });
}

export async function deleteWorkbenchRecord(id: string): Promise<WorkbenchRecord[]> {
  return invoke<WorkbenchRecord[]>("delete_workbench_record", { id });
}

export async function clearWorkbenchRecords(): Promise<void> {
  return invoke("clear_workbench_records");
}

export async function applyCanvasBackgroundTransparent(
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

export async function saveMattedImageDataUrl(
  sourcePath: string,
  dataUrl: string
): Promise<TransparentBackgroundResult> {
  return invoke<TransparentBackgroundResult>("save_matted_image_data_url", {
    sourcePath,
    dataUrl,
  });
}

export async function readImageAsBase64(path: string): Promise<string> {
  return invoke<string>("read_image_as_base64", { path });
}

export async function readFileAsBase64(path: string): Promise<string> {
  return invoke<string>("read_file_as_base64", { path });
}

export async function optimizePrompt(
  apiKey: string,
  apiBase: string,
  prompt: string,
  negPrompt: string,
  model: string,
  style: string,
  ratio: string,
  resolution: string,
  gridRows: number,
  gridCols: number,
  referenceImagePath: string,
  useReferenceImageUnderstanding: boolean
): Promise<PromptOptimizationResult> {
  return invoke<PromptOptimizationResult>("optimize_prompt", {
    apiKey,
    apiBase,
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
  });
}

// ==================== 序列帧 ====================

export async function extractSpriteFrames(
  imagePath: string,
  crops: FrameCrop[]
): Promise<SplitResult> {
  return invoke<SplitResult>("extract_sprite_frames", {
    imagePath,
    crops,
  });
}

export async function exportFrames(
  frames: ExportFrame[],
  outputDir: string,
  prefix: string
): Promise<string[]> {
  return invoke<string[]>("export_frames", {
    frames,
    outputDir,
    prefix,
  });
}

export async function exportGif(
  frames: ExportFrame[],
  outputDir: string,
  fileName: string,
  fps: number
): Promise<string> {
  return invoke<string>("export_gif", {
    frames,
    outputDir,
    fileName,
    fps,
  });
}

export async function saveSpriteSheetDataUrl(
  dataUrl: string,
  fileName: string
): Promise<SavedImageResult> {
  return invoke<SavedImageResult>("save_sprite_sheet_data_url", {
    dataUrl,
    fileName,
  });
}

export async function probeVideoFile(videoPath: string): Promise<VideoProbeResult> {
  return invoke<VideoProbeResult>("probe_video_file", { videoPath });
}

export async function extractVideoFramesWithFfmpeg(
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

export async function logVideoSpriteMessage(message: string): Promise<void> {
  return invoke("log_video_sprite_message", { message });
}

// ==================== 文件操作 ====================

export async function selectDirectory(): Promise<string> {
  return invoke<string>("select_directory");
}

export async function openImageFile(): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("open_image_file");
}

export async function openVideoFile(): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("open_video_file");
}

export async function prepareVideoFileForPlayback(sourcePath: string): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("prepare_video_file_for_playback", { sourcePath });
}

export async function cleanupPreparedVideoFile(path: string): Promise<TempCleanupResult> {
  return invoke<TempCleanupResult>("cleanup_prepared_video_file", { path });
}

export async function cleanupVideoFrameBatchDir(outputDir: string): Promise<TempCleanupResult> {
  return invoke<TempCleanupResult>("cleanup_video_frame_batch_dir", { outputDir });
}

export async function cleanupVideoSpriteTempFiles(): Promise<TempCleanupResult> {
  return invoke<TempCleanupResult>("cleanup_video_sprite_temp_files");
}

export async function revealInExplorer(path: string): Promise<void> {
  return invoke("reveal_in_explorer", { path });
}

export async function openImageFilePath(path: string): Promise<void> {
  return invoke("open_image_file_path", { path });
}
