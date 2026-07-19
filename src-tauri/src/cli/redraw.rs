use std::path::{Path, PathBuf};

use clap::Subcommand;

use crate::commands::generate::{self, ImageGenerationRequest};
use crate::commands::redraw::{
    self, CreateRedrawRunRequest, RedrawApiSnapshot, RedrawExtractionSnapshot, RedrawRunManifest,
};
use crate::config::{ApiProfile, AppState};
use crate::runtime::{AppError, AppErrorKind, AppResult};
use crate::services::config::ConfigService;

use super::output::CliProgress;
use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum RedrawCommand {
    Start {
        #[arg(long)]
        frames_dir: PathBuf,
        #[arg(long)]
        final_cols: u32,
        #[arg(long)]
        group_rows: u32,
        #[arg(long)]
        group_cols: u32,
        #[arg(long)]
        prompt: String,
        #[arg(long, default_value = "")]
        negative: String,
        #[arg(long, default_value = "none")]
        style: String,
        #[arg(long, default_value = "原始")]
        resolution: String,
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        #[arg(long)]
        end: f64,
        #[arg(long)]
        source_name: Option<String>,
        #[arg(long)]
        transparent: bool,
        #[arg(long)]
        constraints: Option<PathBuf>,
    },
    Run,
    Status,
    Resume,
    Pause,
    SetFinalCols {
        cols: u32,
    },
    Finalize {
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Discard {
        #[arg(long)]
        yes: bool,
    },
}

pub async fn execute(
    state: &AppState,
    selected_profile: Option<&str>,
    command: RedrawCommand,
    quiet: bool,
) -> AppResult<CommandResult> {
    match command {
        RedrawCommand::Start {
            frames_dir,
            final_cols,
            group_rows,
            group_cols,
            prompt,
            negative,
            style,
            resolution,
            start,
            end,
            source_name,
            transparent,
            constraints,
        } => start_run(
            state,
            selected_profile,
            StartArgs {
                frames_dir,
                final_cols,
                group_rows,
                group_cols,
                prompt,
                negative,
                style,
                resolution,
                start,
                end,
                source_name,
                transparent,
                constraints,
            },
        ),
        RedrawCommand::Run | RedrawCommand::Resume => {
            run_batches(state, selected_profile, quiet).await
        }
        RedrawCommand::Status => CommandResult::serializable("redraw.status", active_run(state)?),
        RedrawCommand::Pause => {
            let run = required_active_run(state)?;
            let run = redraw::pause_video_sprite_redraw_run_inner(state, run.id)
                .map_err(map_redraw_error)?;
            CommandResult::serializable("redraw.pause", run)
        }
        RedrawCommand::SetFinalCols { cols } => {
            let run = required_active_run(state)?;
            let run = redraw::update_video_sprite_redraw_final_cols_inner(state, run.id, cols)
                .map_err(map_redraw_error)?;
            CommandResult::serializable("redraw.set-final-cols", run)
        }
        RedrawCommand::Finalize { output } => finalize(state, output),
        RedrawCommand::Discard { yes } => {
            require_yes(yes, "删除活动重绘运行及其中间文件不可撤销")?;
            let run = required_active_run(state)?;
            redraw::discard_video_sprite_redraw_run_inner(state, run.id.clone())
                .map_err(map_redraw_error)?;
            CommandResult::serializable(
                "redraw.discard",
                serde_json::json!({"discardedRunId": run.id}),
            )
        }
    }
}

struct StartArgs {
    frames_dir: PathBuf,
    final_cols: u32,
    group_rows: u32,
    group_cols: u32,
    prompt: String,
    negative: String,
    style: String,
    resolution: String,
    start: f64,
    end: f64,
    source_name: Option<String>,
    transparent: bool,
    constraints: Option<PathBuf>,
}

fn start_run(
    state: &AppState,
    selected_profile: Option<&str>,
    args: StartArgs,
) -> AppResult<CommandResult> {
    let profile = ConfigService::new(state).selected_profile(selected_profile)?;
    let frames = frame_files(&args.frames_dir)?;
    let total_frames =
        u32::try_from(frames.len()).map_err(|_| AppError::validation("序列帧数量超出支持范围"))?;
    let source_name = match args.source_name {
        Some(name) => name,
        None => args
            .frames_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| AppError::validation("序列帧目录缺少名称"))?,
    };
    let prompt = match args.constraints {
        Some(path) => {
            let constraints = super::remote::read_constraints::<
                crate::services::constraints::ImageGenerationConstraints,
            >(&path)?;
            crate::services::constraints::build_redraw_constraint_prompt(&args.prompt, &constraints)
                .map_err(AppError::validation)?
        }
        None => args.prompt,
    };
    let request = CreateRedrawRunRequest {
        source_name,
        total_frames,
        final_cols: args.final_cols,
        group_rows: args.group_rows,
        group_cols: args.group_cols,
        prompt,
        negative_prompt: args.negative,
        style: args.style,
        resolution: args.resolution,
        api: RedrawApiSnapshot {
            profile_id: profile.id,
            api_base: profile.api_base,
            model: profile.last_model,
            api_mode: profile.generation_api_mode,
        },
        extraction: RedrawExtractionSnapshot {
            start_seconds: args.start,
            end_seconds: args.end,
        },
    };
    let run =
        redraw::create_video_sprite_redraw_run_inner(state, request).map_err(map_redraw_error)?;
    let frame_paths = frames
        .into_iter()
        .map(|path| path.to_string_lossy().to_string())
        .collect();
    let run = redraw::prepare_video_sprite_redraw_inputs_inner(
        state,
        run.id,
        frame_paths,
        args.transparent,
    )
    .map_err(|error| AppError::partial(format!("准备分组输入图失败: {error}")))?;
    CommandResult::serializable("redraw.start", run)
}

