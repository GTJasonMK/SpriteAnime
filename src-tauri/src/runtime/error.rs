use serde::Serialize;
use std::fmt::{Display, Formatter};

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppErrorKind {
    Validation,
    Config,
    Busy,
    Filesystem,
    Api,
    Processing,
    Partial,
    Internal,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    pub kind: AppErrorKind,
    pub code: String,
    pub message: String,
    pub resolution: String,
}

impl AppError {
    pub fn new(
        kind: AppErrorKind,
        code: impl Into<String>,
        message: impl Into<String>,
        resolution: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            code: code.into(),
            message: message.into(),
            resolution: resolution.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Validation,
            "invalid_argument",
            message,
            "请修正命令参数后重试。",
        )
    }

    pub fn config(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Config,
            "config_invalid",
            message,
            "请检查当前数据目录中的 config.json 和所选 API 配置组。",
        )
    }

    pub fn filesystem(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Filesystem,
            "filesystem_error",
            message,
            "请检查路径、文件权限和磁盘空间。",
        )
    }

    pub fn processing(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Processing,
            "processing_failed",
            message,
            "请检查媒体文件、处理参数和工具配置。",
        )
    }

    pub fn api(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Api,
            "api_request_failed",
            message,
            "请检查 API 配置、模型能力和上游服务状态。",
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Internal,
            "internal_invariant_failed",
            message,
            "请保留完整命令输出和日志并提交问题报告。",
        )
    }

    pub fn partial(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorKind::Partial,
            "resumable_partial_failure",
            message,
            "运行状态已保存；修正问题后执行 redraw resume。",
        )
    }

    pub fn exit_code(&self) -> i32 {
        match self.kind {
            AppErrorKind::Validation => 2,
            AppErrorKind::Config => 3,
            AppErrorKind::Busy => 4,
            AppErrorKind::Filesystem => 5,
            AppErrorKind::Api => 6,
            AppErrorKind::Processing => 7,
            AppErrorKind::Partial => 8,
            AppErrorKind::Internal => 9,
        }
    }
}

impl Display for AppError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}。解决方法：{}", self.message, self.resolution)
    }
}

impl std::error::Error for AppError {}
