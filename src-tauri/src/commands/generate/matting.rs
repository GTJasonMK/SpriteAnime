use std::path::Path;
use tauri::{command, State};

use crate::asset_library::{self, AssetCategory};
use crate::config::AppState;
use crate::image_processor;

use super::types::ConnectedEraseCanvasResult;
use super::types::{TransparentBackgroundCanvasResult, TransparentBackgroundCommandResult};
use crate::path_safety::required_file_name;

#[command]
pub fn apply_canvas_background_transparent(
    data_url: String,
    tolerance: u8,
    feather_radius: u8,
    color_key_mode: String,
) -> Result<TransparentBackgroundCanvasResult, String> {
    if tolerance == 0 {
        return Err("抠图容差必须大于 0".into());
    }
    if feather_radius > 3 {
        return Err("抠图羽化半径不能大于 3".into());
    }
    let image_data = image_processor::require_image_data_url_payload(&data_url, "图片数据")?;
    let img = image_processor::base64_to_image(image_data)?;
    let result = image_processor::make_background_transparent(
        &img,
        image_processor::TransparentBackgroundOptions {
            tolerance,
            feather_radius,
            color_key_mode: parse_color_key_mode(&color_key_mode)?,
        },
    );
    let [r, g, b] = result.background_rgb;
    Ok(TransparentBackgroundCanvasResult {
        base64_data: image_processor::image_to_base64(&result.image)?,
        background_color: format!("#{r:02X}{g:02X}{b:02X}"),
        transparent_pixels: result.transparent_pixels,
    })
}

#[command]
pub fn apply_canvas_connected_erase(
    data_url: String,
    operations: image_processor::EraseOperationsV1,
) -> Result<ConnectedEraseCanvasResult, String> {
    let image_data = image_processor::require_image_data_url_payload(&data_url, "图片数据")?;
    let image = image_processor::base64_to_image(image_data)?;
    let result = image_processor::apply_erase_operations(&image, &operations)?;
    Ok(ConnectedEraseCanvasResult {
        base64_data: image_processor::image_to_base64(&result.image)?,
        erased_pixels: result.erased_pixels,
        operations: result.operations,
    })
}

/// 将前端抠图画布保存为新的 PNG 文件。
#[command]
pub fn save_matted_image_data_url(
    state: State<'_, AppState>,
    source_path: String,
    data_url: String,
) -> Result<TransparentBackgroundCommandResult, String> {
    let image_data = image_processor::require_image_data_url_payload(&data_url, "图片数据")?;
    let img = image_processor::base64_to_image(image_data)?;
    let output_dir =
        asset_library::category_dir(&state.default_save_dir, AssetCategory::MattedImages)?;
    let output_path =
        image_processor::save_transparent_copy_to_dir(&img, Path::new(&source_path), &output_dir)?;
    let transparent_pixels = img
        .to_rgba8()
        .pixels()
        .filter(|pixel| pixel.0[3] == 0)
        .count() as u32;
    transparent_command_result(output_path, transparent_pixels)
}

pub(crate) fn parse_color_key_mode(value: &str) -> Result<image_processor::ColorKeyMode, String> {
    match value {
        "auto" => Ok(image_processor::ColorKeyMode::Auto),
        "edge" => Ok(image_processor::ColorKeyMode::EdgeOnly),
        "global" => Ok(image_processor::ColorKeyMode::Global),
        _ => Err(format!("抠图颜色模式无效：{value}")),
    }
}

pub(crate) fn transparent_command_result(
    output_path: String,
    transparent_pixels: u32,
) -> Result<TransparentBackgroundCommandResult, String> {
    let file_name = required_file_name(
        std::path::Path::new(&output_path),
        "抠图保存结果",
        "请重新保存抠图后再使用。",
    )?;
    Ok(TransparentBackgroundCommandResult {
        file_path: output_path,
        file_name,
        transparent_pixels,
    })
}
