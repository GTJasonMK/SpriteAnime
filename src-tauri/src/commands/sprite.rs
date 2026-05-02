use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::{command, State};

use crate::config::AppState;
use crate::image_processor;

static TEMP_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 分割结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitResult {
    /// 帧数据列表。优先使用 path；base64 仅保留兼容旧前端数据结构。
    pub frames: Vec<FrameData>,
    /// 总帧数
    pub total_frames: usize,
    /// 原始图片尺寸
    pub original_size: ImageSize,
}

/// 帧数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    pub index: usize,
    #[serde(default)]
    pub base64: String,
    #[serde(default)]
    pub path: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, rename = "anchorX")]
    pub anchor_x: Option<f32>,
}

/// 图片尺寸
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// 导出帧数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFrame {
    pub index: usize,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub base64: String,
    #[serde(default, rename = "anchorX")]
    pub anchor_x: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropFrameRequest {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub anchor_x: Option<f32>,
}

/// 按任意裁切框提取帧，用于自定义区域和自动边界拆分。
#[command]
pub fn extract_sprite_frames(
    state: State<'_, AppState>,
    image_path: String,
    crops: Vec<CropFrameRequest>,
) -> Result<SplitResult, String> {
    if crops.is_empty() {
        return Err("没有可拆分的裁切区域".into());
    }

    let img = image_processor::load_image(&image_path)?;
    let original_size = ImageSize {
        width: img.width(),
        height: img.height(),
    };
    let output_dir = create_temp_frame_dir(&state)?;

    let mut frames = Vec::with_capacity(crops.len());
    for crop in crops {
        let frame = crop_frame_with_padding(&img, &crop)?;
        let width = frame.width();
        let height = frame.height();
        let path = save_temp_frame(&frame, &output_dir, crop.index)?;
        frames.push(FrameData {
            index: crop.index,
            base64: String::new(),
            path,
            width,
            height,
            anchor_x: crop
                .anchor_x
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, width as f32))
                .or(Some(width as f32 / 2.0)),
        });
    }

    frames.sort_by_key(|frame| frame.index);
    let total = frames.len();
    Ok(SplitResult {
        frames,
        total_frames: total,
        original_size,
    })
}

fn crop_frame_with_padding(
    img: &image::DynamicImage,
    crop: &CropFrameRequest,
) -> Result<image::DynamicImage, String> {
    if crop.width < 1 || crop.height < 1 {
        return Err(format!("第{}帧裁切区域无效", crop.index + 1));
    }

    let crop_left = i64::from(crop.x);
    let crop_top = i64::from(crop.y);
    let crop_right = crop_left + i64::from(crop.width);
    let crop_bottom = crop_top + i64::from(crop.height);
    let img_right = i64::from(img.width());
    let img_bottom = i64::from(img.height());

    let src_left = crop_left.max(0).min(img_right);
    let src_top = crop_top.max(0).min(img_bottom);
    let src_right = crop_right.max(0).min(img_right);
    let src_bottom = crop_bottom.max(0).min(img_bottom);

    let mut canvas =
        image::RgbaImage::from_pixel(crop.width, crop.height, image::Rgba([0, 0, 0, 0]));

    if src_right > src_left && src_bottom > src_top {
        let src_width = (src_right - src_left) as u32;
        let src_height = (src_bottom - src_top) as u32;
        let sub_image = img
            .crop_imm(src_left as u32, src_top as u32, src_width, src_height)
            .to_rgba8();
        image::imageops::overlay(
            &mut canvas,
            &sub_image,
            src_left - crop_left,
            src_top - crop_top,
        );
    }

    Ok(image::DynamicImage::ImageRgba8(canvas))
}

/// 导出选中帧到指定目录
#[command]
pub fn export_frames(
    frames: Vec<ExportFrame>,
    output_dir: String,
    prefix: String,
) -> Result<Vec<String>, String> {
    let frame_data: Vec<(u32, String, String, Option<f32>)> = frames
        .iter()
        .map(|f| (f.index as u32, f.path.clone(), f.base64.clone(), f.anchor_x))
        .collect();

    image_processor::export_frame_sources(&frame_data, &output_dir, &prefix)
}

/// 导出选中帧为 GIF
#[command]
pub fn export_gif(
    frames: Vec<ExportFrame>,
    output_dir: String,
    file_name: String,
    fps: u32,
) -> Result<String, String> {
    let frame_data: Vec<(u32, String, String, Option<f32>)> = frames
        .iter()
        .map(|f| (f.index as u32, f.path.clone(), f.base64.clone(), f.anchor_x))
        .collect();

    image_processor::export_gif_sources(&frame_data, &output_dir, &file_name, fps)
}

fn create_temp_frame_dir(state: &AppState) -> Result<PathBuf, String> {
    let root = state
        .workbench_records_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("sprite-animte"))
        .join("temp_frames");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时帧目录失败: {}", e))?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("frames_{}_{:04}", timestamp, nonce % 10_000));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时帧批次目录失败: {}", e))?;
    cleanup_old_temp_frame_dirs(&root);
    Ok(dir)
}

fn save_temp_frame(
    frame: &image::DynamicImage,
    output_dir: &Path,
    index: usize,
) -> Result<String, String> {
    let filepath = output_dir.join(format!("frame_{:04}.png", index));
    frame
        .save(&filepath)
        .map_err(|e| format!("保存临时帧失败: {}", e))?;
    Ok(filepath.to_string_lossy().to_string())
}

fn cleanup_old_temp_frame_dirs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    dirs.sort_by_key(|(modified, _)| *modified);

    const MAX_TEMP_FRAME_BATCHES: usize = 24;
    if dirs.len() <= MAX_TEMP_FRAME_BATCHES {
        return;
    }
    let remove_count = dirs.len() - MAX_TEMP_FRAME_BATCHES;
    for (_, path) in dirs.into_iter().take(remove_count) {
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};

    fn marked_image() -> DynamicImage {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        img.put_pixel(3, 3, Rgba([0, 0, 200, 255]));
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_crop_frame_with_padding_keeps_negative_crop_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: -1,
            y: -1,
            width: 4,
            height: 4,
            anchor_x: None,
        };

        let frame = crop_frame_with_padding(&img, &crop).unwrap().to_rgba8();

        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 4);
        assert_eq!(frame.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(frame.get_pixel(1, 1).0, [200, 0, 0, 255]);
    }

    #[test]
    fn test_crop_frame_with_padding_keeps_overflow_crop_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: 2,
            y: 2,
            width: 4,
            height: 4,
            anchor_x: None,
        };

        let frame = crop_frame_with_padding(&img, &crop).unwrap().to_rgba8();

        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 4);
        assert_eq!(frame.get_pixel(1, 1).0, [0, 0, 200, 255]);
        assert_eq!(frame.get_pixel(3, 3).0, [0, 0, 0, 0]);
    }

    #[test]
    fn test_crop_frame_with_padding_rejects_zero_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 4,
            anchor_x: None,
        };

        assert!(crop_frame_with_padding(&img, &crop).is_err());
    }
}
