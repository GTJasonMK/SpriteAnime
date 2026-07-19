use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::ErrorKind;
use std::path::Path;
use tauri::{command, State};

use crate::config::AppState;
use crate::runtime::{DataLock, LockDomain};

const WORKSPACE_SCHEMA_VERSION: u32 = 3;
const MAX_WORKSPACE_JSON_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WorkspaceSnapshot {
    pub schema_version: u32,
    pub task: Option<WorkspaceTaskSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum WorkspaceTaskSnapshot {
    Image { stage: ImageTaskStage, data: Value },
    Video { stage: VideoTaskStage, data: Value },
    Sprite { stage: SpriteTaskStage, data: Value },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageTaskStage {
    Source,
    Matting,
    Grid,
    Bounds,
    Preview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VideoTaskStage {
    Source,
    Range,
    Extract,
    Redraw,
    Preview,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpriteTaskStage {
    Source,
    Grid,
    Bounds,
    Preview,
}

#[command]
pub fn read_workspace_snapshot(
    state: State<'_, AppState>,
) -> Result<Option<WorkspaceSnapshot>, String> {
    let _lock = DataLock::shared(&state.locks_dir, LockDomain::Workspace)
        .map_err(|error| error.to_string())?;
    read_snapshot(&state.workspace_path, &state.app_data_dir)
}

#[command]
pub fn save_workspace_snapshot(
    state: State<'_, AppState>,
    snapshot: WorkspaceSnapshot,
) -> Result<(), String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Workspace)
        .map_err(|error| error.to_string())?;
    save_snapshot(&state.workspace_path, &state.app_data_dir, &snapshot)
}

#[command]
pub fn save_workspace_image_data_url(
    state: State<'_, AppState>,
    slot: String,
    data_url: String,
) -> Result<String, String> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Workspace)
        .map_err(|error| error.to_string())?;
    let file_name = match slot.as_str() {
        "generator-matting" => "generator-matting.png",
        _ => return Err(format!("工作区图片槽位无效：{slot}")),
    };
    let payload = crate::image_processor::require_image_data_url_payload(&data_url, "工作区图片")?;
    let image = crate::image_processor::base64_to_image(payload)?;
    let dir = state.app_data_dir.join("workspace-assets");
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建工作区素材目录失败: {e}"))?;
    let target = dir.join(file_name);
    let temp = dir.join(format!("{file_name}.tmp"));
    image
        .save_with_format(&temp, image::ImageFormat::Png)
        .map_err(|e| format!("写入工作区图片临时文件失败: {e}"))?;
    replace_atomically(&temp, &target, "工作区图片")?;
    Ok(target.to_string_lossy().to_string())
}

#[command]
pub fn reset_workspace(state: State<'_, AppState>) -> Result<(), String> {
    reset_workspace_inner(&state)
}

fn reset_workspace_inner(state: &AppState) -> Result<(), String> {
    let _workspace_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Workspace)
        .map_err(|error| error.to_string())?;
    let _redraw_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Redraw)
        .map_err(|error| error.to_string())?;

    remove_path_if_exists(&state.workspace_path, "工作区快照")?;
    remove_path_if_exists(
        &state.workspace_path.with_extension("json.tmp"),
        "工作区临时快照",
    )?;
    remove_dir_if_exists(
        &state.app_data_dir.join("workspace-assets"),
        "工作区图片目录",
    )?;
    crate::commands::filesystem::cleanup_dirs_in_root(
        &state.app_data_dir.join("temp_video_frames"),
    )?;
    crate::commands::redraw::remove_active_run_dir_for_workspace_reset(state)?;
    Ok(())
}

#[command]
pub fn reveal_workspace_snapshot(state: State<'_, AppState>) -> Result<(), String> {
    let parent = state
        .workspace_path
        .parent()
        .ok_or_else(|| "工作区快照路径缺少父目录".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| format!("创建工作区目录失败: {error}"))?;
    opener::open(parent).map_err(|error| format!("打开工作区目录失败: {error}"))
}

