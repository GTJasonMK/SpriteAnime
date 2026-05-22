use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;
use tauri::{command, State};

use crate::config::AppState;

const FFMPEG_TOOLS_DIR: &str = "tools/ffmpeg";
const FFMPEG_DOWNLOAD_TIMEOUT_SECONDS: u64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegToolStatus {
    pub available: bool,
    pub download_supported: bool,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub install_dir: String,
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

#[derive(Debug, Clone, Copy)]
struct FfmpegArchive {
    url: &'static str,
    file_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
struct FfmpegDownloadManifest {
    platform_dir: &'static str,
    source: &'static str,
    archives: &'static [FfmpegArchive],
}

const WIN64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip",
    file_name: "ffmpeg-master-latest-win64-lgpl.zip",
}];

const WINARM64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-winarm64-lgpl.zip",
    file_name: "ffmpeg-master-latest-winarm64-lgpl.zip",
}];

const LINUX64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-lgpl.tar.xz",
    file_name: "ffmpeg-master-latest-linux64-lgpl.tar.xz",
}];

const LINUXARM64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linuxarm64-lgpl.tar.xz",
    file_name: "ffmpeg-master-latest-linuxarm64-lgpl.tar.xz",
}];

const MACOS_AMD64_ARCHIVES: &[FfmpegArchive] = &[
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffmpeg.zip",
        file_name: "ffmpeg-macos-amd64.zip",
    },
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffprobe.zip",
        file_name: "ffprobe-macos-amd64.zip",
    },
];

const MACOS_ARM64_ARCHIVES: &[FfmpegArchive] = &[
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip",
        file_name: "ffmpeg-macos-arm64.zip",
    },
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffprobe.zip",
        file_name: "ffprobe-macos-arm64.zip",
    },
];

#[command]
pub fn check_ffmpeg_tools(state: State<'_, AppState>) -> FfmpegToolStatus {
    let install_dir = ffmpeg_install_dir(&state).to_string_lossy().to_string();
    let download_supported = current_ffmpeg_download_manifest().is_some();
    let (ffmpeg_path, ffprobe_path) = {
        let config = state.config.lock();
        let ffmpeg_path = non_empty_trimmed(&config.ffmpeg_path).unwrap_or_else(|| "ffmpeg".into());
        let ffprobe_path = non_empty_trimmed(&config.ffprobe_path)
            .or_else(|| derive_ffprobe_path(&ffmpeg_path))
            .unwrap_or_else(|| "ffprobe".into());
        (ffmpeg_path, ffprobe_path)
    };

    let ffmpeg_version = command_version(&ffmpeg_path);
    let ffprobe_version = command_version(&ffprobe_path);
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
        download_supported,
        ffmpeg_path,
        ffprobe_path,
        install_dir,
        message,
        ffmpeg_version: ffmpeg_version.ok(),
        ffprobe_version: ffprobe_version.ok(),
    }
}

#[command]
pub async fn download_ffmpeg_tools(
    state: State<'_, AppState>,
) -> Result<FfmpegToolInstallResult, String> {
    let manifest = current_ffmpeg_download_manifest()
        .ok_or_else(|| "当前系统架构暂不支持自动下载 FFmpeg".to_string())?;
    let proxy_url = {
        let config = state.config.lock();
        non_empty_trimmed(&config.proxy_url)
    };
    let staging_dir = state
        .app_data_dir
        .join("downloads")
        .join(format!("ffmpeg-{}", chrono::Local::now().format("%Y%m%d%H%M%S%f")));
    let install_dir = ffmpeg_install_dir(&state);

    let result = download_and_install_ffmpeg_tools(
        manifest,
        proxy_url.as_deref(),
        &staging_dir,
        &install_dir,
    )
    .await;
    let _ = std::fs::remove_dir_all(&staging_dir);

    let (ffmpeg_path, ffprobe_path) = result?;
    let ffmpeg_path_string = ffmpeg_path.to_string_lossy().to_string();
    let ffprobe_path_string = ffprobe_path.to_string_lossy().to_string();
    let ffmpeg_version = command_version(&ffmpeg_path_string)
        .map_err(|err| format!("FFmpeg 下载完成但无法运行：{err}"))?;
    let ffprobe_version = command_version(&ffprobe_path_string)
        .map_err(|err| format!("FFprobe 下载完成但无法运行：{err}"))?;

    {
        let mut config = state.config.lock();
        config.ffmpeg_path = ffmpeg_path_string.clone();
        config.ffprobe_path = ffprobe_path_string.clone();
        config.save(&state.config_path)?;
    }

    Ok(FfmpegToolInstallResult {
        ffmpeg_path: ffmpeg_path_string,
        ffprobe_path: ffprobe_path_string,
        install_dir: install_dir.to_string_lossy().to_string(),
        source: manifest.source.into(),
        ffmpeg_version,
        ffprobe_version,
    })
}

