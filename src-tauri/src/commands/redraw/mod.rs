mod execution;
mod image_pipeline;
mod input_pipeline;
mod planning;
mod store;
mod types;

use image::DynamicImage;
use std::path::Path;
use tauri::{command, State};

use crate::asset_library::{self, AssetCategory};
use crate::config::AppState;
use crate::image_processor;
use crate::path_safety::required_file_name;

use super::sprite::SavedImageResult;
use execution::*;
use image_pipeline::*;
use input_pipeline::*;
use planning::*;
use store::*;
pub use types::*;

#[command]
pub fn create_video_sprite_redraw_run(
    state: State<'_, AppState>,
    request: CreateRedrawRunRequest,
) -> Result<RedrawRunManifest, String> {
    create_video_sprite_redraw_run_inner(&state, request)
}

pub(crate) fn create_video_sprite_redraw_run_inner(
    state: &AppState,
    request: CreateRedrawRunRequest,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    if active_dir.exists() {
        return Err("已有活动的分组重绘运行。解决方法：请先继续或删除旧运行。".into());
    }
    let manifest = build_manifest(request)?;
    std::fs::create_dir_all(active_dir.join("inputs"))
        .map_err(|e| format!("创建分组输入目录失败: {e}"))?;
    std::fs::create_dir_all(active_dir.join("outputs"))
        .map_err(|e| format!("创建分组输出目录失败: {e}"))?;
    std::fs::create_dir_all(active_dir.join("frames"))
        .map_err(|e| format!("创建重绘帧目录失败: {e}"))?;
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

#[command]
pub fn save_video_sprite_redraw_batch_input(
    state: State<'_, AppState>,
    run_id: String,
    batch_index: u32,
    data_url: String,
) -> Result<RedrawRunManifest, String> {
    save_video_sprite_redraw_batch_input_inner(&state, run_id, batch_index, data_url)
}

fn save_video_sprite_redraw_batch_input_inner(
    state: &AppState,
    run_id: String,
    batch_index: u32,
    data_url: String,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    if manifest.status != "preparing" && manifest.status != "ready" {
        return Err(format!(
            "当前运行状态 `{}` 不允许写入分组输入图",
            manifest.status
        ));
    }
    let batch = required_batch_mut(&mut manifest, batch_index)?;
    let image = decode_data_url_image(&data_url)?;
    let path = active_dir
        .join("inputs")
        .join(format!("batch_{batch_index:03}.png"));
    image
        .save(&path)
        .map_err(|e| format!("保存第{}批输入图失败: {e}", batch_index + 1))?;
    batch.input_path = path.to_string_lossy().to_string();
    batch.status = "pending".into();
    batch.error.clear();
    if manifest
        .batches
        .iter()
        .all(|item| !item.input_path.is_empty())
    {
        manifest.status = "ready".into();
    }
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

pub(crate) fn prepare_video_sprite_redraw_inputs_inner(
    state: &AppState,
    run_id: String,
    frame_paths: Vec<String>,
    transparent: bool,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    compose_batch_inputs(&active_dir, &mut manifest, &frame_paths, transparent)?;
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

#[command]
pub fn load_active_video_sprite_redraw_run(
    state: State<'_, AppState>,
) -> Result<Option<RedrawRunManifest>, String> {
    load_active_video_sprite_redraw_run_inner(&state)
}

pub(crate) fn load_active_video_sprite_redraw_run_inner(
    state: &AppState,
) -> Result<Option<RedrawRunManifest>, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let Some(mut manifest) = load_manifest_if_exists(&active_dir)? else {
        return Ok(None);
    };
    let mut interrupted = false;
    for batch in &mut manifest.batches {
        if batch.status == "generating" {
            batch.status = "failed".into();
            batch.error = "应用在该批生成期间退出，批次已标记为可重试。".into();
            interrupted = true;
        }
    }
    if interrupted {
        manifest.status = "paused".into();
        clear_pause_request(&active_dir)?;
        save_manifest(&active_dir, &manifest)?;
    }
    Ok(Some(manifest))
}

#[command]
pub fn begin_video_sprite_redraw_batch(
    state: State<'_, AppState>,
    run_id: String,
    batch_index: u32,
) -> Result<RedrawBatchExecution, String> {
    begin_video_sprite_redraw_batch_inner(&state, run_id, batch_index)
}

pub(crate) fn begin_video_sprite_redraw_batch_inner(
    state: &AppState,
    run_id: String,
    batch_index: u32,
) -> Result<RedrawBatchExecution, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    let (prompt, reference_image_paths) =
        batch_execution_parameters(&active_dir, &manifest, batch_index)?;
    let batch = required_batch_mut(&mut manifest, batch_index)?;
    batch.status = "generating".into();
    batch.error.clear();
    manifest.status = "running".into();
    save_manifest(&active_dir, &manifest)?;
    Ok(RedrawBatchExecution {
        manifest,
        prompt,
        reference_image_paths,
    })
}

#[command]
pub fn complete_video_sprite_redraw_batch(
    state: State<'_, AppState>,
    run_id: String,
    batch_index: u32,
    generated_path: String,
) -> Result<RedrawRunManifest, String> {
    complete_video_sprite_redraw_batch_inner(&state, run_id, batch_index, generated_path)
}

pub(crate) fn complete_video_sprite_redraw_batch_inner(
    state: &AppState,
    run_id: String,
    batch_index: u32,
    generated_path: String,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    validate_generated_asset_path(state, &generated_path)?;
    let generated_image = image_processor::load_image(&generated_path)?;
    let (image, cell_width, cell_height) = normalize_generated_grid(&manifest, &generated_image)?;
    let batch_snapshot = manifest
        .batches
        .get(batch_index as usize)
        .cloned()
        .ok_or_else(|| format!("第{}批不存在", batch_index + 1))?;
    if batch_snapshot.status != "generating" {
        return Err(format!(
            "第{}批当前状态 `{}` 不能接收生成结果",
            batch_index + 1,
            batch_snapshot.status
        ));
    }

    let output_path = active_dir
        .join("outputs")
        .join(format!("batch_{batch_index:03}.png"));
    image
        .save(&output_path)
        .map_err(|e| format!("保存第{}批生成图失败: {e}", batch_index + 1))?;
    let frame_paths = split_valid_batch_frames(&active_dir, &image, &manifest, &batch_snapshot)?;

    std::fs::remove_file(&generated_path)
        .map_err(|e| format!("清理第{}批生成临时文件失败: {e}", batch_index + 1))?;
    let batch = required_batch_mut(&mut manifest, batch_index)?;
    batch.output_path = output_path.to_string_lossy().to_string();
    batch.frame_paths = frame_paths;
    batch.cell_width = Some(cell_width);
    batch.cell_height = Some(cell_height);
    batch.status = "succeeded".into();
    batch.error.clear();
    manifest.status = if manifest
        .batches
        .iter()
        .all(|item| item.status == "succeeded")
    {
        "ready_to_finalize".into()
    } else {
        "running".into()
    };
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

#[command]
pub fn fail_video_sprite_redraw_batch(
    state: State<'_, AppState>,
    run_id: String,
    batch_index: u32,
    error: String,
) -> Result<RedrawRunManifest, String> {
    fail_video_sprite_redraw_batch_inner(&state, run_id, batch_index, error)
}

pub(crate) fn fail_video_sprite_redraw_batch_inner(
    state: &AppState,
    run_id: String,
    batch_index: u32,
    error: String,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    let error = error.trim();
    if error.is_empty() {
        return Err("批次失败原因为空".into());
    }
    let batch = required_batch_mut(&mut manifest, batch_index)?;
    if batch.status != "generating" {
        return Err(format!("第{}批不在生成状态，不能标记失败", batch_index + 1));
    }
    batch.status = "failed".into();
    batch.error = truncate_with_marker(error, MAX_BATCH_ERROR_CHARS);
    manifest.status = "paused".into();
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

#[command]
pub fn pause_video_sprite_redraw_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<RedrawRunManifest, String> {
    pause_video_sprite_redraw_run_inner(&state, run_id)
}

pub(crate) fn pause_video_sprite_redraw_run_inner(
    state: &AppState,
    run_id: String,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    if manifest
        .batches
        .iter()
        .any(|item| item.status == "generating")
    {
        request_pause(&active_dir)?;
        return Ok(manifest);
    }
    if manifest.status != "completed" {
        manifest.status = "paused".into();
        save_manifest(&active_dir, &manifest)?;
    }
    Ok(manifest)
}

pub(crate) fn clear_video_sprite_redraw_pause_request_inner(
    state: &AppState,
) -> Result<(), String> {
    let _lock = redraw_lock(state)?;
    clear_pause_request(&active_run_dir(state))
}

pub(crate) fn take_video_sprite_redraw_pause_request_inner(
    state: &AppState,
) -> Result<bool, String> {
    let _lock = redraw_lock(state)?;
    take_pause_request(&active_run_dir(state))
}

#[command]
pub fn update_video_sprite_redraw_final_cols(
    state: State<'_, AppState>,
    run_id: String,
    final_cols: u32,
) -> Result<RedrawRunManifest, String> {
    update_video_sprite_redraw_final_cols_inner(&state, run_id, final_cols)
}

pub(crate) fn update_video_sprite_redraw_final_cols_inner(
    state: &AppState,
    run_id: String,
    final_cols: u32,
) -> Result<RedrawRunManifest, String> {
    let _lock = redraw_lock(state)?;
    validate_range(final_cols, 1, MAX_FINAL_COLS, "最终列数")?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    if manifest
        .batches
        .iter()
        .any(|item| item.status == "generating")
    {
        return Err("生成期间不能修改最终列数".into());
    }
    manifest.final_cols = final_cols;
    manifest.final_rows = manifest.total_frames.div_ceil(final_cols);
    manifest.final_output_path.clear();
    if manifest.status == "completed" {
        manifest.status = "ready_to_finalize".into();
    }
    save_manifest(&active_dir, &manifest)?;
    Ok(manifest)
}

#[command]
pub fn finalize_video_sprite_redraw_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<SavedImageResult, String> {
    finalize_video_sprite_redraw_run_inner(&state, run_id)
}

pub(crate) fn finalize_video_sprite_redraw_run_inner(
    state: &AppState,
    run_id: String,
) -> Result<SavedImageResult, String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    let mut manifest = load_required_manifest(&active_dir, &run_id)?;
    if !manifest
        .batches
        .iter()
        .all(|item| item.status == "succeeded")
    {
        return Err("仍有批次未成功，不能合成最终序列帧图".into());
    }
    let final_image = compose_final_image(&manifest, &active_dir)?;
    let output_dir =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::VideoSpriteSheets)?;
    let prefix = safe_output_prefix(&manifest.source_name)?;
    let path = image_processor::save_image(
        &DynamicImage::ImageRgba8(final_image),
        &output_dir.to_string_lossy(),
        &format!("{prefix}_ai_redraw"),
        1,
    )?;
    let file_name = required_file_name(
        Path::new(&path),
        "最终序列帧图保存结果",
        "请重新合成最终序列帧图。",
    )?;
    manifest.final_output_path = path.clone();
    manifest.status = "completed".into();
    save_manifest(&active_dir, &manifest)?;
    Ok(SavedImageResult {
        file_path: path,
        file_name,
    })
}

#[command]
pub fn discard_video_sprite_redraw_run(
    state: State<'_, AppState>,
    run_id: String,
) -> Result<(), String> {
    discard_video_sprite_redraw_run_inner(&state, run_id)
}

pub(crate) fn discard_video_sprite_redraw_run_inner(
    state: &AppState,
    run_id: String,
) -> Result<(), String> {
    let _lock = redraw_lock(state)?;
    let active_dir = active_run_dir(state);
    load_required_manifest(&active_dir, &run_id)?;
    remove_active_run_dir(state)
}

pub(crate) fn remove_active_run_dir_for_workspace_reset(state: &AppState) -> Result<(), String> {
    remove_active_run_dir(state)
}

fn redraw_lock(state: &AppState) -> Result<crate::runtime::DataLock, String> {
    crate::runtime::DataLock::exclusive(&state.locks_dir, crate::runtime::LockDomain::Redraw)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