pub(crate) fn read_snapshot(
    path: &Path,
    app_data_dir: &Path,
) -> Result<Option<WorkspaceSnapshot>, String> {
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(workspace_read_error(path, &format!("读取失败：{err}"))),
    };
    if content.len() > MAX_WORKSPACE_JSON_BYTES {
        return Err(workspace_read_error(path, "文件超过 2 MiB 限制"));
    }
    let value: Value = serde_json::from_str(&content)
        .map_err(|e| workspace_read_error(path, &format!("JSON 解析失败：{e}")))?;
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or_else(|| workspace_read_error(path, "缺少整数 schemaVersion"))?;
    if schema_version != u64::from(WORKSPACE_SCHEMA_VERSION) {
        return Err(workspace_read_error(
            path,
            &format!(
                "工作区 schemaVersion 必须为 {WORKSPACE_SCHEMA_VERSION}，实际为 {schema_version}"
            ),
        ));
    }
    let snapshot: WorkspaceSnapshot = serde_json::from_value(value)
        .map_err(|e| workspace_read_error(path, &format!("JSON 结构无效：{e}")))?;
    validate_snapshot(&snapshot, app_data_dir).map_err(|e| workspace_read_error(path, &e))?;
    Ok(Some(snapshot))
}

fn save_snapshot(
    path: &Path,
    app_data_dir: &Path,
    snapshot: &WorkspaceSnapshot,
) -> Result<(), String> {
    validate_snapshot(snapshot, app_data_dir)?;
    let json =
        serde_json::to_string_pretty(snapshot).map_err(|e| format!("序列化工作区快照失败: {e}"))?;
    if json.len() > MAX_WORKSPACE_JSON_BYTES {
        return Err("工作区快照超过 2 MiB 限制".into());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "工作区快照路径缺少父目录".to_string())?;
    std::fs::create_dir_all(parent).map_err(|e| format!("创建工作区目录失败: {e}"))?;
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, json).map_err(|e| format!("写入工作区临时快照失败: {e}"))?;
    replace_atomically(&temp, path, "工作区快照")
}

fn validate_snapshot(snapshot: &WorkspaceSnapshot, app_data_dir: &Path) -> Result<(), String> {
    if snapshot.schema_version != WORKSPACE_SCHEMA_VERSION {
        return Err(format!(
            "工作区 schemaVersion 必须为 {WORKSPACE_SCHEMA_VERSION}，实际为 {}",
            snapshot.schema_version
        ));
    }
    if let Some(task) = &snapshot.task {
        let data = match task {
            WorkspaceTaskSnapshot::Image { data, .. }
            | WorkspaceTaskSnapshot::Video { data, .. }
            | WorkspaceTaskSnapshot::Sprite { data, .. } => data,
        };
        validate_paths(data, app_data_dir, "task.data")?;
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path, context: &str) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除{context}失败: {error}")),
    }
}

fn remove_dir_if_exists(path: &Path, context: &str) -> Result<(), String> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("删除{context}失败: {error}")),
    }
}