async fn run_batches(
    state: &AppState,
    selected_profile: Option<&str>,
    quiet: bool,
) -> AppResult<CommandResult> {
    let mut run = required_active_run(state)?;
    redraw::clear_video_sprite_redraw_pause_request_inner(state).map_err(map_redraw_error)?;
    let profile = ConfigService::new(state).selected_profile(selected_profile)?;
    validate_profile_snapshot(&run, &profile)?;
    while let Some(batch) = run
        .batches
        .iter()
        .find(|batch| matches!(batch.status.as_str(), "pending" | "failed"))
        .cloned()
    {
        let execution =
            redraw::begin_video_sprite_redraw_batch_inner(state, run.id.clone(), batch.index)
                .map_err(map_redraw_error)?;
        run = execution.manifest;
        let progress = CliProgress::new(quiet);
        let generation = generate::generate_image_inner(
            state,
            &progress,
            ImageGenerationRequest {
                api_key: profile.api_key.clone(),
                api_base: profile.api_base.clone(),
                proxy_url: profile.proxy_url.clone(),
                prompt: execution.prompt,
                neg_prompt: run.negative_prompt.clone(),
                model: profile.last_model.clone(),
                style: run.style.clone(),
                ratio: format!("{}:{}", run.group_cols, run.group_rows),
                resolution: run.resolution.clone(),
                count: 1,
                api_mode: profile.generation_api_mode.clone(),
                reference_image_paths: execution.reference_image_paths,
                output_dir: None,
            },
        )
        .await;
        let result = match generation {
            Ok(result) => result,
            Err(error) => {
                redraw::fail_video_sprite_redraw_batch_inner(
                    state,
                    run.id.clone(),
                    batch.index,
                    error.clone(),
                )
                .map_err(map_redraw_error)?;
                return Err(AppError::partial(format!(
                    "第{}批生成失败: {error}",
                    batch.index + 1
                )));
            }
        };
        if result.image_urls.len() != 1 {
            let error = format!(
                "第{}批应返回 1 张图片，实际返回 {} 张",
                batch.index + 1,
                result.image_urls.len()
            );
            redraw::fail_video_sprite_redraw_batch_inner(
                state,
                run.id.clone(),
                batch.index,
                error.clone(),
            )
            .map_err(map_redraw_error)?;
            return Err(AppError::partial(error));
        }
        run = redraw::complete_video_sprite_redraw_batch_inner(
            state,
            run.id.clone(),
            batch.index,
            result.image_urls[0].clone(),
        )
        .map_err(|error| {
            AppError::partial(format!("处理第{}批结果失败: {error}", batch.index + 1))
        })?;
        if redraw::take_video_sprite_redraw_pause_request_inner(state).map_err(map_redraw_error)? {
            run = redraw::pause_video_sprite_redraw_run_inner(state, run.id.clone())
                .map_err(map_redraw_error)?;
            break;
        }
    }
    CommandResult::serializable("redraw.run", run)
}

