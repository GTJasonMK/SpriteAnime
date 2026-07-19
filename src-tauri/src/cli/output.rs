use serde::Serialize;

use crate::runtime::{AppError, AppResult, ProgressEvent, ProgressReporter};

use super::OutputFormat;

pub struct CliOutput {
    format: OutputFormat,
    quiet: bool,
}

pub struct CliProgress {
    quiet: bool,
}

impl CliProgress {
    pub fn new(quiet: bool) -> Self {
        Self { quiet }
    }
}

impl ProgressReporter for CliProgress {
    fn emit(&self, event: ProgressEvent) -> AppResult<()> {
        if !self.quiet {
            let json = serde_json::to_string(&event)
                .map_err(|error| AppError::internal(format!("序列化进度失败: {error}")))?;
            eprintln!("{json}");
        }
        Ok(())
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuccessEnvelope<T> {
    schema_version: u32,
    ok: bool,
    command: String,
    data: T,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope<'a> {
    schema_version: u32,
    ok: bool,
    command: String,
    error: &'a AppError,
}

impl CliOutput {
    pub fn new(format: OutputFormat, quiet: bool) -> Self {
        Self { format, quiet }
    }

    pub fn success<T: Serialize>(&self, command: &str, data: T) -> AppResult<()> {
        match self.format {
            OutputFormat::Json => print_json(&SuccessEnvelope {
                schema_version: 1,
                ok: true,
                command: command.into(),
                data,
            }),
            OutputFormat::Human if !self.quiet => {
                let value = serde_json::to_string_pretty(&data)
                    .map_err(|error| AppError::internal(format!("序列化 CLI 输出失败: {error}")))?;
                println!("{value}");
                Ok(())
            }
            OutputFormat::Human => Ok(()),
        }
    }

    pub fn error(&self, command: &str, error: &AppError) -> AppResult<()> {
        match self.format {
            OutputFormat::Json => print_json(&ErrorEnvelope {
                schema_version: 1,
                ok: false,
                command: command.into(),
                error,
            }),
            OutputFormat::Human => {
                eprintln!("错误: {error}");
                Ok(())
            }
        }
    }
}

fn print_json<T: Serialize>(value: &T) -> AppResult<()> {
    let json = serde_json::to_string(value)
        .map_err(|error| AppError::internal(format!("序列化 CLI 输出失败: {error}")))?;
    println!("{json}");
    Ok(())
}
