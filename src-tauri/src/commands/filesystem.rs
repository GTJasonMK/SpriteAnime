use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{command, State};

use crate::config::AppState;

static TEMP_VIDEO_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 文件打开结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpenResult {
    pub file_path: String,
    pub file_name: String,
    pub base64_data: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct TempCleanupResult {
    pub removed_files: usize,
    pub removed_dirs: usize,
}

impl TempCleanupResult {
    fn add(&mut self, other: TempCleanupResult) {
        self.removed_files += other.removed_files;
        self.removed_dirs += other.removed_dirs;
    }
}

/// 使用系统对话框选择目录并返回路径
#[command]
pub async fn select_directory(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app.dialog().file().blocking_pick_folder();

    match result {
        Some(path) => Ok(path.to_string()),
        None => Err("用户取消选择".into()),
    }
}

/// 使用系统对话框选择图片文件并返回路径
#[command]
pub async fn open_image_file(app: tauri::AppHandle) -> Result<FileOpenResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app
        .dialog()
        .file()
        .add_filter("图片文件", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .blocking_pick_file();

    match result {
        Some(file_path) => {
            let path_str = file_path.to_string();
            let file_name = std::path::Path::new(&path_str)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            Ok(FileOpenResult {
                file_path: path_str,
                file_name,
                base64_data: String::new(),
            })
        }
        None => Err("用户取消选择".into()),
    }
}

/// 使用系统对话框选择视频文件并返回路径
#[command]
pub async fn open_video_file(app: tauri::AppHandle) -> Result<FileOpenResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app
        .dialog()
        .file()
        .add_filter("视频文件", &["mp4", "webm", "mov", "m4v", "avi", "mkv"])
        .blocking_pick_file();

    match result {
        Some(file_path) => {
            let path_str = file_path.to_string();
            let file_name = std::path::Path::new(&path_str)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            Ok(FileOpenResult {
                file_path: path_str,
                file_name,
                base64_data: String::new(),
            })
        }
        None => Err("用户取消选择".into()),
    }
}

/// 将视频复制到 app 数据目录，规避源路径不在 asset scope 中导致的 WebView 加载失败。
#[command]
pub fn prepare_video_file_for_playback(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<FileOpenResult, String> {
    let source = Path::new(&source_path);
    if !source.exists() {
        return Err("视频文件不存在".into());
    }
    if !source.is_file() {
        return Err("请选择视频文件".into());
    }

    let file_name = source
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "video.mp4".into());
    let extension = source
        .extension()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "mp4".into());
    let stem = source
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "video".into());

    let root = state
        .workbench_records_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("sprite-animte"))
        .join("temp_videos");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时视频目录失败: {}", e))?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_VIDEO_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dest_name = format!(
        "{}_{}_{:04}.{}",
        sanitize_video_temp_name(&stem),
        timestamp,
        nonce % 10_000,
        sanitize_video_temp_name(&extension)
    );
    let dest = root.join(dest_name);
    std::fs::copy(source, &dest).map_err(|e| format!("复制视频到临时目录失败: {}", e))?;
    cleanup_old_temp_videos(&root);

    Ok(FileOpenResult {
        file_path: dest.to_string_lossy().to_string(),
        file_name,
        base64_data: String::new(),
    })
}

/// 删除某个为 WebView 播放复制出来的临时视频文件。
#[command]
pub fn cleanup_prepared_video_file(
    state: State<'_, AppState>,
    path: String,
) -> Result<TempCleanupResult, String> {
    cleanup_path_inside_root(
        &temp_videos_root(&state),
        Path::new(&path),
        TempPathKind::File,
    )
}

/// 删除某个 ffmpeg 视频抽帧批次目录。
#[command]
pub fn cleanup_video_frame_batch_dir(
    state: State<'_, AppState>,
    output_dir: String,
) -> Result<TempCleanupResult, String> {
    cleanup_path_inside_root(
        &temp_video_frames_root(&state),
        Path::new(&output_dir),
        TempPathKind::Dir,
    )
}

/// 清理视频序列帧页面上次运行遗留的临时文件。
#[command]
pub fn cleanup_video_sprite_temp_files(
    state: State<'_, AppState>,
) -> Result<TempCleanupResult, String> {
    let mut summary = TempCleanupResult::default();
    summary.add(cleanup_files_in_root(&temp_videos_root(&state))?);
    summary.add(cleanup_dirs_in_root(&temp_video_frames_root(&state))?);
    Ok(summary)
}

fn app_data_root(state: &AppState) -> PathBuf {
    state
        .workbench_records_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("sprite-animte"))
}

fn temp_videos_root(state: &AppState) -> PathBuf {
    app_data_root(state).join("temp_videos")
}

fn temp_video_frames_root(state: &AppState) -> PathBuf {
    app_data_root(state).join("temp_video_frames")
}

#[derive(Debug, Clone, Copy)]
enum TempPathKind {
    File,
    Dir,
}

