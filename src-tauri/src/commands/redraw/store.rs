use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::config::AppState;
use crate::path_safety::sanitize_file_name_component;

use super::types::{RedrawBatchRecord, RedrawRunManifest};

pub(super) const MAX_BATCH_ERROR_CHARS: usize = 4_000;

pub(super) fn validate_range(value: u32, min: u32, max: u32, label: &str) -> Result<(), String> {
    if value < min || value > max {
        Err(format!("{label}必须在 {min} 到 {max} 之间，实际为 {value}"))
    } else {
        Ok(())
    }
}

pub(super) fn active_run_dir(state: &AppState) -> PathBuf {
    state.app_data_dir.join("video-sprite-runs").join("active")
}

pub(super) fn validate_file_inside(root: &Path, path: &Path, context: &str) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("{context}不存在或不是普通文件"));
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("读取{context}根目录失败: {e}"))?;
    let target = path
        .canonicalize()
        .map_err(|e| format!("读取{context}路径失败: {e}"))?;
    if !target.starts_with(&root) {
        return Err(format!("拒绝读取运行目录之外的{context}"));
    }
    Ok(())
}

pub(super) fn truncate_with_marker(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}…（错误信息已截断）")
    } else {
        truncated
    }
}

fn manifest_path(active_dir: &Path) -> PathBuf {
    active_dir.join("manifest.json")
}

fn pause_request_path(active_dir: &Path) -> PathBuf {
    active_dir.join("pause.requested")
}

pub(super) fn request_pause(active_dir: &Path) -> Result<(), String> {
    std::fs::write(pause_request_path(active_dir), b"pause")
        .map_err(|error| format!("写入重绘暂停请求失败: {error}"))
}

pub(super) fn clear_pause_request(active_dir: &Path) -> Result<(), String> {
    let path = pause_request_path(active_dir);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("清除重绘暂停请求失败: {error}")),
    }
}

pub(super) fn take_pause_request(active_dir: &Path) -> Result<bool, String> {
    let path = pause_request_path(active_dir);
    if !path.is_file() {
        return Ok(false);
    }
    std::fs::remove_file(path).map_err(|error| format!("读取重绘暂停请求失败: {error}"))?;
    Ok(true)
}

pub(super) fn load_manifest_if_exists(
    active_dir: &Path,
) -> Result<Option<RedrawRunManifest>, String> {
    let path = manifest_path(active_dir);
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(format!("读取分组重绘运行失败: {err}")),
    };
    let manifest =
        serde_json::from_str(&content).map_err(|e| format!("分组重绘 manifest JSON 无效: {e}"))?;
    Ok(Some(manifest))
}

pub(super) fn load_required_manifest(
    active_dir: &Path,
    run_id: &str,
) -> Result<RedrawRunManifest, String> {
    let manifest =
        load_manifest_if_exists(active_dir)?.ok_or_else(|| "没有活动的分组重绘运行".to_string())?;
    if run_id.trim().is_empty() || manifest.id != run_id.trim() {
        return Err("活动运行 ID 不匹配，请重新加载当前运行".into());
    }
    Ok(manifest)
}

pub(super) fn save_manifest(active_dir: &Path, manifest: &RedrawRunManifest) -> Result<(), String> {
    std::fs::create_dir_all(active_dir).map_err(|e| format!("创建分组重绘运行目录失败: {e}"))?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("序列化分组重绘运行失败: {e}"))?;
    let path = manifest_path(active_dir);
    let temp = active_dir.join("manifest.json.tmp");
    std::fs::write(&temp, json).map_err(|e| format!("写入临时 manifest 失败: {e}"))?;
    match std::fs::rename(&temp, &path) {
        Ok(()) => Ok(()),
        Err(err)
            if err.kind() == ErrorKind::AlreadyExists
                || err.kind() == ErrorKind::PermissionDenied =>
        {
            std::fs::remove_file(&path).map_err(|e| format!("替换旧 manifest 失败: {e}"))?;
            std::fs::rename(&temp, &path).map_err(|e| format!("提交新 manifest 失败: {e}"))
        }
        Err(err) => Err(format!("提交新 manifest 失败: {err}")),
    }
}

pub(crate) fn remove_active_run_dir(state: &AppState) -> Result<(), String> {
    let runs_root = state.app_data_dir.join("video-sprite-runs");
    let active = runs_root.join("active");
    if !active.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&runs_root).map_err(|e| format!("创建运行根目录失败: {e}"))?;
    let root = runs_root
        .canonicalize()
        .map_err(|e| format!("读取运行根目录失败: {e}"))?;
    let target = active
        .canonicalize()
        .map_err(|e| format!("读取活动运行目录失败: {e}"))?;
    if target == root || !target.starts_with(&root) {
        return Err("拒绝删除活动运行根目录之外的路径".into());
    }
    std::fs::remove_dir_all(target).map_err(|e| format!("删除活动运行失败: {e}"))
}

pub(super) fn required_batch_mut(
    manifest: &mut RedrawRunManifest,
    batch_index: u32,
) -> Result<&mut RedrawBatchRecord, String> {
    manifest
        .batches
        .get_mut(batch_index as usize)
        .ok_or_else(|| format!("第{}批不存在", batch_index + 1))
}

pub(super) fn safe_output_prefix(source_name: &str) -> Result<String, String> {
    let stem = Path::new(source_name)
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| "视频名称缺少文件名".to_string())?;
    let sanitized = sanitize_file_name_component(&stem);
    if sanitized.is_empty() {
        return Err("视频名称清洗后为空".into());
    }
    Ok(sanitized)
}
