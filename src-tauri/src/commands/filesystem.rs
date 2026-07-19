use serde::Serialize;
use std::path::{Path, PathBuf};
use tauri::{command, State};

use crate::asset_library::{self, AssetCategory};
use crate::config::AppState;
use crate::path_safety::required_file_name;
use crate::runtime::{DataLock, LockDomain};

/// 文件打开结果
#[derive(Debug, Clone, Serialize)]
pub struct FileOpenResult {
    pub file_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct TempCleanupResult {
    pub removed_dirs: usize,
}

/// 使用系统对话框选择图片文件并返回路径
// 保持命令异步：对话框插件的阻塞选择器不能运行在 WebView 主线程。
#[command]
pub async fn open_image_file(app: tauri::AppHandle) -> Result<FileOpenResult, String> {
    pick_media_file(
        &app,
        "图片文件",
        &["png", "jpg", "jpeg", "webp", "gif", "bmp"],
    )
}

/// 使用系统对话框选择视频文件并返回路径
#[command]
pub async fn open_video_file(app: tauri::AppHandle) -> Result<FileOpenResult, String> {
    pick_media_file(
        &app,
        "视频文件",
        &["mp4", "webm", "mov", "m4v", "avi", "mkv"],
    )
}

fn pick_media_file(
    app: &tauri::AppHandle,
    label: &str,
    extensions: &[&str],
) -> Result<FileOpenResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app
        .dialog()
        .file()
        .add_filter(label, extensions)
        .blocking_pick_file();

    match result {
        Some(file_path) => {
            let path_str = file_path.to_string();
            let file_name = required_file_name(
                Path::new(&path_str),
                label,
                "请重新选择一个带文件名的本地文件，不要选择磁盘根目录、目录或虚拟路径。",
            )?;

            Ok(FileOpenResult {
                file_path: path_str,
                file_name,
            })
        }
        None => Err("用户取消选择".into()),
    }
}

#[command]
pub fn import_image_to_library(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<FileOpenResult, String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)
        .map_err(|error| error.to_string())?;
    import_file_to_library(
        state,
        source_path,
        AssetCategory::ImportedImages,
        "图片素材",
    )
}

#[command]
pub fn import_video_to_library(
    state: State<'_, AppState>,
    source_path: String,
) -> Result<FileOpenResult, String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)
        .map_err(|error| error.to_string())?;
    import_file_to_library(
        state,
        source_path,
        AssetCategory::OriginalVideos,
        "视频素材",
    )
}

fn import_file_to_library(
    state: State<'_, AppState>,
    source_path: String,
    category: AssetCategory,
    context: &str,
) -> Result<FileOpenResult, String> {
    import_file_to_library_inner(&state, source_path, category, context)
}

pub(crate) fn import_file_to_library_inner(
    state: &AppState,
    source_path: String,
    category: AssetCategory,
    context: &str,
) -> Result<FileOpenResult, String> {
    let dest =
        asset_library::copy_file_to_category(&source_path, &state.default_save_dir, category)?;
    let file_name = required_file_name(
        &dest,
        context,
        "请重新选择一个带文件名的本地文件，不要选择磁盘根目录、目录或虚拟路径。",
    )?;

    Ok(FileOpenResult {
        file_path: dest.to_string_lossy().to_string(),
        file_name,
    })
}

/// 删除某个 ffmpeg 视频抽帧批次目录。
#[command]
pub fn cleanup_video_frame_batch_dir(
    state: State<'_, AppState>,
    output_dir: String,
) -> Result<TempCleanupResult, String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)
        .map_err(|error| error.to_string())?;
    cleanup_dir_inside_root(&temp_video_frames_root(&state), Path::new(&output_dir))
}

/// 清理视频序列帧页面上次运行遗留的临时文件。
#[command]
pub fn cleanup_video_sprite_temp_files(
    state: State<'_, AppState>,
) -> Result<TempCleanupResult, String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)
        .map_err(|error| error.to_string())?;
    cleanup_dirs_in_root(&temp_video_frames_root(&state))
}

fn temp_video_frames_root(state: &AppState) -> PathBuf {
    state.app_data_dir.join("temp_video_frames")
}

fn cleanup_dir_inside_root(root: &Path, path: &Path) -> Result<TempCleanupResult, String> {
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
    if !metadata.is_dir() {
        return Err("临时路径不是目录".into());
    }
    std::fs::remove_dir_all(&target).map_err(|e| format!("删除临时抽帧目录失败: {}", e))?;
    Ok(TempCleanupResult { removed_dirs: 1 })
}

pub(crate) fn cleanup_dirs_in_root(root: &Path) -> Result<TempCleanupResult, String> {
    if !root.exists() {
        return Ok(TempCleanupResult::default());
    }
    let mut summary = TempCleanupResult::default();
    for entry in std::fs::read_dir(root).map_err(|e| format!("读取临时抽帧目录失败: {}", e))?
    {
        let entry = entry.map_err(|e| format!("读取临时抽帧目录项失败: {}", e))?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|e| format!("读取临时抽帧目录项元数据失败: {}", e))?;
        if metadata.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| format!("删除临时抽帧目录失败: {}", e))?;
            summary.removed_dirs += 1;
        }
    }
    Ok(summary)
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
    fn test_cleanup_dir_inside_root_deletes_child_dir() {
        let root = unique_temp_root("cleanup_dir_root");
        let dir = root.join("video_frames_1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frame_0000.png"), b"temp").unwrap();

        let result = cleanup_dir_inside_root(&root, &dir).unwrap();

        assert_eq!(result.removed_dirs, 1);
        assert!(!dir.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn test_cleanup_dir_inside_root_rejects_outside_dir() {
        let root = unique_temp_root("cleanup_reject_root");
        let outside_root = unique_temp_root("cleanup_reject_outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside_root).unwrap();
        let dir = outside_root.join("video_frames_1");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("frame_0000.png"), b"temp").unwrap();

        let err = cleanup_dir_inside_root(&root, &dir).unwrap_err();

        assert!(err.contains("拒绝清理非应用临时路径"));
        assert!(dir.exists());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(outside_root);
    }

    #[test]
    fn test_cleanup_dir_inside_root_rejects_root_itself() {
        let root = unique_temp_root("cleanup_reject_self");
        std::fs::create_dir_all(&root).unwrap();

        let err = cleanup_dir_inside_root(&root, &root).unwrap_err();

        assert!(err.contains("拒绝清理非应用临时路径"));
        assert!(root.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