async fn download_and_install_ffmpeg_tools(
    manifest: FfmpegDownloadManifest,
    proxy_url: Option<&str>,
    staging_dir: &Path,
    install_dir: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    std::fs::create_dir_all(staging_dir)
        .map_err(|e| format!("创建 FFmpeg 下载目录失败: {e}"))?;
    let client = ffmpeg_download_client(proxy_url)?;
    let mut archive_paths = Vec::with_capacity(manifest.archives.len());
    for archive in manifest.archives {
        let path = staging_dir.join(archive.file_name);
        download_archive(&client, archive.url, &path).await?;
        archive_paths.push(path);
    }

    let staging_dir = staging_dir.to_path_buf();
    let install_dir = install_dir.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        install_ffmpeg_archives(&archive_paths, &staging_dir, &install_dir)
    })
    .await
    .map_err(|e| format!("安装 FFmpeg 任务失败: {e}"))?
}

fn ffmpeg_download_client(proxy_url: Option<&str>) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder()
        .user_agent("SpriteAnimte FFmpeg Downloader")
        .timeout(std::time::Duration::from_secs(
            FFMPEG_DOWNLOAD_TIMEOUT_SECONDS,
        ));
    if let Some(proxy_url) = proxy_url {
        builder = builder.proxy(
            reqwest::Proxy::all(proxy_url)
                .map_err(|e| format!("FFmpeg 下载代理配置无效: {e}"))?,
        );
    }
    builder
        .build()
        .map_err(|e| format!("创建 FFmpeg 下载客户端失败: {e}"))
}

async fn download_archive(
    client: &reqwest::Client,
    url: &str,
    output_path: &Path,
) -> Result<(), String> {
    eprintln!("[tools] 下载 FFmpeg: {url}");
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载 FFmpeg 失败: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "下载 FFmpeg 失败，HTTP {status}: {}",
            summarize_text(&body)
        ));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| format!("读取 FFmpeg 下载内容失败: {e}"))?;
    if bytes.is_empty() {
        return Err("FFmpeg 下载内容为空".into());
    }
    std::fs::write(output_path, bytes).map_err(|e| format!("写入 FFmpeg 下载文件失败: {e}"))
}

fn install_ffmpeg_archives(
    archive_paths: &[PathBuf],
    staging_dir: &Path,
    install_dir: &Path,
) -> Result<(PathBuf, PathBuf), String> {
    let extract_dir = staging_dir.join("extracted");
    std::fs::create_dir_all(&extract_dir).map_err(|e| format!("创建 FFmpeg 解压目录失败: {e}"))?;

    for archive_path in archive_paths {
        extract_archive_with_tar(archive_path, &extract_dir)?;
    }

    let ffmpeg_name = executable_name("ffmpeg");
    let ffprobe_name = executable_name("ffprobe");
    let extracted_ffmpeg = find_file_recursively(&extract_dir, &ffmpeg_name)
        .ok_or_else(|| "下载包中未找到 ffmpeg 可执行文件".to_string())?;
    let extracted_ffprobe = find_file_recursively(&extract_dir, &ffprobe_name)
        .ok_or_else(|| "下载包中未找到 ffprobe 可执行文件".to_string())?;

    if install_dir.exists() {
        std::fs::remove_dir_all(install_dir).map_err(|e| format!("清理旧 FFmpeg 目录失败: {e}"))?;
    }
    std::fs::create_dir_all(install_dir).map_err(|e| format!("创建 FFmpeg 安装目录失败: {e}"))?;

    let ffmpeg_path = install_dir.join(&ffmpeg_name);
    let ffprobe_path = install_dir.join(&ffprobe_name);
    std::fs::copy(&extracted_ffmpeg, &ffmpeg_path)
        .map_err(|e| format!("安装 ffmpeg 失败: {e}"))?;
    std::fs::copy(&extracted_ffprobe, &ffprobe_path)
        .map_err(|e| format!("安装 ffprobe 失败: {e}"))?;
    make_executable(&ffmpeg_path)?;
    make_executable(&ffprobe_path)?;

    Ok((ffmpeg_path, ffprobe_path))
}