fn cleanup_path_inside_root(
    root: &Path,
    path: &Path,
    kind: TempPathKind,
) -> Result<TempCleanupResult, String> {
    if path.as_os_str().is_empty() || !path.exists() {
        return Ok(TempCleanupResult::default());
    }

    let root = root
        .canonicalize()
        .map_err(|e| format!("读取临时根目录失败: {}", e))?;
    let target = path
        .canonicalize()
        .map_err(|e| format!("读取临时路径失败: {}", e))?;

    if target == root || !target.starts_with(&root) {
        return Err("拒绝清理非应用临时路径".into());
    }

    let metadata = std::fs::metadata(&target).map_err(|e| format!("读取临时路径失败: {}", e))?;
    match kind {
        TempPathKind::File => {
            if !metadata.is_file() {
                return Err("临时路径不是文件".into());
            }
            std::fs::remove_file(&target).map_err(|e| format!("删除临时视频失败: {}", e))?;
            Ok(TempCleanupResult {
                removed_files: 1,
                removed_dirs: 0,
            })
        }
        TempPathKind::Dir => {
            if !metadata.is_dir() {
                return Err("临时路径不是目录".into());
            }
            std::fs::remove_dir_all(&target).map_err(|e| format!("删除临时抽帧目录失败: {}", e))?;
            Ok(TempCleanupResult {
                removed_files: 0,
                removed_dirs: 1,
            })
        }
    }
}

fn cleanup_files_in_root(root: &Path) -> Result<TempCleanupResult, String> {
    if !root.exists() {
        return Ok(TempCleanupResult::default());
    }
    let mut summary = TempCleanupResult::default();
    for entry in std::fs::read_dir(root).map_err(|e| format!("读取临时视频目录失败: {}", e))?
    {
        let path = entry
            .map_err(|e| format!("读取临时视频目录项失败: {}", e))?
            .path();
        if path.is_file() {
            std::fs::remove_file(&path).map_err(|e| format!("删除临时视频失败: {}", e))?;
            summary.removed_files += 1;
        }
    }
    Ok(summary)
}

fn cleanup_dirs_in_root(root: &Path) -> Result<TempCleanupResult, String> {
    if !root.exists() {
        return Ok(TempCleanupResult::default());
    }
    let mut summary = TempCleanupResult::default();
    for entry in std::fs::read_dir(root).map_err(|e| format!("读取临时抽帧目录失败: {}", e))?
    {
        let path = entry
            .map_err(|e| format!("读取临时抽帧目录项失败: {}", e))?
            .path();
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| format!("删除临时抽帧目录失败: {}", e))?;
            summary.removed_dirs += 1;
        }
    }
    Ok(summary)
}

fn sanitize_video_temp_name(value: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let sanitized = sanitized
        .trim_matches(|ch: char| ch == '.' || ch == '_' || ch == '-' || ch.is_whitespace())
        .to_string();
    if sanitized.is_empty() {
        "video".into()
    } else {
        sanitized
    }
}

fn cleanup_old_temp_videos(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, PathBuf)> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    files.sort_by_key(|(modified, _)| *modified);

    const MAX_TEMP_VIDEOS: usize = 8;
    if files.len() <= MAX_TEMP_VIDEOS {
        return;
    }
    let remove_count = files.len() - MAX_TEMP_VIDEOS;
    for (_, path) in files.into_iter().take(remove_count) {
        let _ = std::fs::remove_file(path);
    }
}

/// 在系统文件管理器中打开目录
#[command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    opener::open(path).map_err(|e| format!("打开目录失败: {}", e))
}

/// 用系统默认应用打开图片文件
#[command]
pub fn open_image_file_path(path: String) -> Result<(), String> {
    opener::open(path).map_err(|e| format!("打开文件失败: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sprite_anime_{name}_{}_{}",
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn test_cleanup_path_inside_root_deletes_child_file() {
        let root = unique_temp_root("cleanup_file_root");
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("video.webm");
        std::fs::write(&file, b"temp").unwrap();

        let result = cleanup_path_inside_root(&root, &file, TempPathKind::File).unwrap();

        assert_eq!(result.removed_files, 1);
        assert_eq!(result.removed_dirs, 0);
        assert!(!file.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_cleanup_path_inside_root_deletes_child_dir() {
        let root = unique_temp_root("cleanup_dir_root");
        let dir = root.join("video_frames_1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frame_0000.png"), b"temp").unwrap();

        let result = cleanup_path_inside_root(&root, &dir, TempPathKind::Dir).unwrap();

        assert_eq!(result.removed_files, 0);
        assert_eq!(result.removed_dirs, 1);
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_cleanup_path_inside_root_rejects_outside_path() {
        let root = unique_temp_root("cleanup_reject_root");
        let outside_root = unique_temp_root("cleanup_reject_outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside_root).unwrap();
        let file = outside_root.join("video.webm");
        std::fs::write(&file, b"temp").unwrap();

        let err = cleanup_path_inside_root(&root, &file, TempPathKind::File).unwrap_err();

        assert!(err.contains("拒绝清理非应用临时路径"));
        assert!(file.exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside_root);
    }

    #[test]
    fn test_cleanup_path_inside_root_rejects_root_itself() {
        let root = unique_temp_root("cleanup_reject_self");
        std::fs::create_dir_all(&root).unwrap();

        let err = cleanup_path_inside_root(&root, &root, TempPathKind::Dir).unwrap_err();

        assert!(err.contains("拒绝清理非应用临时路径"));
        assert!(root.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
