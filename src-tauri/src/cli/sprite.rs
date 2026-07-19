use std::path::{Path, PathBuf};

use clap::{Args, Subcommand, ValueEnum};
use image::{DynamicImage, GenericImage, RgbaImage};

use crate::asset_library::{self, AssetCategory};
use crate::commands::sprite::{self, CropFrameRequest, SplitResult};
use crate::config::AppState;
use crate::image_processor::{
    self, ExportFrameSource, SpriteBackgroundMode, SpriteLayoutV1, SpriteRegion,
};
use crate::runtime::{AppError, AppResult, DataLock, LockDomain};

use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum SpriteCommand {
    Detect {
        input: PathBuf,
        #[arg(long)]
        rows: u32,
        #[arg(long)]
        cols: u32,
        #[arg(long, default_value_t = 32)]
        threshold: u8,
        #[arg(long, value_enum, default_value = "auto")]
        background: BackgroundMode,
        #[arg(long)]
        allow_expand: bool,
        #[command(flatten)]
        region: RegionOptions,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Split {
        input: PathBuf,
        #[arg(long)]
        layout: PathBuf,
        #[arg(long, value_enum, default_value = "fixed")]
        mode: SplitMode,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    Preview {
        input: PathBuf,
        #[arg(long)]
        layout: PathBuf,
        #[arg(long, value_enum, default_value = "fixed")]
        mode: SplitMode,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    ExportFrames {
        frames_dir: PathBuf,
        #[arg(long, default_value = "frame")]
        prefix: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
    ExportGif {
        frames_dir: PathBuf,
        #[arg(long, default_value = "animation")]
        name: String,
        #[arg(long, default_value_t = 12)]
        fps: u32,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum BackgroundMode {
    Auto,
    White,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SplitMode {
    Tight,
    Fixed,
}

#[derive(Debug, Default, Args)]
pub struct RegionOptions {
    #[arg(long)]
    x: Option<i32>,
    #[arg(long)]
    y: Option<i32>,
    #[arg(long)]
    width: Option<u32>,
    #[arg(long)]
    height: Option<u32>,
}

pub fn execute(state: &AppState, command: SpriteCommand) -> AppResult<CommandResult> {
    match command {
        SpriteCommand::Detect {
            input,
            rows,
            cols,
            threshold,
            background,
            allow_expand,
            region,
            output,
        } => detect(
            input,
            rows,
            cols,
            threshold,
            background,
            allow_expand,
            region,
            output,
        ),
        SpriteCommand::Split {
            input,
            layout,
            mode,
            output,
        } => split(state, input, layout, mode, output),
        SpriteCommand::Preview {
            input,
            layout,
            mode,
            output,
        } => preview(state, input, layout, mode, output),
        SpriteCommand::ExportFrames {
            frames_dir,
            prefix,
            output,
        } => export_frames(state, frames_dir, prefix, output),
        SpriteCommand::ExportGif {
            frames_dir,
            name,
            fps,
            output,
        } => export_gif(state, frames_dir, name, fps, output),
    }
}

#[allow(clippy::too_many_arguments)]
fn detect(
    input: PathBuf,
    rows: u32,
    cols: u32,
    threshold: u8,
    background: BackgroundMode,
    allow_expand: bool,
    region: RegionOptions,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let image = image_processor::load_image(&input.to_string_lossy())
        .map_err(AppError::processing)?
        .to_rgba8();
    let background = match background {
        BackgroundMode::Auto => SpriteBackgroundMode::Auto,
        BackgroundMode::White => SpriteBackgroundMode::White,
    };
    let layout = image_processor::detect_sprite_layout(
        &image,
        rows,
        cols,
        region.into_region()?,
        background,
        threshold,
        allow_expand,
    )
    .map_err(AppError::processing)?;
    if let Some(path) = output {
        write_layout(&path, &layout)?;
    }
    CommandResult::serializable("sprite.detect", layout)
}

fn split(
    state: &AppState,
    input: PathBuf,
    layout_path: PathBuf,
    mode: SplitMode,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
    let layout = read_layout(&layout_path)?;
    validate_layout_image(&layout, &input)?;
    let result = sprite::extract_sprite_frames_inner(
        state,
        input.to_string_lossy().to_string(),
        layout_crops(&layout, mode)?,
    )
    .map_err(AppError::processing)?;
    CommandResult::serializable("sprite.split", materialize_split(result, output)?)
}

fn preview(
    state: &AppState,
    input: PathBuf,
    layout_path: PathBuf,
    mode: SplitMode,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
    let layout = read_layout(&layout_path)?;
    validate_layout_image(&layout, &input)?;
    let result = sprite::extract_sprite_frames_inner(
        state,
        input.to_string_lossy().to_string(),
        layout_crops(&layout, mode)?,
    )
    .map_err(AppError::processing)?;
    let path = preview_path(state, output)?;
    let composed = compose_sheet(&result, layout.cols, &path);
    let cleanup = cleanup_split(&result);
    composed?;
    cleanup?;
    CommandResult::serializable("sprite.preview", serde_json::json!({"path": path}))
}

fn export_frames(
    state: &AppState,
    frames_dir: PathBuf,
    prefix: String,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
    let frames = frame_sources(&frames_dir)?;
    let output = match output {
        Some(path) => path,
        None => {
            asset_library::category_dir(&state.default_save_dir, AssetCategory::ExportedFrameSets)
                .map_err(AppError::filesystem)?
                .join(timestamped(&prefix))
        }
    };
    let paths = image_processor::export_frame_sources(&frames, &output.to_string_lossy(), &prefix)
        .map_err(AppError::processing)?;
    CommandResult::serializable("sprite.export-frames", paths)
}

fn export_gif(
    state: &AppState,
    frames_dir: PathBuf,
    name: String,
    fps: u32,
    output: Option<PathBuf>,
) -> AppResult<CommandResult> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
    let frames = frame_sources(&frames_dir)?;
    let output = match output {
        Some(path) => path,
        None => asset_library::category_dir(&state.default_save_dir, AssetCategory::ExportedGifs)
            .map_err(AppError::filesystem)?,
    };
    let path = image_processor::export_gif_sources(&frames, &output.to_string_lossy(), &name, fps)
        .map_err(AppError::processing)?;
    CommandResult::serializable("sprite.export-gif", serde_json::json!({"path": path}))
}

fn layout_crops(layout: &SpriteLayoutV1, mode: SplitMode) -> AppResult<Vec<CropFrameRequest>> {
    if layout.schema_version != 1 {
        return Err(AppError::validation("SpriteLayout schemaVersion 必须为 1"));
    }
    layout
        .frame_bounds
        .iter()
        .map(|frame| {
            let (x, y, width, height) = match mode {
                SplitMode::Tight => (frame.x, frame.y, frame.width, frame.height),
                SplitMode::Fixed => (
                    frame.cell_x + layout.fixed_offset_x,
                    frame.cell_y + layout.fixed_offset_y,
                    layout.fixed_width,
                    layout.fixed_height,
                ),
            };
            Ok(CropFrameRequest {
                index: frame.index,
                x,
                y,
                width,
                height,
                anchor_x: (frame.anchor_x - x as f32).clamp(0.0, width as f32),
            })
        })
        .collect()
}

fn materialize_split(mut result: SplitResult, output: Option<PathBuf>) -> AppResult<SplitResult> {
    let Some(output) = output else {
        return Ok(result);
    };
    std::fs::create_dir_all(&output)
        .map_err(|error| AppError::filesystem(format!("创建拆分输出目录失败: {error}")))?;
    let temp_dir = result
        .frames
        .first()
        .and_then(|frame| Path::new(&frame.path).parent())
        .map(Path::to_path_buf);
    for frame in &mut result.frames {
        let target = output.join(format!("frame_{:04}.png", frame.index));
        std::fs::copy(&frame.path, &target)
            .map_err(|error| AppError::filesystem(format!("复制拆分帧失败: {error}")))?;
        frame.path = target.to_string_lossy().to_string();
    }
    if let Some(temp_dir) = temp_dir {
        cleanup_dir(&temp_dir)?;
    }
    Ok(result)
}

fn compose_sheet(result: &SplitResult, cols: u32, path: &Path) -> AppResult<()> {
    if cols == 0 {
        return Err(AppError::validation("精灵预览列数必须大于 0"));
    }
    let first = result
        .frames
        .first()
        .ok_or_else(|| AppError::internal("拆分结果没有可合成的帧"))?;
    let (width, height) = result
        .frames
        .iter()
        .skip(1)
        .fold((first.width, first.height), |(width, height), frame| {
            (width.max(frame.width), height.max(frame.height))
        });
    let rows = (result.frames.len() as u32).div_ceil(cols);
    let mut sheet = RgbaImage::new(width * cols, height * rows);
    for (index, frame) in result.frames.iter().enumerate() {
        let image = image::open(&frame.path)
            .map_err(|error| AppError::processing(format!("读取拆分帧失败: {error}")))?
            .to_rgba8();
        let x = index as u32 % cols * width + (width - image.width()) / 2;
        let y = index as u32 / cols * height + height - image.height();
        sheet
            .copy_from(&image, x, y)
            .map_err(|error| AppError::processing(format!("合成精灵预览图失败: {error}")))?;
    }
    DynamicImage::ImageRgba8(sheet)
        .save(path)
        .map_err(|error| AppError::processing(format!("保存精灵预览图失败: {error}")))
}

fn frame_sources(dir: &Path) -> AppResult<Vec<ExportFrameSource>> {
    frame_files(dir)?
        .into_iter()
        .enumerate()
        .map(|(index, path)| {
            let width = image::image_dimensions(&path)
                .map_err(|error| AppError::processing(format!("读取帧尺寸失败: {error}")))?
                .0;
            Ok(ExportFrameSource {
                index: index as u32,
                path: path.to_string_lossy().to_string(),
                anchor_x: width as f32 / 2.0,
            })
        })
        .collect()
}

fn frame_files(dir: &Path) -> AppResult<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir)
        .map_err(|error| AppError::filesystem(format!("读取帧目录失败: {error}")))?
    {
        let path = entry
            .map_err(|error| AppError::filesystem(format!("读取帧目录项失败: {error}")))?
            .path();
        if path.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
        {
            files.push(path);
        }
    }
    files.sort();
    if files.is_empty() {
        return Err(AppError::validation("帧目录中没有 PNG 图片"));
    }
    Ok(files)
}

fn read_layout(path: &Path) -> AppResult<SpriteLayoutV1> {
    let content = std::fs::read_to_string(path)
        .map_err(|error| AppError::filesystem(format!("读取布局文件失败: {error}")))?;
    serde_json::from_str(&content)
        .map_err(|error| AppError::validation(format!("解析布局文件失败: {error}")))
}

fn write_layout(path: &Path, layout: &SpriteLayoutV1) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::filesystem("布局输出路径缺少父目录"))?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::filesystem(format!("创建布局输出目录失败: {error}")))?;
    let json = serde_json::to_string_pretty(layout)
        .map_err(|error| AppError::internal(format!("序列化布局失败: {error}")))?;
    std::fs::write(path, json)
        .map_err(|error| AppError::filesystem(format!("写入布局文件失败: {error}")))
}

fn validate_layout_image(layout: &SpriteLayoutV1, input: &Path) -> AppResult<()> {
    let (width, height) = image::image_dimensions(input)
        .map_err(|error| AppError::processing(format!("读取精灵图尺寸失败: {error}")))?;
    if width != layout.image_width || height != layout.image_height {
        return Err(AppError::validation(format!(
            "布局对应图片尺寸为 {}x{}，当前图片为 {width}x{height}",
            layout.image_width, layout.image_height
        )));
    }
    Ok(())
}

fn preview_path(state: &AppState, output: Option<PathBuf>) -> AppResult<PathBuf> {
    let path = output.unwrap_or(
        asset_library::category_dir(&state.default_save_dir, AssetCategory::VideoSpriteSheets)
            .map_err(AppError::filesystem)?
            .join(format!("{}.png", timestamped("sprite-preview"))),
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| AppError::filesystem(format!("创建预览输出目录失败: {error}")))?;
    }
    Ok(path)
}

fn cleanup_split(result: &SplitResult) -> AppResult<()> {
    if let Some(parent) = result
        .frames
        .first()
        .and_then(|frame| Path::new(&frame.path).parent())
    {
        cleanup_dir(parent)?;
    }
    Ok(())
}

fn cleanup_dir(path: &Path) -> AppResult<()> {
    std::fs::remove_dir_all(path)
        .map_err(|error| AppError::filesystem(format!("清理临时拆分目录失败: {error}")))
}

fn timestamped(prefix: &str) -> String {
    format!(
        "{}_{}",
        prefix,
        chrono::Local::now().format("%Y%m%d_%H%M%S_%f")
    )
}

impl RegionOptions {
    fn into_region(self) -> AppResult<Option<SpriteRegion>> {
        match (self.x, self.y, self.width, self.height) {
            (None, None, None, None) => Ok(None),
            (Some(x), Some(y), Some(width), Some(height)) => Ok(Some(SpriteRegion {
                x,
                y,
                width,
                height,
            })),
            _ => Err(AppError::validation(
                "检测区域必须同时提供 --x、--y、--width、--height",
            )),
        }
    }
}
