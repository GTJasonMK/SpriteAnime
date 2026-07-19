mod download;
mod manifest;
mod text;

#[cfg(test)]
use download::format_ffmpeg_download_error_body_read_failure;
use manifest::current_ffmpeg_download_manifest;
#[cfg(test)]
use manifest::ffmpeg_download_manifest_for;
use text::{non_empty_trimmed, summarize_text};

use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use tauri::{command, State};

use crate::config::{AppState, UserConfig};
use crate::runtime::{DataLock, LockDomain};

const FFMPEG_TOOLS_DIR: &str = "tools/ffmpeg";

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegToolStatus {
    pub available: bool,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub message: String,
    pub ffmpeg_version: Option<String>,
    pub ffprobe_version: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegToolInstallResult {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub install_dir: String,
    pub source: String,
    pub ffmpeg_version: String,
    pub ffprobe_version: String,
}

#[derive(Debug, Clone)]
pub struct ConfiguredFfmpegTools {
    pub ffmpeg: String,
    pub ffprobe: String,
}

#[command]
pub fn check_ffmpeg_tools(state: State<'_, AppState>) -> Result<FfmpegToolStatus, String> {
    let _tools_lock =
        DataLock::shared(&state.locks_dir, LockDomain::Tools).map_err(|error| error.to_string())?;
    let _config_lock = DataLock::shared(&state.locks_dir, LockDomain::Config)
        .map_err(|error| error.to_string())?;
    Ok(check_ffmpeg_tools_inner(&state))
}

pub(crate) fn check_ffmpeg_tools_inner(state: &AppState) -> FfmpegToolStatus {
    let download_supported = current_ffmpeg_download_manifest().is_some();
    let configured_tools = {
        let config = state.config.lock();
        configured_ffmpeg_tools(&config)
    };
    let tools = match configured_tools {
        Ok(tools) => tools,
        Err(message) => {
            return FfmpegToolStatus {
                available: false,
                ffmpeg_path: String::new(),
                ffprobe_path: String::new(),
                message,
                ffmpeg_version: None,
                ffprobe_version: None,
            };
        }
    };

    let ffmpeg_version = command_version(&tools.ffmpeg);
    let ffprobe_version = command_version(&tools.ffprobe);
    let available = ffmpeg_version.is_ok() && ffprobe_version.is_ok();
    let message = match (&ffmpeg_version, &ffprobe_version, download_supported) {
        (Ok(ffmpeg), Ok(ffprobe), _) => {
            format!("FFmpeg 可用。{ffmpeg}；{ffprobe}")
        }
        (Err(ffmpeg_error), Ok(_), true) => {
            format!("未检测到 FFmpeg：{ffmpeg_error}。可点击下载并自动配置。")
        }
        (Ok(_), Err(ffprobe_error), true) => {
            format!("未检测到 FFprobe：{ffprobe_error}。可点击下载并自动配置。")
        }
        (Err(ffmpeg_error), Err(ffprobe_error), true) => {
            format!(
                "未检测到 FFmpeg/FFprobe：{ffmpeg_error}；{ffprobe_error}。可点击下载并自动配置。"
            )
        }
        (_, _, false) => "当前系统架构暂不支持自动下载，请手动填写 FFmpeg/FFprobe 路径。".into(),
    };

    FfmpegToolStatus {
        available,
        ffmpeg_path: tools.ffmpeg,
        ffprobe_path: tools.ffprobe,
        message,
        ffmpeg_version: ffmpeg_version.ok(),
        ffprobe_version: ffprobe_version.ok(),
    }
}

pub fn configured_ffmpeg_tools(config: &UserConfig) -> Result<ConfiguredFfmpegTools, String> {
    let ffmpeg = non_empty_trimmed(&config.ffmpeg_path);
    let ffprobe = non_empty_trimmed(&config.ffprobe_path);
    match (ffmpeg, ffprobe) {
        (Some(ffmpeg), Some(ffprobe)) => Ok(ConfiguredFfmpegTools { ffmpeg, ffprobe }),
        (None, None) => Err("FFmpeg/FFprobe 路径未配置。请在 设置 > 输出与工具 同时填写 FFmpeg 和 FFprobe 可执行文件路径，或点击“下载并配置”自动安装后重试。".into()),
        (None, Some(_)) => Err("FFmpeg 路径未配置。请在 设置 > 输出与工具 填写 FFmpeg 可执行文件路径，或点击“下载并配置”自动安装后重试。".into()),
        (Some(_), None) => Err("FFprobe 路径未配置。请在 设置 > 输出与工具 填写 FFprobe 可执行文件路径，或点击“下载并配置”自动安装后重试。".into()),
    }
}

#[command]
pub async fn download_ffmpeg_tools(
    state: State<'_, AppState>,
    proxy_url: String,
) -> Result<FfmpegToolInstallResult, String> {
    let _tools_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Tools)
        .map_err(|error| error.to_string())?;
    let _config_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Config)
        .map_err(|error| error.to_string())?;
    download::download_ffmpeg_tools(&state, proxy_url).await
}

pub(crate) async fn download_ffmpeg_tools_inner(
    state: &AppState,
    proxy_url: String,
) -> Result<FfmpegToolInstallResult, String> {
    download::download_ffmpeg_tools(state, proxy_url).await
}

fn command_version(command: &str) -> Result<String, String> {
    let output = Command::new(command)
        .arg("-version")
        .output()
        .map_err(|e| format!("无法运行 {command}: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "{command} 返回失败状态: {}",
            summarize_text(&String::from_utf8_lossy(&output.stderr))
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let first_line = text
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or_else(|| format!("{command} 未返回版本信息"))?;
    Ok(first_line.to_string())
}

fn ffmpeg_install_dir(state: &AppState, platform_dir: &str) -> PathBuf {
    state.app_data_dir.join(FFMPEG_TOOLS_DIR).join(platform_dir)
}

#[cfg(test)]
mod tests;
