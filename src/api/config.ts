import { invoke } from "@tauri-apps/api/core";

export interface StyleOption {
  key: string;
  label: string;
}

export interface RatioOption {
  key: string;
}

export interface PresetsPayload {
  models: string[];
  styles: StyleOption[];
  ratios: RatioOption[];
  resolutions: string[];
}

export interface ApiProfile {
  id: string;
  name: string;
  api_key: string;
  api_base: string;
  proxy_url: string;
  generation_api_mode: string;
  last_model: string;
  video_api_key: string;
  video_api_base: string;
  video_proxy_url: string;
  video_model: string;
  video_api_mode: string;
  prompt_optimizer_api_key: string;
  prompt_optimizer_api_base: string;
  prompt_optimizer_api_mode: string;
  prompt_optimizer_model: string;
  prompt_optimizer_vision: boolean;
}

export interface ActiveApiSettings {
  profileId: string;
  apiKey: string;
  apiBase: string;
  proxyUrl: string;
  apiMode: string;
  model: string;
  videoApiKey: string;
  videoApiBase: string;
  videoProxyUrl: string;
  videoModel: string;
  videoApiMode: string;
  profileName: string;
}

export interface UserConfig {
  api_profiles: ApiProfile[];
  active_api_profile_id: string;
  last_ratio: string;
  last_resolution: string;
  last_style: string;
  last_count: number;
  ffmpeg_path: string;
  ffprobe_path: string;
  prompt_history: string[];
}

export interface ApiCheckResult {
  status: "ok" | "warning";
  message: string;
  endpoint: string;
  model: string;
}

export interface ConfigFileResult {
  file_path: string;
}

export interface ImportedConfigResult {
  file_path: string;
  config: UserConfig;
}


export function getPresets(): Promise<PresetsPayload> {
  return invoke<PresetsPayload>("get_presets");
}

export function loadConfig(): Promise<UserConfig> {
  return invoke<UserConfig>("load_config");
}

export function saveConfig(config: UserConfig): Promise<void> {
  return invoke("save_config", { config });
}

export function exportConfig(config: UserConfig): Promise<ConfigFileResult> {
  return invoke<ConfigFileResult>("export_config", { config });
}

export function importConfig(): Promise<ImportedConfigResult> {
  return invoke<ImportedConfigResult>("import_config");
}

export function checkGenerationApi(
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

export function checkPromptOptimizerApi(
  apiKey: string,
  apiBase: string,
  model: string,
  apiMode: string,
  proxyUrl: string
): Promise<ApiCheckResult> {
  return invoke<ApiCheckResult>("check_prompt_optimizer_api", {
    apiKey,
    apiBase,
    model,
    apiMode,
    proxyUrl,
  });
}
