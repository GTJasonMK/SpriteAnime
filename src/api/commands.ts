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
  save_dir: string;
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
  count: number
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
  gridCols: number
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

// ==================== 文件操作 ====================

export async function selectDirectory(): Promise<string> {
  return invoke<string>("select_directory");
}

export async function openImageFile(): Promise<FileOpenResult> {
  return invoke<FileOpenResult>("open_image_file");
}

export async function revealInExplorer(path: string): Promise<void> {
  return invoke("reveal_in_explorer", { path });
}

export async function openImageFilePath(path: string): Promise<void> {
  return invoke("open_image_file_path", { path });
}
