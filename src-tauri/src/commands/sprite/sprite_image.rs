use tauri::{command, State};

use crate::asset_library::{self, AssetCategory};
use crate::config::AppState;
use crate::image_processor;
use crate::path_safety::required_file_name;

use super::storage::{
    create_temp_frame_dir, required_export_asset_name, save_temp_frame, unique_timestamped_name,
};
use super::types::{
    CropFrameRequest, ExportFrame, FrameData, ImageSize, SavedImageResult, SplitResult,
};

/// 按任意裁切框提取帧，用于自定义区域和自动边界拆分。
#[command]
pub fn extract_sprite_frames(
    state: State<'_, AppState>,
    image_path: String,
    crops: Vec<CropFrameRequest>,
) -> Result<SplitResult, String> {
    extract_sprite_frames_inner(&state, image_path, crops)
}

pub(crate) fn extract_sprite_frames_inner(
    state: &AppState,
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
    let output_dir = create_temp_frame_dir(state)?;

    let mut frames = Vec::with_capacity(crops.len());
    for crop in crops {
        let frame = crop_frame_with_padding(&img, &crop)?;
        let width = frame.width();
        let height = frame.height();
        if !crop.anchor_x.is_finite() {
            return Err(format!("第{}帧定位针无效", crop.index + 1));
        }
        let path = save_temp_frame(&frame, &output_dir, crop.index)?;
        frames.push(FrameData {
            index: crop.index,
            path,
            width,
            height,
            anchor_x: crop.anchor_x.clamp(0.0, width as f32),
        });
    }

    frames.sort_by_key(|frame| frame.index);
    Ok(SplitResult {
        frames,
        original_size,
    })
}

pub(super) fn crop_frame_with_padding(
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
    state: State<'_, AppState>,
    frames: Vec<ExportFrame>,
    prefix: String,
) -> Result<Vec<String>, String> {
    let frame_data: Vec<image_processor::ExportFrameSource> = frames
        .iter()
        .map(|f| image_processor::ExportFrameSource {
            index: f.index as u32,
            path: f.path.clone(),
            anchor_x: f.anchor_x,
        })
        .collect();
    let base_name = required_export_asset_name(&prefix, "导出序列帧文件夹名称")?;
    let output_dir =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::ExportedFrameSets)?
            .join(unique_timestamped_name(&base_name));
    let output_dir = output_dir.to_string_lossy().to_string();

    image_processor::export_frame_sources(&frame_data, &output_dir, &base_name)
}

/// 导出选中帧为 GIF
#[command]
pub fn export_gif(
    state: State<'_, AppState>,
    frames: Vec<ExportFrame>,
    file_name: String,
    fps: u32,
) -> Result<String, String> {
    let frame_data: Vec<image_processor::ExportFrameSource> = frames
        .iter()
        .map(|f| image_processor::ExportFrameSource {
            index: f.index as u32,
            path: f.path.clone(),
            anchor_x: f.anchor_x,
        })
        .collect();
    let file_name = required_export_asset_name(&file_name, "导出 GIF 文件名")?;
    let output_dir =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::ExportedGifs)?;
    let output_dir = output_dir.to_string_lossy().to_string();

    image_processor::export_gif_sources(&frame_data, &output_dir, &file_name, fps)
}

/// 保存前端 Canvas 生成的序列帧大图到默认输出目录。
#[command]
pub fn save_sprite_sheet_data_url(
    state: State<'_, AppState>,
    data_url: String,
    file_name: String,
) -> Result<SavedImageResult, String> {
    let image_data = image_processor::require_image_data_url_payload(&data_url, "图片数据")?;
    let img = image_processor::base64_to_image(image_data)?;
    let prefix = required_export_asset_name(&file_name, "视频精灵图文件名")?;
    let save_dir =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::VideoSpriteSheets)?
            .to_string_lossy()
            .to_string();
    let file_path = image_processor::save_image(&img, &save_dir, &prefix, 1)?;
    let file_name = required_file_name(
        std::path::Path::new(&file_path),
        "视频精灵图保存结果",
        "请检查素材库保存目录是否是可写的本地目录，然后重新保存。",
    )?;

    Ok(SavedImageResult {
        file_path,
        file_name,
    })
}
