use serde::Serialize;

/// 图片生成过程中的进度事件（通过Channel推送给前端）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum GenerateEvent {
    /// 正在发送请求
    SendingRequest,
    /// 从响应中提取到URL
    ExtractingUrls { found: usize },
    /// 正在处理（裁剪/缩放）第N张图片
    ProcessingImage { index: usize, step: String },
    /// 生成完成
    Completed { total_images: usize },
}

/// 视频生成过程中的进度事件（通过Channel推送给前端）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum VideoGenerationEvent {
    /// 正在调用视频生成模型
    Submitting,
    /// 正在保存到本地
    Saving,
    /// 生成完成
    Completed,
}
