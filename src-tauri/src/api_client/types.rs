use std::time::Duration;

use serde::Serialize;

pub(super) const GENERATION_TIMEOUT: Duration = Duration::from_secs(360);
pub(super) const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
pub(super) const API_CHECK_TIMEOUT: Duration = Duration::from_secs(25);
pub(super) const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(3);
pub(super) const VIDEO_POLL_TIMEOUT: Duration = Duration::from_secs(900);
pub(super) const VIDEO_STATUS_RETRY_ATTEMPTS: usize = 3;
pub(super) const VIDEO_STATUS_RETRY_INTERVAL: Duration = Duration::from_secs(1);
pub(super) const USER_AGENT: &str = "SpriteAnimte/0.1";

pub(super) struct ApiResponseBody {
    pub(super) content_type: String,
    pub(super) body: String,
}

pub(super) struct VideoJobStatus {
    pub(super) status: String,
    pub(super) error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCheckResult {
    pub status: String,
    pub message: String,
    pub endpoint: String,
    pub model: String,
}

#[derive(Default)]
pub(super) struct StreamResponseState {
    pub(super) response_id: Option<String>,
    pub(super) status: Option<String>,
    pub(super) model: Option<String>,
    pub(super) last_event: Option<String>,
}

/// 生成结果
#[derive(Debug, Clone, Serialize)]
pub struct GenerationResult {
    /// 图片本地路径列表（保存后回填）
    pub image_urls: Vec<String>,
    /// 本次生成总耗时（秒）
    pub duration_seconds: f64,
}