fn extract_archive_with_tar(archive_path: &Path, output_dir: &Path) -> Result<(), String> {
    let output = Command::new("tar")
        .arg("-xf")
        .arg(archive_path)
        .arg("-C")
        .arg(output_dir)
        .output()
        .map_err(|e| {
            format!(
                "无法解压 FFmpeg 下载包，请确认系统可运行 tar 解压工具: {e}"
            )
        })?;
    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(format!(
        "解压 FFmpeg 下载包失败: {}{}",
        summarize_text(&stderr),
        if stdout.trim().is_empty() {
            String::new()
        } else {
            format!("；{}", summarize_text(&stdout))
        }
    ))
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
        .unwrap_or(command);
    Ok(first_line.to_string())
}

fn ffmpeg_install_dir(state: &AppState) -> PathBuf {
    match current_ffmpeg_download_manifest() {
        Some(manifest) => state.app_data_dir.join(FFMPEG_TOOLS_DIR).join(manifest.platform_dir),
        None => state.app_data_dir.join(FFMPEG_TOOLS_DIR).join("unsupported"),
    }
}

fn current_ffmpeg_download_manifest() -> Option<FfmpegDownloadManifest> {
    ffmpeg_download_manifest_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn ffmpeg_download_manifest_for(os: &str, arch: &str) -> Option<FfmpegDownloadManifest> {
    match (os, arch) {
        ("windows", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "windows-x86_64",
            source: "BtbN FFmpeg LGPL static build",
            archives: WIN64_ARCHIVES,
        }),
        ("windows", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "windows-aarch64",
            source: "BtbN FFmpeg LGPL static build",
            archives: WINARM64_ARCHIVES,
        }),
        ("linux", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "linux-x86_64",
            source: "BtbN FFmpeg LGPL static build",
            archives: LINUX64_ARCHIVES,
        }),
        ("linux", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "linux-aarch64",
            source: "BtbN FFmpeg LGPL static build",
            archives: LINUXARM64_ARCHIVES,
        }),
        ("macos", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "macos-x86_64",
            source: "Martin Riedl FFmpeg static build",
            archives: MACOS_AMD64_ARCHIVES,
        }),
        ("macos", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "macos-aarch64",
            source: "Martin Riedl FFmpeg static build",
            archives: MACOS_ARM64_ARCHIVES,
        }),
        _ => None,
    }
}

fn executable_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn find_file_recursively(root: &Path, file_name: &str) -> Option<PathBuf> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if names_match(name, file_name) {
                return Some(path);
            }
        }
    }
    None
}

fn names_match(actual: &str, expected: &str) -> bool {
    if cfg!(windows) {
        actual.eq_ignore_ascii_case(expected)
    } else {
        actual == expected
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|e| format!("读取可执行文件权限失败: {e}"))?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|e| format!("设置可执行权限失败: {e}"))
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn derive_ffprobe_path(ffmpeg_command: &str) -> Option<String> {
    let path = Path::new(ffmpeg_command);
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_string_lossy();
    if !file_name.starts_with("ffmpeg") {
        return None;
    }

    let candidate = parent.join(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    if candidate.is_file() {
        Some(candidate.to_string_lossy().to_string())
    } else {
        None
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn summarize_text(text: &str) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= 280 {
        return compact;
    }
    compact.chars().take(280).collect::<String>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ffmpeg_download_manifest_supports_main_desktop_targets() {
        let linux = ffmpeg_download_manifest_for("linux", "x86_64").unwrap();
        assert_eq!(linux.platform_dir, "linux-x86_64");
        assert!(linux.archives[0].file_name.ends_with(".tar.xz"));

        let windows = ffmpeg_download_manifest_for("windows", "x86_64").unwrap();
        assert_eq!(windows.platform_dir, "windows-x86_64");
        assert!(windows.archives[0].file_name.ends_with(".zip"));

        let macos = ffmpeg_download_manifest_for("macos", "aarch64").unwrap();
        assert_eq!(macos.platform_dir, "macos-aarch64");
        assert_eq!(macos.archives.len(), 2);
    }

    #[test]
    fn ffmpeg_download_manifest_rejects_unsupported_arch() {
        assert!(ffmpeg_download_manifest_for("linux", "riscv64").is_none());
    }
}
