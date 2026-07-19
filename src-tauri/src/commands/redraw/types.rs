use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedrawExtractionSnapshot {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedrawApiSnapshot {
    pub profile_id: String,
    pub api_base: String,
    pub model: String,
    pub api_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateRedrawRunRequest {
    pub source_name: String,
    pub total_frames: u32,
    pub final_cols: u32,
    pub group_rows: u32,
    pub group_cols: u32,
    pub prompt: String,
    pub negative_prompt: String,
    pub style: String,
    pub resolution: String,
    pub api: RedrawApiSnapshot,
    pub extraction: RedrawExtractionSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedrawBatchRecord {
    pub index: u32,
    pub global_start: u32,
    pub valid_count: u32,
    pub status: String,
    pub input_path: String,
    pub output_path: String,
    pub frame_paths: Vec<String>,
    pub cell_width: Option<u32>,
    pub cell_height: Option<u32>,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedrawRunManifest {
    pub id: String,
    pub status: String,
    pub source_name: String,
    pub total_frames: u32,
    pub final_cols: u32,
    pub final_rows: u32,
    pub group_rows: u32,
    pub group_cols: u32,
    pub prompt: String,
    pub negative_prompt: String,
    pub style: String,
    pub resolution: String,
    pub api: RedrawApiSnapshot,
    pub extraction: RedrawExtractionSnapshot,
    pub batches: Vec<RedrawBatchRecord>,
    pub final_output_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedrawBatchExecution {
    pub manifest: RedrawRunManifest,
    pub prompt: String,
    pub reference_image_paths: Vec<String>,
}
