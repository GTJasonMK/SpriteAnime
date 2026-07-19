use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use image::{DynamicImage, GenericImage, RgbaImage};

use crate::asset_library::{self, AssetCategory};
use crate::commands::generate::{self, GenerateVideoRequest};
use crate::commands::sprite::{self, VideoExtractRegion, VideoFramesResult};
use crate::commands::tools::{configured_ffmpeg_tools, ConfiguredFfmpegTools};
use crate::config::AppState;
use crate::runtime::{AppError, AppResult};
use crate::services::config::ConfigService;

use super::output::CliProgress;
use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum VideoCommand {
    Generate {
        #[arg(long)]
        prompt: String,
        #[arg(long, default_value = "1280x720")]
        size: String,
        #[arg(long, default_value_t = 4)]
        seconds: u32,
        #[arg(long, default_value = "")]
        source_video_id: String,
        #[arg(long, default_value = "")]
        direction: String,
        #[arg(long, default_value = "")]
        reference: String,
        #[arg(long)]
        constraints: Option<PathBuf>,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Probe {
        input: PathBuf,
    },
    Extract {
        input: PathBuf,
        #[arg(long)]
        frames: usize,
        #[arg(long, default_value_t = 0.0)]
        start: f64,
        #[arg(long)]
        end: Option<f64>,
        #[arg(long)]
        max_edge: Option<u32>,
        #[command(flatten)]
        crop: CropOptions,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Preview {
        input: PathBuf,
        #[arg(long, default_value_t = 16)]
        frames: usize,
        #[arg(long, default_value_t = 4)]
        cols: u32,
        #[arg(long)]
        max_edge: Option<u32>,
        #[command(flatten)]
        crop: CropOptions,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Default, Args)]
pub struct CropOptions {
    #[arg(long)]
    crop_x: Option<f64>,
    #[arg(long)]
    crop_y: Option<f64>,
    #[arg(long)]
    crop_width: Option<f64>,
    #[arg(long)]
    crop_height: Option<f64>,
}

pub async fn execute(
    state: &AppState,
    selected_profile: Option<&str>,
    command: VideoCommand,
    quiet: bool,
) -> AppResult<CommandResult> {
    match command {
        VideoCommand::Generate {
            prompt,
            size,
            seconds,
            source_video_id,
            direction,
            reference,
            constraints,
            output,
        } => {
            generate(
                state,
                selected_profile,
                quiet,
                GenerateArgs {
                    prompt,
                    size,
                    seconds,
                    source_video_id,
                    direction,
                    reference,
                    constraints,
                    output,
                },
            )
            .await
        }
        VideoCommand::Probe { input } => probe(state, input),
        VideoCommand::Extract {
            input,
            frames,
            start,
            end,
            max_edge,
            crop,
            output,
        } => {
            let result = extract(state, input, frames, start, end, max_edge, crop, quiet).await?;
            CommandResult::serializable("video.extract", materialize_frames(result, output)?)
        }
        VideoCommand::Preview {
            input,
            frames,
            cols,
            max_edge,
            crop,
            output,
        } => {
            if cols == 0 {
                return Err(AppError::validation("预览图列数必须大于 0"));
            }
            let result = extract(state, input, frames, 0.0, None, max_edge, crop, quiet).await?;
            let preview = compose_preview(state, &result, cols, output);
            let cleanup = cleanup_extract_dir(&result);
            let path = preview?;
            cleanup?;
            CommandResult::serializable("video.preview", serde_json::json!({"path": path}))
        }
    }
}

struct GenerateArgs {
    prompt: String,
    size: String,
    seconds: u32,
    source_video_id: String,
    direction: String,
    reference: String,
    constraints: Option<PathBuf>,
    output: Option<PathBuf>,
}

async fn generate(
    state: &AppState,
    selected_profile: Option<&str>,
    quiet: bool,
    args: GenerateArgs,
) -> AppResult<CommandResult> {
    let profile = ConfigService::new(state).selected_profile(selected_profile)?;
    let progress = CliProgress::new(quiet);
    let prompt = match args.constraints {
        Some(path) => {
            let constraints = super::remote::read_constraints::<
                crate::services::constraints::VideoGenerationConstraints,
            >(&path)?;
            crate::services::constraints::build_video_prompt(
                &args.prompt,
                &constraints,
                !args.reference.is_empty(),
            )
            .map_err(AppError::validation)?
        }
        None => args.prompt,
    };
    let result = generate::generate_video_inner(
        state,
        &progress,
        GenerateVideoRequest {
            api_key: profile.video_api_key,
            api_base: profile.video_api_base,
            proxy_url: profile.video_proxy_url,
            prompt,
            model: profile.video_model,
            api_mode: profile.video_api_mode,
            size: args.size,
            seconds: args.seconds,
            source_video_id: args.source_video_id,
            extension_direction: args.direction,
            reference_image_path: args.reference,
        },
        args.output,
    )
    .await
    .map_err(AppError::api)?;
    CommandResult::serializable("video.generate", result)
}

fn probe(state: &AppState, input: PathBuf) -> AppResult<CommandResult> {
    let tools = configured_tools(state)?;
    let result = sprite::probe_video_file_inner(&input.to_string_lossy(), &tools.ffprobe)
        .map_err(AppError::processing)?;
    CommandResult::serializable("video.probe", result)
}

#[allow(clippy::too_many_arguments)]
async fn extract(
    state: &AppState,
    input: PathBuf,
    frames: usize,
    start: f64,
    end: Option<f64>,
    max_edge: Option<u32>,
    crop: CropOptions,
    quiet: bool,
) -> AppResult<VideoFramesResult> {
    let tools = configured_tools(state)?;
    let crop = crop.into_region()?;
    let log_dir = state.log_dir.clone();
    let data_dir = state.app_data_dir.clone();
    let video_path = input.to_string_lossy().to_string();
    tokio::task::spawn_blocking(move || {
        let progress = CliProgress::new(quiet);
        let probe = sprite::probe_video_file_inner(&video_path, &tools.ffprobe)?;
        sprite::extract_video_frames_blocking(
            log_dir,
            data_dir,
            tools,
            &progress,
            video_path,
            frames,
            start,
            end.unwrap_or(probe.duration_seconds),
            crop,
            max_edge,
        )
    })
    .await
    .map_err(|error| AppError::internal(format!("视频抽帧任务执行失败: {error}")))?
    .map_err(AppError::processing)
}

fn configured_tools(state: &AppState) -> AppResult<ConfiguredFfmpegTools> {
    configured_ffmpeg_tools(&state.config.lock()).map_err(AppError::filesystem)
}

fn materialize_frames(
    mut result: VideoFramesResult,
    output: Option<PathBuf>,
) -> AppResult<VideoFramesResult> {
    let Some(output) = output else {
        return Ok(result);
    };
    std::fs::create_dir_all(&output)
        .map_err(|error| AppError::filesystem(format!("创建抽帧输出目录失败: {error}")))?;
    for (index, frame) in result.frames.iter_mut().enumerate() {
        let target = output.join(format!("frame_{index:04}.png"));
        std::fs::copy(&frame.path, &target)
            .map_err(|error| AppError::filesystem(format!("复制抽帧结果失败: {error}")))?;
        frame.path = target.to_string_lossy().to_string();
    }
    cleanup_dir(Path::new(&result.output_dir))?;
    result.output_dir = output.to_string_lossy().to_string();
    Ok(result)
}

fn compose_preview(
    state: &AppState,
    result: &VideoFramesResult,
    cols: u32,
    output: Option<PathBuf>,
) -> AppResult<PathBuf> {
    let first = result
        .frames
        .first()
        .ok_or_else(|| AppError::processing("没有可组成预览图的视频帧"))?;
    let rows = (result.frames.len() as u32).div_ceil(cols);
    let mut sheet = RgbaImage::new(first.width * cols, first.height * rows);
    for (index, frame) in result.frames.iter().enumerate() {
        let image = image::open(&frame.path)
            .map_err(|error| AppError::processing(format!("读取预览帧失败: {error}")))?
            .to_rgba8();
        let x = index as u32 % cols * first.width;
        let y = index as u32 / cols * first.height;
        sheet
            .copy_from(&image, x, y)
            .map_err(|error| AppError::processing(format!("合成预览图失败: {error}")))?;
    }
    let path = preview_output_path(state, output)?;
    DynamicImage::ImageRgba8(sheet)
        .save(&path)
        .map_err(|error| AppError::processing(format!("保存视频预览图失败: {error}")))?;
    Ok(path)
}

fn preview_output_path(state: &AppState, output: Option<PathBuf>) -> AppResult<PathBuf> {
    let path = match output {
        Some(path) if path.extension().is_some() => path,
        Some(dir) => dir.join("video-preview.png"),
        None => {
            asset_library::category_dir(&state.default_save_dir, AssetCategory::VideoSpriteSheets)
                .map_err(AppError::filesystem)?
                .join(format!(
                    "video-preview-{}.png",
                    chrono::Local::now().format("%Y%m%d_%H%M%S_%f")
                ))
        }
    };
    let parent = path
        .parent()
        .ok_or_else(|| AppError::filesystem("预览图输出路径缺少父目录"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::filesystem(format!("创建预览图输出目录失败: {error}")))?;
    Ok(path)
}

fn cleanup_extract_dir(result: &VideoFramesResult) -> AppResult<()> {
    cleanup_dir(Path::new(&result.output_dir))
}

fn cleanup_dir(path: &Path) -> AppResult<()> {
    std::fs::remove_dir_all(path).map_err(|error| {
        AppError::filesystem(format!(
            "清理临时抽帧目录失败：{} ({error})",
            path.display()
        ))
    })
}

impl CropOptions {
    fn into_region(self) -> AppResult<Option<VideoExtractRegion>> {
        match (self.crop_x, self.crop_y, self.crop_width, self.crop_height) {
            (None, None, None, None) => Ok(None),
            (Some(x), Some(y), Some(width), Some(height)) => Ok(Some(VideoExtractRegion {
                x,
                y,
                width,
                height,
            })),
            _ => Err(AppError::validation(
                "裁切区域必须同时提供 --crop-x、--crop-y、--crop-width、--crop-height",
            )),
        }
    }
}