fn finalize(state: &AppState, output: Option<PathBuf>) -> AppResult<CommandResult> {
    let run = required_active_run(state)?;
    let mut result =
        redraw::finalize_video_sprite_redraw_run_inner(state, run.id).map_err(map_redraw_error)?;
    if let Some(output) = output {
        let target = output_target(&output, &result.file_name)?;
        std::fs::copy(&result.file_path, &target)
            .map_err(|error| AppError::filesystem(format!("复制最终重绘图失败: {error}")))?;
        result.file_path = target.to_string_lossy().to_string();
        result.file_name = target
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .ok_or_else(|| AppError::filesystem("最终重绘图输出路径缺少文件名"))?;
    }
    CommandResult::serializable("redraw.finalize", result)
}

fn active_run(state: &AppState) -> AppResult<Option<RedrawRunManifest>> {
    redraw::load_active_video_sprite_redraw_run_inner(state).map_err(map_redraw_error)
}

fn required_active_run(state: &AppState) -> AppResult<RedrawRunManifest> {
    active_run(state)?.ok_or_else(|| AppError::validation("没有活动的分组重绘运行"))
}

fn validate_profile_snapshot(run: &RedrawRunManifest, profile: &ApiProfile) -> AppResult<()> {
    if run.api.profile_id != profile.id
        || run.api.api_base != profile.api_base
        || run.api.model != profile.last_model
        || run.api.api_mode != profile.generation_api_mode
    {
        return Err(AppError::config(
            "当前图片 API 配置与重绘运行快照不一致，请选择创建运行时使用的配置组",
        ));
    }
    Ok(())
}

fn frame_files(dir: &Path) -> AppResult<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Err(AppError::validation(format!(
            "序列帧目录不存在：{}",
            dir.display()
        )));
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|error| AppError::filesystem(format!("读取序列帧目录失败: {error}")))?
    {
        let path = entry
            .map_err(|error| AppError::filesystem(format!("读取序列帧目录项失败: {error}")))?
            .path();
        if path.is_file()
            && path.extension().is_some_and(|extension| {
                matches!(
                    extension.to_string_lossy().to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp"
                )
            })
        {
            paths.push(path);
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err(AppError::validation("序列帧目录中没有支持的图片"));
    }
    Ok(paths)
}

fn output_target(output: &Path, file_name: &str) -> AppResult<PathBuf> {
    let target = if output.extension().is_some() {
        output.to_path_buf()
    } else {
        output.join(file_name)
    };
    let parent = target
        .parent()
        .ok_or_else(|| AppError::filesystem("最终重绘图输出路径缺少父目录"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::filesystem(format!("创建最终输出目录失败: {error}")))?;
    Ok(target)
}

fn map_redraw_error(message: String) -> AppError {
    if message.starts_with("共享数据正在被另一个 SpriteAnime 进程使用") {
        AppError::new(
            AppErrorKind::Busy,
            "data_store_busy",
            message,
            "请等待另一个桌面应用或 CLI 操作完成后重试。",
        )
    } else {
        AppError::processing(message)
    }
}

fn require_yes(yes: bool, message: &str) -> AppResult<()> {
    if yes {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{message}；请添加 --yes 明确确认"
        )))
    }
}
