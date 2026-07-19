use base64::Engine;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

use crate::asset_library::{self, AssetCategory};
use crate::config::AppState;
use crate::image_processor;

use super::store::validate_file_inside;
use super::types::{RedrawBatchRecord, RedrawRunManifest};

const MAX_FINAL_DIMENSION: u32 = 16_384;
const MAX_FINAL_PIXELS: u64 = 64 * 1024 * 1024;
const MAX_BATCH_INPUT_DATA_URL_BYTES: usize = 48 * 1024 * 1024;

pub(super) fn normalize_generated_grid(
    manifest: &RedrawRunManifest,
    image: &DynamicImage,
) -> Result<(DynamicImage, u32, u32), String> {
    let width = image.width();
    let height = image.height();
    let cols = manifest.group_cols;
    let rows = manifest.group_rows;
    let (mut crop_width, mut crop_height) =
        if u64::from(width) * u64::from(rows) > u64::from(height) * u64::from(cols) {
            (
                (u64::from(height) * u64::from(cols) / u64::from(rows)) as u32,
                height,
            )
        } else {
            (
                width,
                (u64::from(width) * u64::from(rows) / u64::from(cols)) as u32,
            )
        };
    crop_width -= crop_width % cols;
    crop_height -= crop_height % rows;
    if crop_width == 0 || crop_height == 0 {
        return Err(format!(
            "生成图尺寸 {}x{} 无法按 {}:{} 网格裁切",
            width, height, cols, rows
        ));
    }
    let cell_width = crop_width / cols;
    let cell_height = crop_height / rows;
    if cell_width < 128 || cell_height < 128 {
        return Err(format!(
            "生成图 {}x{} 按 {}:{} 网格居中裁切后单格仅 {}x{}，最低要求 128x128",
            width, height, cols, rows, cell_width, cell_height
        ));
    }
    let crop_x = (width - crop_width) / 2;
    let crop_y = (height - crop_height) / 2;
    Ok((
        image.crop_imm(crop_x, crop_y, crop_width, crop_height),
        cell_width,
        cell_height,
    ))
}

pub(super) fn split_valid_batch_frames(
    active_dir: &Path,
    image: &DynamicImage,
    manifest: &RedrawRunManifest,
    batch: &RedrawBatchRecord,
) -> Result<Vec<String>, String> {
    let mut paths = Vec::with_capacity(batch.valid_count as usize);
    for local_index in 0..batch.valid_count {
        let col = local_index % manifest.group_cols;
        let row = local_index / manifest.group_cols;
        let x0 = col * image.width() / manifest.group_cols;
        let x1 = (col + 1) * image.width() / manifest.group_cols;
        let y0 = row * image.height() / manifest.group_rows;
        let y1 = (row + 1) * image.height() / manifest.group_rows;
        let frame = image.crop_imm(x0, y0, (x1 - x0).max(1), (y1 - y0).max(1));
        let global_index = batch.global_start + local_index;
        let path = active_dir
            .join("frames")
            .join(format!("frame_{global_index:04}.png"));
        frame
            .save(&path)
            .map_err(|e| format!("保存第{}帧失败: {e}", global_index + 1))?;
        paths.push(path.to_string_lossy().to_string());
    }
    Ok(paths)
}

pub(super) fn compose_final_image(
    manifest: &RedrawRunManifest,
    active_dir: &Path,
) -> Result<RgbaImage, String> {
    let first_batch = manifest
        .batches
        .first()
        .ok_or_else(|| "运行缺少批次".to_string())?;
    let cell_width = first_batch
        .cell_width
        .ok_or_else(|| "缺少最终单格宽度".to_string())?;
    let cell_height = first_batch
        .cell_height
        .ok_or_else(|| "缺少最终单格高度".to_string())?;
    let width = manifest
        .final_cols
        .checked_mul(cell_width)
        .ok_or_else(|| "最终图片宽度溢出".to_string())?;
    let height = manifest
        .final_rows
        .checked_mul(cell_height)
        .ok_or_else(|| "最终图片高度溢出".to_string())?;
    if width > MAX_FINAL_DIMENSION || height > MAX_FINAL_DIMENSION {
        return Err(format!(
            "最终图片尺寸 {}x{} 超过最长边 {}。解决方法：请调整最终列数或降低重绘分辨率。",
            width, height, MAX_FINAL_DIMENSION
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_FINAL_PIXELS {
        return Err(format!(
            "最终图片像素数 {} 超过上限 {}。解决方法：请调整最终列数或降低重绘分辨率。",
            u64::from(width) * u64::from(height),
            MAX_FINAL_PIXELS
        ));
    }
    let mut canvas = RgbaImage::from_pixel(width, height, image::Rgba([0, 0, 0, 0]));
    let mut ordered_paths = vec![None; manifest.total_frames as usize];
    for batch in &manifest.batches {
        if batch.frame_paths.len() != batch.valid_count as usize {
            return Err(format!("第{}批拆分帧数量不完整", batch.index + 1));
        }
        for (local_index, path) in batch.frame_paths.iter().enumerate() {
            let global_index = batch.global_start as usize + local_index;
            if global_index < ordered_paths.len() {
                ordered_paths[global_index] = Some(path.clone());
            }
        }
    }
    for (index, path) in ordered_paths.into_iter().enumerate() {
        let path = path.ok_or_else(|| format!("缺少第{}帧", index + 1))?;
        validate_file_inside(&active_dir.join("frames"), Path::new(&path), "重绘拆分帧")?;
        let frame = image_processor::load_image(&path)?.to_rgba8();
        let scale = (cell_width as f64 / frame.width().max(1) as f64)
            .min(cell_height as f64 / frame.height().max(1) as f64);
        let target_width = ((frame.width() as f64 * scale).round() as u32).clamp(1, cell_width);
        let target_height = ((frame.height() as f64 * scale).round() as u32).clamp(1, cell_height);
        let resized = image::imageops::resize(
            &frame,
            target_width,
            target_height,
            image::imageops::FilterType::Lanczos3,
        );
        let col = index as u32 % manifest.final_cols;
        let row = index as u32 / manifest.final_cols;
        let x = col * cell_width + (cell_width - target_width) / 2;
        let y = row * cell_height + (cell_height - target_height);
        image::imageops::overlay(&mut canvas, &resized, i64::from(x), i64::from(y));
    }
    Ok(canvas)
}

pub(super) fn validate_generated_asset_path(state: &AppState, path: &str) -> Result<(), String> {
    let path = Path::new(path);
    if !path.is_file() {
        return Err("批次生成结果不存在或不是普通文件".into());
    }
    let generated_root =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::GeneratedImages)?;
    let root = generated_root
        .canonicalize()
        .map_err(|e| format!("读取生成图片目录失败: {e}"))?;
    let target = path
        .canonicalize()
        .map_err(|e| format!("读取批次生成结果失败: {e}"))?;
    if !target.starts_with(&root) {
        return Err("拒绝使用生成图片目录之外的批次结果".into());
    }
    Ok(())
}

pub(super) fn decode_data_url_image(data_url: &str) -> Result<DynamicImage, String> {
    if data_url.len() > MAX_BATCH_INPUT_DATA_URL_BYTES {
        return Err(format!(
            "分组输入图数据超过 {:.0} MiB 上限，请降低单帧边长或分组大小",
            MAX_BATCH_INPUT_DATA_URL_BYTES as f64 / 1024.0 / 1024.0
        ));
    }
    let encoded = image_processor::require_image_data_url_payload(data_url, "分组输入图数据")?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| format!("分组输入图 base64 解码失败: {e}"))?;
    image_processor::bytes_to_image(&bytes)
}
