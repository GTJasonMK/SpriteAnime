use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::AppState;

use super::manifest::{current_ffmpeg_download_manifest, executable_name, FfmpegDownloadManifest};
use super::text::{non_empty_trimmed, summarize_text};
use super::{command_version, ffmpeg_install_dir, FfmpegToolInstallResult};

const FFMPEG_DOWNLOAD_TIMEOUT_SECONDS: u64 = 600;

pub(super) async fn download_ffmpeg_tools(
    state: &AppState,
    proxy_url: String,
) -> Result<FfmpegToolInstallResult, String> {
    let manifest = current_ffmpeg_download_manifest()
        .ok_or_else(|| "当前系统架构暂不支持自动下载 FFmpeg".to_string())?;
    let proxy_url = non_empty_trimmed(&proxy_url);
    let staging_dir = state.app_data_dir.join("downloads").join(format!(
        "ffmpeg-{}",
        chrono::Local::now().format("%Y%m%d%H%M%S%f")
    ));
    let install_dir = ffmpeg_install_dir(state, manifest.platform_dir);

    let result = download_and_install_ffmpeg_tools(
        manifest,
        proxy_url.as_deref(),
        &staging_dir,
        &install_dir,
    )
    .await;
    let cleanup_result =
        std::fs::remove_dir_all(&staging_dir).map_err(|e| format!("清理 FFmpeg 下载目录失败: {e}"));
    let (ffmpeg_path, ffprobe_path) = match (result, cleanup_result) {
        (Ok(paths), Ok(())) => paths,
        (Err(err), Ok(())) => return Err(err),
        (Ok(_), Err(cleanup_err)) => return Err(cleanup_err),
        (Err(err), Err(cleanup_err)) => return Err(format!("{err}; {cleanup_err}")),
    };
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
    std::fs::create_dir_all(staging_dir).map_err(|e| format!("创建 FFmpeg 下载目录失败: {e}"))?;
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
            reqwest::Proxy::all(proxy_url).map_err(|e| format!("FFmpeg 下载代理配置无效: {e}"))?,
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
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载 FFmpeg 失败: {e}"))?;
    let status = response.status();
    if !status.is_success() {
        let body = read_ffmpeg_download_error_body(response, status).await?;
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

async fn read_ffmpeg_download_error_body(
    response: reqwest::Response,
    status: reqwest::StatusCode,
) -> Result<String, String> {
    response
        .text()
        .await
        .map_err(|e| format_ffmpeg_download_error_body_read_failure(status, &e.to_string()))
}

pub(super) fn format_ffmpeg_download_error_body_read_failure(
    status: reqwest::StatusCode,
    detail: &str,
) -> String {
    format!(
        "下载 FFmpeg 失败，HTTP {status}，且读取错误响应体失败：{detail}。解决方法：请检查网络连接、代理配置和下载源是否提前断开连接后重试。"
    )
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
    let extracted_ffmpeg = find_file_recursively(&extract_dir, &ffmpeg_name)?
        .ok_or_else(|| "下载包中未找到 ffmpeg 可执行文件".to_string())?;
    let extracted_ffprobe = find_file_recursively(&extract_dir, &ffprobe_name)?
        .ok_or_else(|| "下载包中未找到 ffprobe 可执行文件".to_string())?;

    if install_dir.exists() {
        std::fs::remove_dir_all(install_dir).map_err(|e| format!("清理旧 FFmpeg 目录失败: {e}"))?;
    }
    std::fs::create_dir_all(install_dir).map_err(|e| format!("创建 FFmpeg 安装目录失败: {e}"))?;

    let ffmpeg_path = install_dir.join(&ffmpeg_name);
    let ffprobe_path = install_dir.join(&ffprobe_name);
    std::fs::copy(&extracted_ffmpeg, &ffmpeg_path).map_err(|e| format!("安装 ffmpeg 失败: {e}"))?;
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
        .map_err(|e| format!("无法解压 FFmpeg 下载包，请确认系统可运行 tar 解压工具: {e}"))?;
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

fn find_file_recursively(root: &Path, file_name: &str) -> Result<Option<PathBuf>, String> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir)
            .map_err(|e| format!("读取 FFmpeg 解压目录 {} 失败: {e}", dir.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("读取 FFmpeg 解压目录项 {} 失败: {e}", dir.display()))?;
            let path = entry.path();
            let metadata = entry
                .metadata()
                .map_err(|e| format!("读取 FFmpeg 解压文件 {} 元数据失败: {e}", path.display()))?;
            if metadata.is_dir() {
                stack.push(path);
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if names_match(name, file_name) {
                return Ok(Some(path));
            }
        }
    }
    Ok(None)
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
