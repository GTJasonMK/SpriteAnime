use serde::Serialize;

/// 图片生成过程中的进度事件（通过Channel推送给前端）
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum GenerateEvent {
    /// 开始生成
    Started,
    /// 正在发送请求
    SendingRequest,
    /// 正在接收模型响应
    ReceivingResponse,
    /// 从响应中提取到URL
    ExtractingUrls { found: usize },
    /// 正在处理（裁剪/缩放）第N张图片
    ProcessingImage { index: usize, step: String },
    /// 生成完成
    Completed { total_images: usize },
    /// 生成出错
    Error { message: String },
}
