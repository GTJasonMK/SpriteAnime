use serde::Serialize;

use super::AppResult;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: String,
    pub message: String,
    pub current: Option<u64>,
    pub total: Option<u64>,
    pub time_seconds: Option<f64>,
}

impl ProgressEvent {
    pub fn stage(stage: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
            current: None,
            total: None,
            time_seconds: None,
        }
    }

    pub fn counted(
        stage: impl Into<String>,
        message: impl Into<String>,
        current: u64,
        total: u64,
    ) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
            current: Some(current),
            total: Some(total),
            time_seconds: None,
        }
    }

    pub fn timed_counted(
        stage: impl Into<String>,
        message: impl Into<String>,
        current: u64,
        total: u64,
        time_seconds: f64,
    ) -> Self {
        Self {
            stage: stage.into(),
            message: message.into(),
            current: Some(current),
            total: Some(total),
            time_seconds: Some(time_seconds),
        }
    }
}

pub trait ProgressReporter: Send + Sync {
    fn emit(&self, event: ProgressEvent) -> AppResult<()>;
}
