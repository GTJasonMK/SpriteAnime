use image::{GenericImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

use crate::image_processor;

use super::types::RedrawRunManifest;

const MAX_INPUT_DIMENSION: u32 = 8192;
const MAX_INPUT_PIXELS: u64 = 32 * 1024 * 1024;

pub(super) fn compose_batch_inputs(
    active_dir: &Path,
    manifest: &mut RedrawRunManifest,
    frame_paths: &[String],
    transparent: bool,
) -> Result<(), String> {
    if frame_paths.len() != manifest.total_frames as usize {
        return Err(format!(
            "重绘运行需要 {} 帧，实际提供 {} 帧",
            manifest.total_frames,
            frame_paths.len()
        ));
    }
    let frames = load_frames(frame_paths)?;
    let cell_width = frames
        .iter()
        .map(|image| image.width())
        .max()
        .ok_or_else(|| "没有可用于分组重绘的帧".to_string())?;
    let cell_height = frames
        .iter()
        .map(|image| image.height())
        .max()
        .ok_or_else(|| "没有可用于分组重绘的帧".to_string())?;
    let width = cell_width
        .checked_mul(manifest.group_cols)
        .ok_or_else(|| "分组输入图宽度溢出".to_string())?;
    let height = cell_height
        .checked_mul(manifest.group_rows)
        .ok_or_else(|| "分组输入图高度溢出".to_string())?;
    validate_output_size(width, height)?;
    let background = if transparent {
        Rgba([0, 0, 0, 0])
    } else {
        Rgba([244, 239, 232, 255])
    };

    for batch in &mut manifest.batches {
        let mut canvas = RgbaImage::from_pixel(width, height, background);
        let capacity = manifest.group_rows * manifest.group_cols;
        let last_valid = batch.global_start + batch.valid_count - 1;
        for local_index in 0..capacity {
            let source_index = (batch.global_start + local_index).min(last_valid) as usize;
            let frame = &frames[source_index];
            let col = local_index % manifest.group_cols;
            let row = local_index / manifest.group_cols;
            let x = col * cell_width + (cell_width - frame.width()) / 2;
            let y = row * cell_height + cell_height - frame.height();
            canvas
                .copy_from(frame, x, y)
                .map_err(|error| format!("合成第{}批输入图失败: {error}", batch.index + 1))?;
        }
        let path = active_dir
            .join("inputs")
            .join(format!("batch_{:03}.png", batch.index));
        canvas
            .save(&path)
            .map_err(|error| format!("保存第{}批输入图失败: {error}", batch.index + 1))?;
        batch.input_path = path.to_string_lossy().to_string();
        batch.status = "pending".into();
        batch.error.clear();
    }
    manifest.status = "ready".into();
    Ok(())
}

fn load_frames(paths: &[String]) -> Result<Vec<RgbaImage>, String> {
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            if !PathBuf::from(path).is_file() {
                return Err(format!("第{}帧文件不存在：{path}", index + 1));
            }
            image_processor::load_image(path)
                .map(|image| image.to_rgba8())
                .map_err(|error| format!("读取第{}帧失败: {error}", index + 1))
        })
        .collect()
}

fn validate_output_size(width: u32, height: u32) -> Result<(), String> {
    if width > MAX_INPUT_DIMENSION || height > MAX_INPUT_DIMENSION {
        return Err(format!(
            "分组输入图尺寸 {width}x{height} 超过最长边 {MAX_INPUT_DIMENSION}"
        ));
    }
    if u64::from(width) * u64::from(height) > MAX_INPUT_PIXELS {
        return Err(format!("分组输入图像素数超过上限 {MAX_INPUT_PIXELS}"));
    }
    Ok(())
}
