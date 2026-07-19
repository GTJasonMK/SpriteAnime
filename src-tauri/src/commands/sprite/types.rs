use serde::{Deserialize, Serialize};

use crate::commands::tools::ConfiguredFfmpegTools;

/// 分割结果
#[derive(Debug, Clone, Serialize)]
pub struct SplitResult {
    /// 帧数据列表。帧图片必须通过临时文件 path 传递。
    pub frames: Vec<FrameData>,
    /// 原始图片尺寸
    pub original_size: ImageSize,
}

/// 帧数据
#[derive(Debug, Clone, Serialize)]
pub struct FrameData {
    pub index: usize,
    pub path: String,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "anchorX")]
    pub anchor_x: f32,
}

/// 图片尺寸
#[derive(Debug, Clone, Serialize)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// 保存图片结果
#[derive(Debug, Clone, Serialize)]
pub struct SavedImageResult {
    pub file_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoProbeResult {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoFrameFile {
    pub path: String,
    pub time_seconds: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct VideoFramesResult {
    pub frames: Vec<VideoFrameFile>,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub output_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoExtractRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub(crate) type VideoToolCommands = ConfiguredFfmpegTools;

#[derive(Debug, Clone, Copy)]
pub(super) struct NormalizedVideoExtractRegion {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Debug, Clone)]
pub(super) struct VideoFrameFilter {
    pub(super) value: String,
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum VideoExtractEvent {
    Probing,
    ExtractingFrame {
        index: usize,
        total: usize,
        time_seconds: f64,
    },
    Completed {
        frames: usize,
    },
}

/// 导出帧数据
#[derive(Debug, Clone, Deserialize)]
pub struct ExportFrame {
    pub index: usize,
    pub path: String,
    #[serde(rename = "anchorX")]
    pub anchor_x: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropFrameRequest {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub anchor_x: f32,
}
