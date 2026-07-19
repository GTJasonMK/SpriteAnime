use serde::{Deserialize, Serialize};

use crate::config::UserConfig;

#[derive(Debug, Clone, Serialize)]
pub struct TransparentBackgroundCommandResult {
    pub file_path: String,
    pub file_name: String,
    pub transparent_pixels: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransparentBackgroundCanvasResult {
    pub base64_data: String,
    pub background_color: String,
    pub transparent_pixels: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConnectedEraseCanvasResult {
    pub base64_data: String,
    pub erased_pixels: u32,
    pub operations: Vec<crate::image_processor::EraseOperationResult>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptOptimizationResult {
    pub prompt: String,
    pub negative_prompt: String,
    pub grid_rows: u32,
    pub grid_cols: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawPromptOptimizationResult {
    pub(super) prompt: Option<String>,
    pub(super) negative_prompt: Option<String>,
    pub(super) grid_rows: Option<u32>,
    pub(super) grid_cols: Option<u32>,
}

#[derive(Debug, Clone)]
pub(super) struct ReferenceImagePayload {
    pub(super) data_url: String,
    pub(super) bytes: Vec<u8>,
    pub(super) mime: &'static str,
    pub(super) label: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GeneratedVideoResult {
    pub file_path: String,
    pub file_name: String,
    pub duration_seconds: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenerateVideoRequest {
    pub api_key: String,
    pub api_base: String,
    pub proxy_url: String,
    pub prompt: String,
    pub model: String,
    pub api_mode: String,
    pub size: String,
    pub seconds: u32,
    pub source_video_id: String,
    pub extension_direction: String,
    pub reference_image_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ConfigFileResult {
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportedConfigResult {
    pub file_path: String,
    pub config: UserConfig,
}