fn validate_paths(value: &Value, root: &Path, context: &str) -> Result<(), String> {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if key.ends_with("Path") {
                    validate_path_value(child, root, &format!("{context}.{key}"))?;
                } else {
                    validate_paths(child, root, &format!("{context}.{key}"))?;
                }
            }
        }
        Value::Array(items) => {
            for (index, child) in items.iter().enumerate() {
                validate_paths(child, root, &format!("{context}[{index}]"))?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_path_value(value: &Value, root: &Path, context: &str) -> Result<(), String> {
    let Some(path) = value.as_str() else {
        if value.is_null() {
            return Ok(());
        }
        return Err(format!("{context} 必须是字符串或 null"));
    };
    if path.is_empty() {
        return Ok(());
    }
    let target = Path::new(path);
    if !target.is_file() {
        return Err(format!("{context} 指向的文件不存在：{path}"));
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("读取应用数据目录失败: {e}"))?;
    let target = target
        .canonicalize()
        .map_err(|e| format!("读取 {context} 路径失败: {e}"))?;
    if !target.starts_with(root) {
        return Err(format!("{context} 不在应用数据目录内"));
    }
    Ok(())
}

fn replace_atomically(temp: &Path, target: &Path, context: &str) -> Result<(), String> {
    std::fs::rename(temp, target).map_err(|e| format!("提交{context}失败: {e}"))
}

fn workspace_read_error(path: &Path, reason: &str) -> String {
    format!(
        "读取工作区快照失败：{}。原因：{}。解决方法：可打开工作区目录备份并修复该文件；如果不需要恢复，请在恢复对话框中重置工作区。",
        path.display(),
        reason
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("sprite_workspace_{}_{}", std::process::id(), stamp))
    }

    fn snapshot(path: &Path) -> WorkspaceSnapshot {
        WorkspaceSnapshot {
            schema_version: 3,
            task: Some(WorkspaceTaskSnapshot::Video {
                stage: VideoTaskStage::Range,
                data: json!({"sourcePath": path.to_string_lossy()}),
            }),
        }
    }

    #[test]
    fn saves_and_reads_current_workspace_atomically() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source.mp4");
        std::fs::write(&source, b"video").unwrap();
        let path = root.join("workspace.json");
        save_snapshot(&path, &root, &snapshot(&source)).unwrap();
        let mut updated = snapshot(&source);
        updated.task = Some(WorkspaceTaskSnapshot::Sprite {
            stage: SpriteTaskStage::Grid,
            data: json!({"sheetImagePath": source.to_string_lossy()}),
        });
        save_snapshot(&path, &root, &updated).unwrap();
        let restored = read_snapshot(&path, &root).unwrap().unwrap();
        assert!(matches!(
            restored.task,
            Some(WorkspaceTaskSnapshot::Sprite {
                stage: SpriteTaskStage::Grid,
                ..
            })
        ));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_paths_outside_application_data() {
        let root = temp_dir();
        let outside = temp_dir().with_extension("png");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, b"image").unwrap();
        let err = validate_snapshot(&snapshot(&outside), &root).unwrap_err();
        assert!(err.contains("不在应用数据目录内"));
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_file(outside);
    }

    #[test]
    fn rejects_unknown_workspace_schema_version() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let mut value = snapshot(Path::new(""));
        value.schema_version = 2;
        let err = validate_snapshot(&value, &root).unwrap_err();
        assert!(err.contains("schemaVersion 必须为 3"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn read_rejects_v2_without_parsing_legacy_fields() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("workspace.json");
        std::fs::write(
            &path,
            r#"{"schemaVersion":2,"activeTab":"generator","generator":{},"videoSprite":{},"sprite":{}}"#,
        )
        .unwrap();

        let err = read_snapshot(&path, &root).unwrap_err();

        assert!(err.contains("schemaVersion 必须为 3，实际为 2"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reads_empty_v3_workspace() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("workspace.json");
        std::fs::write(&path, r#"{"schemaVersion":3,"task":null}"#).unwrap();

        let restored = read_snapshot(&path, &root).unwrap().unwrap();

        assert!(restored.task.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unknown_fields() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("workspace.json");
        std::fs::write(&path, r#"{"schemaVersion":3,"task":null,"legacy":true}"#).unwrap();

        let err = read_snapshot(&path, &root).unwrap_err();

        assert!(err.contains("JSON 结构无效"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_stage_from_another_task_kind() {
        let root = temp_dir();
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("workspace.json");
        std::fs::write(
            &path,
            r#"{"schemaVersion":3,"task":{"kind":"image","stage":"extract","data":{}}}"#,
        )
        .unwrap();

        let err = read_snapshot(&path, &root).unwrap_err();

        assert!(err.contains("JSON 结构无效"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reset_removes_work_state_and_preserves_saved_assets() {
        let root = temp_dir();
        let state = crate::runtime::create_app_state(Some(root.clone())).unwrap();
        std::fs::write(&state.workspace_path, "{}").unwrap();
        let temporary_snapshot = state.workspace_path.with_extension("json.tmp");
        std::fs::write(&temporary_snapshot, "{}").unwrap();
        let workspace_asset = state.app_data_dir.join("workspace-assets/matting.png");
        std::fs::create_dir_all(workspace_asset.parent().unwrap()).unwrap();
        std::fs::write(&workspace_asset, b"draft").unwrap();
        let temp_frame = state.app_data_dir.join("temp_video_frames/batch/frame.png");
        std::fs::create_dir_all(temp_frame.parent().unwrap()).unwrap();
        std::fs::write(&temp_frame, b"frame").unwrap();
        let redraw_file = state
            .app_data_dir
            .join("video-sprite-runs/active/manifest.json");
        std::fs::create_dir_all(redraw_file.parent().unwrap()).unwrap();
        std::fs::write(&redraw_file, b"run").unwrap();
        let saved = state.default_save_dir.join("generated-images/kept.png");
        std::fs::create_dir_all(saved.parent().unwrap()).unwrap();
        std::fs::write(&saved, b"asset").unwrap();

        reset_workspace_inner(&state).unwrap();
        reset_workspace_inner(&state).unwrap();

        assert!(!state.workspace_path.exists());
        assert!(!temporary_snapshot.exists());
        assert!(!workspace_asset.exists());
        assert!(!temp_frame.exists());
        assert!(!redraw_file.exists());
        assert!(saved.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}
