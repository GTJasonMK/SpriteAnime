use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};

use crate::api_client;
use crate::asset_library::{self, AssetCategory};
use crate::commands::generate::{self, ImageGenerationRequest};
use crate::config::AppState;
use crate::image_processor;
use crate::runtime::{AppError, AppResult};
use crate::services::config::ConfigService;
use crate::services::records::{PromptHistoryService, WorkbenchService};
use crate::workbench::WorkbenchRecord;

use super::output::CliProgress;
use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum ApiCommand {
    Check { target: ApiTarget },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ApiTarget {
    Image,
    Video,
    Optimizer,
}

#[derive(Debug, Subcommand)]
pub enum PromptCommand {
    Optimize {
        #[arg(long)]
        prompt: String,
        #[arg(long, default_value = "")]
        negative: String,
        #[arg(long, default_value = "none")]
        style: String,
        #[arg(long, default_value = "1:1")]
        ratio: String,
        #[arg(long, default_value = "原始")]
        resolution: String,
        #[arg(long, default_value_t = 1)]
        rows: u32,
        #[arg(long, default_value_t = 1)]
        cols: u32,
        #[arg(long, default_value = "")]
        reference: String,
        #[arg(long)]
        understand_reference: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ImageCommand {
    Generate {
        #[arg(long)]
        prompt: String,
        #[arg(long, default_value = "")]
        negative: String,
        #[arg(long, default_value = "none")]
        style: String,
        #[arg(long, default_value = "1:1")]
        ratio: String,
        #[arg(long, default_value = "原始")]
        resolution: String,
        #[arg(long, default_value_t = 1)]
        count: u32,
        #[arg(long, default_value = "")]
        reference: String,
        #[arg(long)]
        constraints: Option<PathBuf>,
        #[arg(long, default_value_t = 1)]
        grid_rows: u32,
        #[arg(long, default_value_t = 1)]
        grid_cols: u32,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Matte {
        input: PathBuf,
        #[arg(long, default_value_t = 24)]
        tolerance: u8,
        #[arg(long, default_value_t = 1)]
        feather: u8,
        #[arg(long, value_enum, default_value = "auto")]
        mode: MatteMode,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Erase {
        input: PathBuf,
        #[arg(long)]
        operations: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MatteMode {
    Auto,
    Edge,
    Global,
}

impl MatteMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Edge => "edge",
            Self::Global => "global",
        }
    }
}

pub async fn execute_api(
    state: &AppState,
    selected_profile: Option<&str>,
    command: ApiCommand,
) -> AppResult<CommandResult> {
    let profile = ConfigService::new(state).selected_profile(selected_profile)?;
    match command {
        ApiCommand::Check {
            target: ApiTarget::Image,
        } => {
            let result = api_client::check_models_api_connection(
                require(&profile.api_base, "图片 API 地址")?,
                require(&profile.api_key, "图片 API Key")?,
                require(&profile.last_model, "图片模型")?,
                &profile.proxy_url,
            )
            .await
            .map_err(AppError::api)?;
            CommandResult::serializable("api.check.image", result)
        }
        ApiCommand::Check {
            target: ApiTarget::Video,
        } => {
            let result = api_client::check_models_api_connection(
                require(&profile.video_api_base, "视频 API 地址")?,
                require(&profile.video_api_key, "视频 API Key")?,
                require(&profile.video_model, "视频模型")?,
                &profile.video_proxy_url,
            )
            .await
            .map_err(AppError::api)?;
            CommandResult::serializable("api.check.video", result)
        }
        ApiCommand::Check {
            target: ApiTarget::Optimizer,
        } => {
            let result = generate::check_prompt_optimizer_api(
                profile.prompt_optimizer_api_key,
                profile.prompt_optimizer_api_base,
                profile.prompt_optimizer_model,
                profile.prompt_optimizer_api_mode,
                profile.proxy_url,
            )
            .await
            .map_err(AppError::api)?;
            CommandResult::serializable("api.check.optimizer", result)
        }
    }
}

pub async fn execute_prompt(
    state: &AppState,
    selected_profile: Option<&str>,
    command: PromptCommand,
) -> AppResult<CommandResult> {
    let profile = ConfigService::new(state).selected_profile(selected_profile)?;
    match command {
        PromptCommand::Optimize {
            prompt,
            negative,
            style,
            ratio,
            resolution,
            rows,
            cols,
            reference,
            understand_reference,
        } => {
            let result = generate::optimize_prompt(
                profile.prompt_optimizer_api_key,
                profile.prompt_optimizer_api_base,
                profile.prompt_optimizer_api_mode,
                profile.proxy_url,
                prompt,
                negative,
                profile.prompt_optimizer_model,
                style,
                ratio,
                resolution,
                rows,
                cols,
                reference,
                understand_reference,
            )
            .await
            .map_err(AppError::api)?;
            CommandResult::serializable("prompt.optimize", result)
        }
    }
}

pub async fn execute_image(
    state: &AppState,
    selected_profile: Option<&str>,
    command: ImageCommand,
    no_record: bool,
    quiet: bool,
) -> AppResult<CommandResult> {
    match command {
        ImageCommand::Generate {
            prompt,
            negative,
            style,
            ratio,
            resolution,
            count,
            reference,
            constraints,
            grid_rows,
            grid_cols,
            output,
        } => {
            let profile = ConfigService::new(state).selected_profile(selected_profile)?;
            let progress = CliProgress::new(quiet);
            let prompt = apply_image_constraints(
                prompt,
                constraints,
                grid_rows,
                grid_cols,
                !reference.is_empty(),
            )?;
            let result = generate::generate_image_inner(
                state,
                &progress,
                ImageGenerationRequest {
                    api_key: profile.api_key,
                    api_base: profile.api_base,
                    proxy_url: profile.proxy_url,
                    prompt: prompt.clone(),
                    neg_prompt: negative,
                    model: profile.last_model.clone(),
                    style,
                    ratio,
                    resolution,
                    count,
                    api_mode: profile.generation_api_mode,
                    reference_image_paths: if reference.trim().is_empty() {
                        Vec::new()
                    } else {
                        vec![reference]
                    },
                    output_dir: output,
                },
            )
            .await
            .map_err(AppError::api)?;
            if !no_record {
                record_generated_images(state, &prompt, &profile.last_model, &result)?;
            }
            CommandResult::serializable("image.generate", result)
        }
        ImageCommand::Matte {
            input,
            tolerance,
            feather,
            mode,
            output,
        } => matte_image(state, input, tolerance, feather, mode, output),
        ImageCommand::Erase {
            input,
            operations,
            output,
        } => erase_image(state, input, operations, output),
    }
}

fn apply_image_constraints(
    prompt: String,
    path: Option<PathBuf>,
    rows: u32,
    cols: u32,
    has_reference: bool,
) -> AppResult<String> {
    let Some(path) = path else {
        return Ok(prompt);
    };
    let constraints =
        read_constraints::<crate::services::constraints::ImageGenerationConstraints>(&path)?;
    crate::services::constraints::build_sprite_image_prompt(
        &prompt,
        &constraints,
        rows,
        cols,
        has_reference,
    )
    .map_err(AppError::validation)
}

pub(crate) fn read_constraints<T: serde::de::DeserializeOwned>(path: &Path) -> AppResult<T> {
    let content = std::fs::read_to_string(path).map_err(|error| {
        AppError::filesystem(format!(
            "读取生成约束文件失败：{} ({error})",
            path.display()
        ))
    })?;
    serde_json::from_str(&content)
        .map_err(|error| AppError::validation(format!("解析生成约束文件失败: {error}")))
}

fn matte_image(
    state: &AppState,
    input: PathBuf,
    tolerance: u8,
    feather: u8,
    mode: MatteMode,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    if tolerance == 0 || feather > 3 {
        return Err(AppError::validation(
            "抠图容差必须大于 0，羽化半径不能超过 3",
        ));
    }
    let image =
        image_processor::load_image(&input.to_string_lossy()).map_err(AppError::processing)?;
    let result = image_processor::make_background_transparent(
        &image,
        image_processor::TransparentBackgroundOptions {
            tolerance,
            feather_radius: feather,
            color_key_mode: generate::parse_color_key_mode(mode.as_str())
                .map_err(AppError::validation)?,
        },
    );
    let output_dir = output.unwrap_or(
        asset_library::category_dir(&state.default_save_dir, AssetCategory::MattedImages)
            .map_err(AppError::filesystem)?,
    );
    let path = image_processor::save_transparent_copy_to_dir(&result.image, &input, &output_dir)
        .map_err(AppError::processing)?;
    CommandResult::serializable(
        "image.matte",
        serde_json::json!({
            "path": path,
            "transparentPixels": result.transparent_pixels,
            "backgroundRgb": result.background_rgb,
        }),
    )
}

fn erase_image(
    state: &AppState,
    input: PathBuf,
    operations: PathBuf,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let content = std::fs::read_to_string(&operations).map_err(|error| {
        AppError::filesystem(format!(
            "读取擦除操作文件失败：{} ({error})",
            operations.display()
        ))
    })?;
    let request = serde_json::from_str::<image_processor::EraseOperationsV1>(&content)
        .map_err(|error| AppError::validation(format!("解析擦除操作文件失败: {error}")))?;
    let image =
        image_processor::load_image(&input.to_string_lossy()).map_err(AppError::processing)?;
    let result =
        image_processor::apply_erase_operations(&image, &request).map_err(AppError::processing)?;
    let output_dir = output.unwrap_or(
        asset_library::category_dir(&state.default_save_dir, AssetCategory::MattedImages)
            .map_err(AppError::filesystem)?,
    );
    let path = image_processor::save_transparent_copy_to_dir(&result.image, &input, &output_dir)
        .map_err(AppError::processing)?;
    CommandResult::serializable(
        "image.erase",
        serde_json::json!({
            "path": path,
            "erasedPixels": result.erased_pixels,
            "operations": result.operations,
        }),
    )
}

fn record_generated_images(
    state: &AppState,
    prompt: &str,
    model: &str,
    result: &api_client::GenerationResult,
) -> AppResult<()> {
    PromptHistoryService::new(state).add(prompt)?;
    let now = chrono::Local::now().to_rfc3339();
    let records = result
        .image_urls
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let label = Path::new(path)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .ok_or_else(|| AppError::internal(format!("生成图片路径缺少文件名：{path}")))?;
            Ok(WorkbenchRecord {
                id: format!("cli-{}-{index}", chrono::Local::now().timestamp_micros()),
                path: path.clone(),
                label,
                prompt: prompt.into(),
                model: model.into(),
                duration_seconds: Some(result.duration_seconds),
                created_at: now.clone(),
                updated_at: now.clone(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    WorkbenchService::new(state).upsert(records)?;
    Ok(())
}

fn require<'a>(value: &'a str, label: &str) -> AppResult<&'a str> {
    if value.trim().is_empty() {
        Err(AppError::config(format!("{label}为空")))
    } else {
        Ok(value.trim())
    }
}
