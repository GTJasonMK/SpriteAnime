import { invoke } from "@tauri-apps/api/core";

export interface FfmpegToolStatus {
  available: boolean;
  ffmpeg_path: string;
  ffprobe_path: string;
  message: string;
  ffmpeg_version?: string | null;
  ffprobe_version?: string | null;
}

export interface FfmpegToolInstallResult {
  ffmpeg_path: string;
  ffprobe_path: string;
  install_dir: string;
  source: string;
  ffmpeg_version: string;
  ffprobe_version: string;
}

export function checkFfmpegTools(): Promise<FfmpegToolStatus> {
  return invoke<FfmpegToolStatus>("check_ffmpeg_tools");
}

export function downloadFfmpegTools(proxyUrl: string): Promise<FfmpegToolInstallResult> {
  return invoke<FfmpegToolInstallResult>("download_ffmpeg_tools", { proxyUrl });
}
