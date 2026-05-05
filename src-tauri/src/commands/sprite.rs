use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use tauri::ipc::Channel;
use tauri::{command, State};

use crate::config::{AppState, UserConfig};
use crate::image_processor;

static TEMP_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEMP_VIDEO_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);

/// 分割结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitResult {
    /// 帧数据列表。优先使用 path；base64 仅保留兼容旧前端数据结构。
    pub frames: Vec<FrameData>,
    /// 总帧数
    pub total_frames: usize,
    /// 原始图片尺寸
    pub original_size: ImageSize,
}

/// 帧数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameData {
    pub index: usize,
    #[serde(default)]
    pub base64: String,
    #[serde(default)]
    pub path: String,
    pub width: u32,
    pub height: u32,
    #[serde(default, rename = "anchorX")]
    pub anchor_x: Option<f32>,
}

/// 图片尺寸
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}

/// 保存图片结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedImageResult {
    pub file_path: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoProbeResult {
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFrameFile {
    pub index: usize,
    pub path: String,
    pub time_seconds: f64,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoFramesResult {
    pub frames: Vec<VideoFrameFile>,
    pub duration_seconds: f64,
    pub width: u32,
    pub height: u32,
    pub output_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoExtractRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone)]
struct VideoToolCommands {
    ffmpeg: String,
    ffprobe: String,
}

#[derive(Debug, Clone, Copy)]
struct NormalizedVideoExtractRegion {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone)]
struct VideoFrameFilter {
    value: String,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Copy)]
struct VideoFrameDimensions {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", content = "data")]
pub enum VideoExtractEvent {
    Started,
    Probing,
    ExtractingFrame {
        index: usize,
        total: usize,
        time_seconds: f64,
    },
    Completed {
        frames: usize,
    },
    Error {
        message: String,
    },
}

/// 导出帧数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportFrame {
    pub index: usize,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub base64: String,
    #[serde(default, rename = "anchorX")]
    pub anchor_x: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropFrameRequest {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(default)]
    pub anchor_x: Option<f32>,
}

/// 按任意裁切框提取帧，用于自定义区域和自动边界拆分。
#[command]
pub fn extract_sprite_frames(
    state: State<'_, AppState>,
    image_path: String,
    crops: Vec<CropFrameRequest>,
) -> Result<SplitResult, String> {
    if crops.is_empty() {
        return Err("没有可拆分的裁切区域".into());
    }

    let img = image_processor::load_image(&image_path)?;
    let original_size = ImageSize {
        width: img.width(),
        height: img.height(),
    };
    let output_dir = create_temp_frame_dir(&state)?;

    let mut frames = Vec::with_capacity(crops.len());
    for crop in crops {
        let frame = crop_frame_with_padding(&img, &crop)?;
        let width = frame.width();
        let height = frame.height();
        let path = save_temp_frame(&frame, &output_dir, crop.index)?;
        frames.push(FrameData {
            index: crop.index,
            base64: String::new(),
            path,
            width,
            height,
            anchor_x: crop
                .anchor_x
                .filter(|value| value.is_finite())
                .map(|value| value.clamp(0.0, width as f32))
                .or(Some(width as f32 / 2.0)),
        });
    }

    frames.sort_by_key(|frame| frame.index);
    let total = frames.len();
    Ok(SplitResult {
        frames,
        total_frames: total,
        original_size,
    })
}

fn crop_frame_with_padding(
    img: &image::DynamicImage,
    crop: &CropFrameRequest,
) -> Result<image::DynamicImage, String> {
    if crop.width < 1 || crop.height < 1 {
        return Err(format!("第{}帧裁切区域无效", crop.index + 1));
    }

    let crop_left = i64::from(crop.x);
    let crop_top = i64::from(crop.y);
    let crop_right = crop_left + i64::from(crop.width);
    let crop_bottom = crop_top + i64::from(crop.height);
    let img_right = i64::from(img.width());
    let img_bottom = i64::from(img.height());

    let src_left = crop_left.max(0).min(img_right);
    let src_top = crop_top.max(0).min(img_bottom);
    let src_right = crop_right.max(0).min(img_right);
    let src_bottom = crop_bottom.max(0).min(img_bottom);

    let mut canvas =
        image::RgbaImage::from_pixel(crop.width, crop.height, image::Rgba([0, 0, 0, 0]));

    if src_right > src_left && src_bottom > src_top {
        let src_width = (src_right - src_left) as u32;
        let src_height = (src_bottom - src_top) as u32;
        let sub_image = img
            .crop_imm(src_left as u32, src_top as u32, src_width, src_height)
            .to_rgba8();
        image::imageops::overlay(
            &mut canvas,
            &sub_image,
            src_left - crop_left,
            src_top - crop_top,
        );
    }

    Ok(image::DynamicImage::ImageRgba8(canvas))
}

/// 导出选中帧到指定目录
#[command]
pub fn export_frames(
    frames: Vec<ExportFrame>,
    output_dir: String,
    prefix: String,
) -> Result<Vec<String>, String> {
    let frame_data: Vec<(u32, String, String, Option<f32>)> = frames
        .iter()
        .map(|f| (f.index as u32, f.path.clone(), f.base64.clone(), f.anchor_x))
        .collect();

    image_processor::export_frame_sources(&frame_data, &output_dir, &prefix)
}

/// 导出选中帧为 GIF
#[command]
pub fn export_gif(
    frames: Vec<ExportFrame>,
    output_dir: String,
    file_name: String,
    fps: u32,
) -> Result<String, String> {
    let frame_data: Vec<(u32, String, String, Option<f32>)> = frames
        .iter()
        .map(|f| (f.index as u32, f.path.clone(), f.base64.clone(), f.anchor_x))
        .collect();

    image_processor::export_gif_sources(&frame_data, &output_dir, &file_name, fps)
}

/// 保存前端 Canvas 生成的序列帧大图到默认输出目录。
#[command]
pub fn save_sprite_sheet_data_url(
    state: State<'_, AppState>,
    data_url: String,
    file_name: String,
) -> Result<SavedImageResult, String> {
    let image_data = extract_base64_image_data(&data_url)?;
    let img = image_processor::base64_to_image(image_data)?;
    let prefix = sanitize_sprite_sheet_prefix(&file_name);
    let save_dir = state.default_save_dir.to_string_lossy().to_string();
    let file_path = image_processor::save_image(&img, &save_dir, &prefix, 1)?;
    let file_name = Path::new(&file_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "video_sprite_sheet.png".into());

    Ok(SavedImageResult {
        file_path,
        file_name,
    })
}

/// 用 ffprobe 读取视频元数据。
#[command]
pub async fn probe_video_file(
    state: State<'_, AppState>,
    video_path: String,
) -> Result<VideoProbeResult, String> {
    let tools = video_tool_commands_from_state(&state);
    let log_dir = state.log_dir.clone();
    append_video_sprite_log_to_dir(
        &log_dir,
        &format!(
            "probe start | path={} ffprobe={}",
            video_path,
            summarize_log_text(&tools.ffprobe)
        ),
    );
    tauri::async_runtime::spawn_blocking(move || {
        let result = probe_video_file_inner(&video_path, &tools.ffprobe);
        match &result {
            Ok(probe) => append_video_sprite_log_to_dir(
                &log_dir,
                &format!(
                    "probe ok | path={} duration={:.3}s size={}x{}",
                    video_path, probe.duration_seconds, probe.width, probe.height
                ),
            ),
            Err(err) => append_video_sprite_log_to_dir(
                &log_dir,
                &format!("probe failed | path={video_path} error={err}"),
            ),
        }
        result
    })
    .await
    .map_err(|e| format!("视频元数据任务执行失败: {e}"))?
}

/// 用 ffmpeg 从视频中按时间均匀抽取 PNG 帧。
#[command]
pub async fn extract_video_frames_with_ffmpeg(
    state: State<'_, AppState>,
    channel: Channel<VideoExtractEvent>,
    video_path: String,
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    crop_region: Option<VideoExtractRegion>,
    max_extract_edge: Option<u32>,
) -> Result<VideoFramesResult, String> {
    let tools = video_tool_commands_from_state(&state);
    let log_dir = state.log_dir.clone();
    let workbench_records_path = state.workbench_records_path.clone();
    append_video_sprite_log_to_dir(
        &log_dir,
        &format!(
            "extract request | path={} frame_count={} start={:.3} end={:.3}",
            video_path, frame_count, start_seconds, end_seconds
        ),
    );
    let _ = channel.send(VideoExtractEvent::Started);
    tauri::async_runtime::spawn_blocking(move || {
        extract_video_frames_with_ffmpeg_blocking(
            log_dir,
            workbench_records_path,
            tools,
            channel,
            video_path,
            frame_count,
            start_seconds,
            end_seconds,
            crop_region,
            max_extract_edge,
        )
    })
    .await
    .map_err(|e| format!("视频抽帧任务执行失败: {e}"))?
}

/// 写入视频序列帧前端处理日志，便于定位 Canvas/WebView 侧问题。
#[command]
pub fn log_video_sprite_message(state: State<'_, AppState>, message: String) {
    append_video_sprite_log(
        &state,
        &format!("frontend | {}", summarize_log_text(&message)),
    );
}

#[allow(clippy::too_many_arguments)]
fn extract_video_frames_with_ffmpeg_blocking(
    log_dir: PathBuf,
    workbench_records_path: PathBuf,
    tools: VideoToolCommands,
    channel: Channel<VideoExtractEvent>,
    video_path: String,
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    crop_region: Option<VideoExtractRegion>,
    max_extract_edge: Option<u32>,
) -> Result<VideoFramesResult, String> {
    let mut output_dir_for_cleanup: Option<PathBuf> = None;
    let result: Result<VideoFramesResult, String> = (|| {
        let _ = channel.send(VideoExtractEvent::Probing);
        let probe = probe_video_file_inner(&video_path, &tools.ffprobe)?;
        let normalized = normalize_video_extract_request(
            frame_count,
            start_seconds,
            end_seconds,
            probe.duration_seconds,
        );

        let output_dir = create_temp_video_frame_dir_for_records_path(&workbench_records_path)?;
        output_dir_for_cleanup = Some(output_dir.clone());
        let times = create_video_sample_times(
            normalized.frame_count,
            normalized.start_seconds,
            normalized.end_seconds,
        );
        let frame_filter = build_video_frame_filter(
            crop_region.as_ref(),
            max_extract_edge,
            probe.width,
            probe.height,
        );
        append_video_sprite_log_to_dir(
            &log_dir,
            &format!(
                "extract normalized | mode={} duration={:.3}s size={}x{} frame_count={} start={:.3} end={:.3} filter={} output_size={}x{} ffmpeg={} output_dir={}",
                if normalized.point_extract { "point" } else { "range" },
                probe.duration_seconds,
                probe.width,
                probe.height,
                normalized.frame_count,
                normalized.start_seconds,
                normalized.end_seconds,
                summarize_log_text(&frame_filter.value),
                frame_filter.width,
                frame_filter.height,
                summarize_log_text(&tools.ffmpeg),
                output_dir.display()
            ),
        );
        let frames = if times.len() > 1 {
            match extract_video_frames_batch(
                &log_dir,
                &channel,
                &tools.ffmpeg,
                &video_path,
                &output_dir,
                &times,
                &frame_filter.value,
            ) {
                Ok(frames) => frames,
                Err(err) => {
                    append_video_sprite_log_to_dir(
                        &log_dir,
                        &format!("batch extract failed, fallback to per-frame | error={err}"),
                    );
                    extract_video_frames_one_by_one(
                        &log_dir,
                        &channel,
                        &tools.ffmpeg,
                        &video_path,
                        &output_dir,
                        &times,
                        &frame_filter.value,
                    )?
                }
            }
        } else {
            extract_video_frames_one_by_one(
                &log_dir,
                &channel,
                &tools.ffmpeg,
                &video_path,
                &output_dir,
                &times,
                &frame_filter.value,
            )?
        };

        append_video_sprite_log_to_dir(
            &log_dir,
            &format!(
                "extract ok | frames={} output_dir={}",
                frames.len(),
                output_dir.display()
            ),
        );
        let _ = channel.send(VideoExtractEvent::Completed {
            frames: frames.len(),
        });
        Ok(VideoFramesResult {
            frames,
            duration_seconds: probe.duration_seconds,
            width: probe.width,
            height: probe.height,
            output_dir: output_dir.to_string_lossy().to_string(),
        })
    })();

    if let Err(err) = &result {
        if let Some(output_dir) = output_dir_for_cleanup {
            match std::fs::remove_dir_all(&output_dir) {
                Ok(()) => append_video_sprite_log_to_dir(
                    &log_dir,
                    &format!(
                        "extract cleanup after error ok | output_dir={}",
                        output_dir.display()
                    ),
                ),
                Err(cleanup_err) if output_dir.exists() => append_video_sprite_log_to_dir(
                    &log_dir,
                    &format!(
                        "extract cleanup after error failed | output_dir={} error={}",
                        output_dir.display(),
                        cleanup_err
                    ),
                ),
                Err(_) => {}
            }
        }
        let _ = channel.send(VideoExtractEvent::Error {
            message: err.clone(),
        });
    }
    result
}

fn extract_video_frames_batch(
    log_dir: &Path,
    channel: &Channel<VideoExtractEvent>,
    ffmpeg_command: &str,
    video_path: &str,
    output_dir: &Path,
    times: &[f64],
    filter: &str,
) -> Result<Vec<VideoFrameFile>, String> {
    if times.len() <= 1 {
        return Err("批量抽帧至少需要 2 个时间点".into());
    }

    let start = times.first().copied().unwrap_or(0.0).max(0.0);
    let end = times.last().copied().unwrap_or(start).max(start);
    let span = (end - start).max(0.001);
    let fps = ((times.len() - 1) as f64 / span).max(0.001);
    let pattern = output_dir.join("frame_%04d.png");
    let _ = channel.send(VideoExtractEvent::ExtractingFrame {
        index: 1,
        total: times.len(),
        time_seconds: start,
    });
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "ffmpeg batch command | bin={} start={:.3}s end={:.3}s frames={} fps={:.6} filter={} output={}",
            summarize_log_text(ffmpeg_command),
            start,
            end,
            times.len(),
            fps,
            summarize_log_text(filter),
            pattern.display()
        ),
    );

    let output = Command::new(ffmpeg_command)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-y")
        .arg("-ss")
        .arg(format!("{start:.3}"))
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-an")
        .arg("-sn")
        .arg("-dn")
        .arg("-vf")
        .arg(format!("fps={fps:.6},{filter}"))
        .arg("-frames:v")
        .arg(times.len().to_string())
        .arg("-start_number")
        .arg("0")
        .arg(&pattern)
        .output()
        .map_err(|e| {
            format!(
                "无法运行 ffmpeg({})，请安装 ffmpeg 或在设置中填写 FFmpeg 路径: {}",
                ffmpeg_command, e
            )
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "ffmpeg batch result | status={} stderr={}",
            output.status,
            summarize_log_text(&stderr)
        ),
    );
    if !output.status.success() {
        return Err(format!("ffmpeg 批量抽帧失败: {}", stderr));
    }

    let mut frames = Vec::with_capacity(times.len());
    for (index, time_seconds) in times.iter().copied().enumerate() {
        let output_path = output_dir.join(format!("frame_{:04}.png", index));
        let metadata = std::fs::metadata(&output_path)
            .map_err(|e| format!("ffmpeg 批量抽帧缺少第{}帧: {}", index + 1, e))?;
        if metadata.len() == 0 {
            return Err(format!("ffmpeg 批量抽帧第{}帧为空", index + 1));
        }
        let (width, height) = image::image_dimensions(&output_path)
            .map_err(|e| format!("读取批量抽帧尺寸失败: {}", e))?;
        append_video_sprite_log_to_dir(
            log_dir,
            &format!(
                "frame ok | mode=batch index={} time={:.3}s saved={} size={}x{} bytes={}",
                index,
                time_seconds,
                output_path.display(),
                width,
                height,
                metadata.len()
            ),
        );
        frames.push(VideoFrameFile {
            index,
            path: output_path.to_string_lossy().to_string(),
            time_seconds,
            width,
            height,
        });
    }
    Ok(frames)
}

fn extract_video_frames_one_by_one(
    log_dir: &Path,
    channel: &Channel<VideoExtractEvent>,
    ffmpeg_command: &str,
    video_path: &str,
    output_dir: &Path,
    times: &[f64],
    filter: &str,
) -> Result<Vec<VideoFrameFile>, String> {
    let total = times.len();
    if total == 0 {
        return Ok(Vec::new());
    }

    let concurrency = compute_video_extract_concurrency(total);
    append_video_sprite_log_to_dir(
        log_dir,
        &format!("per-frame extract start | frames={total} concurrency={concurrency}"),
    );

    if concurrency <= 1 {
        let mut frames = Vec::with_capacity(total);
        for (index, time_seconds) in times.iter().copied().enumerate() {
            let _ = channel.send(VideoExtractEvent::ExtractingFrame {
                index: index + 1,
                total,
                time_seconds,
            });
            frames.push(extract_single_video_frame_file(
                log_dir,
                ffmpeg_command,
                video_path,
                output_dir,
                filter,
                index,
                time_seconds,
                "single",
            )?);
        }
        return Ok(frames);
    }

    let tasks = Arc::new(times.iter().copied().enumerate().collect::<Vec<_>>());
    let next_index = Arc::new(AtomicUsize::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<Result<VideoFrameFile, String>>();
    let mut handles = Vec::with_capacity(concurrency);

    for worker_id in 0..concurrency {
        let tasks = Arc::clone(&tasks);
        let next_index = Arc::clone(&next_index);
        let tx = tx.clone();
        let log_dir = log_dir.to_path_buf();
        let ffmpeg_command = ffmpeg_command.to_string();
        let video_path = video_path.to_string();
        let output_dir = output_dir.to_path_buf();
        let filter = filter.to_string();
        handles.push(std::thread::spawn(move || loop {
            let task_index = next_index.fetch_add(1, Ordering::Relaxed);
            if task_index >= tasks.len() {
                break;
            }
            let (index, time_seconds) = tasks[task_index];
            let result = extract_single_video_frame_file(
                &log_dir,
                &ffmpeg_command,
                &video_path,
                &output_dir,
                &filter,
                index,
                time_seconds,
                &format!("concurrent worker={worker_id}"),
            );
            if tx.send(result).is_err() {
                break;
            }
        }));
    }
    drop(tx);

    let mut frames: Vec<Option<VideoFrameFile>> = (0..total).map(|_| None).collect();
    let mut errors = Vec::new();
    let mut completed = 0usize;
    for result in rx {
        completed += 1;
        match result {
            Ok(frame) => {
                let _ = channel.send(VideoExtractEvent::ExtractingFrame {
                    index: completed,
                    total,
                    time_seconds: frame.time_seconds,
                });
                let index = frame.index;
                if index < frames.len() {
                    frames[index] = Some(frame);
                }
            }
            Err(err) => errors.push(err),
        }
    }

    for handle in handles {
        if handle.join().is_err() {
            errors.push("ffmpeg 并发抽帧线程异常退出".into());
        }
    }

    if !errors.is_empty() {
        return Err(format!("ffmpeg 并发逐帧抽帧失败: {}", errors.join("；")));
    }

    frames
        .into_iter()
        .enumerate()
        .map(|(index, frame)| frame.ok_or_else(|| format!("ffmpeg 并发抽帧缺少第{}帧", index + 1)))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn extract_single_video_frame_file(
    log_dir: &Path,
    ffmpeg_command: &str,
    video_path: &str,
    output_dir: &Path,
    filter: &str,
    index: usize,
    time_seconds: f64,
    mode: &str,
) -> Result<VideoFrameFile, String> {
    let output_path = output_dir.join(format!("frame_{:04}.png", index));
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "frame start | mode={} index={} time={:.3}s path={}",
            summarize_log_text(mode),
            index,
            time_seconds,
            output_path.display()
        ),
    );
    let dimensions = extract_single_video_frame(
        ffmpeg_command,
        video_path,
        time_seconds,
        &output_path,
        filter,
        Some(log_dir),
    )?;
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "frame ok | mode={} index={} time={:.3}s saved={} size={}x{}",
            summarize_log_text(mode),
            index,
            time_seconds,
            output_path.display(),
            dimensions.width,
            dimensions.height
        ),
    );
    Ok(VideoFrameFile {
        index,
        path: output_path.to_string_lossy().to_string(),
        time_seconds,
        width: dimensions.width,
        height: dimensions.height,
    })
}

fn compute_video_extract_concurrency(frame_count: usize) -> usize {
    if frame_count <= 1 {
        return 1;
    }
    let parallelism = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(2);
    frame_count.min(parallelism).min(4)
}

fn video_tool_commands_from_state(state: &AppState) -> VideoToolCommands {
    let config = state.config.lock();
    video_tool_commands_from_config(&config)
}

fn video_tool_commands_from_config(config: &UserConfig) -> VideoToolCommands {
    let ffmpeg = non_empty_trimmed(&config.ffmpeg_path).unwrap_or_else(|| "ffmpeg".into());
    let ffprobe = non_empty_trimmed(&config.ffprobe_path)
        .or_else(|| derive_ffprobe_path(&ffmpeg))
        .unwrap_or_else(|| "ffprobe".into());

    VideoToolCommands { ffmpeg, ffprobe }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn derive_ffprobe_path(ffmpeg_command: &str) -> Option<String> {
    let path = Path::new(ffmpeg_command);
    let parent = path.parent()?;
    let file_name = path.file_name()?.to_string_lossy();
    if !file_name.starts_with("ffmpeg") {
        return None;
    }

    let candidate = parent.join(if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    });
    if candidate.is_file() {
        Some(candidate.to_string_lossy().to_string())
    } else {
        None
    }
}

fn probe_video_file_inner(
    video_path: &str,
    ffprobe_command: &str,
) -> Result<VideoProbeResult, String> {
    if video_path.trim().is_empty() {
        return Err("视频路径为空".into());
    }
    if !Path::new(video_path).is_file() {
        return Err("视频文件不存在".into());
    }

    let output = Command::new(ffprobe_command)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("stream=width,height,duration,nb_frames,avg_frame_rate,r_frame_rate:format=duration")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .output()
        .map_err(|e| {
            format!(
                "无法运行 ffprobe({})，请安装 ffmpeg 或在设置中填写 FFmpeg/FFprobe 路径: {}",
                ffprobe_command, e
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe({}) 读取视频失败: {}",
            ffprobe_command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("解析 ffprobe 输出失败: {}", e))?;
    let stream = value
        .get("streams")
        .and_then(|streams| streams.as_array())
        .and_then(|streams| streams.first())
        .ok_or_else(|| "视频中没有可用的视频流".to_string())?;
    let width = stream
        .get("width")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    let height = stream
        .get("height")
        .and_then(|value| value.as_u64())
        .unwrap_or(0) as u32;
    if width == 0 || height == 0 {
        return Err("视频尺寸无效".into());
    }

    let duration = extract_video_duration_from_probe_value(&value, stream)
        .or_else(|| {
            eprintln!(
                "[video-sprite] ffprobe metadata has no duration, trying frame-count fallback"
            );
            probe_video_duration_from_counted_frames(video_path, ffprobe_command)
                .inspect_err(|err| {
                    eprintln!("[video-sprite] frame-count duration fallback failed: {err}");
                })
                .ok()
                .flatten()
        })
        .or_else(|| {
            eprintln!(
                "[video-sprite] frame-count duration unavailable, trying packet timestamp fallback"
            );
            probe_video_duration_from_packets(video_path, ffprobe_command)
                .inspect_err(|err| {
                    eprintln!("[video-sprite] packet duration fallback failed: {err}");
                })
                .ok()
                .flatten()
        })
        .unwrap_or(0.0)
        .max(0.0);

    Ok(VideoProbeResult {
        duration_seconds: duration,
        width,
        height,
    })
}

fn extract_video_duration_from_probe_value(
    value: &serde_json::Value,
    stream: &serde_json::Value,
) -> Option<f64> {
    value
        .pointer("/format/duration")
        .and_then(parse_json_number_like)
        .or_else(|| stream.get("duration").and_then(parse_json_number_like))
        .or_else(|| estimate_video_duration_from_stream_frame_count(stream))
        .filter(|duration| *duration > 0.0)
}

fn probe_video_duration_from_counted_frames(
    video_path: &str,
    ffprobe_command: &str,
) -> Result<Option<f64>, String> {
    let output = Command::new(ffprobe_command)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-count_frames")
        .arg("-show_entries")
        .arg("stream=duration,nb_frames,nb_read_frames,avg_frame_rate,r_frame_rate:format=duration")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .output()
        .map_err(|e| format!("无法运行 ffprobe({}) 估算视频时长: {}", ffprobe_command, e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe({}) 估算视频时长失败: {}",
            ffprobe_command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("解析 ffprobe 时长估算输出失败: {}", e))?;
    let stream = value
        .get("streams")
        .and_then(|streams| streams.as_array())
        .and_then(|streams| streams.first());
    Ok(stream
        .and_then(|stream| extract_video_duration_from_probe_value(&value, stream))
        .filter(|duration| *duration > 0.0))
}

fn probe_video_duration_from_packets(
    video_path: &str,
    ffprobe_command: &str,
) -> Result<Option<f64>, String> {
    let output = Command::new(ffprobe_command)
        .arg("-v")
        .arg("error")
        .arg("-select_streams")
        .arg("v:0")
        .arg("-show_entries")
        .arg("packet=pts_time,dts_time,duration_time")
        .arg("-of")
        .arg("json")
        .arg(video_path)
        .output()
        .map_err(|e| {
            format!(
                "无法运行 ffprobe({}) 读取包时间戳估算视频时长: {}",
                ffprobe_command, e
            )
        })?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe({}) 读取包时间戳失败: {}",
            ffprobe_command,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("解析 ffprobe 包时间戳输出失败: {}", e))?;
    Ok(estimate_video_duration_from_packets(&value))
}

fn estimate_video_duration_from_stream_frame_count(stream: &serde_json::Value) -> Option<f64> {
    let frame_count = stream
        .get("nb_read_frames")
        .and_then(parse_json_u64_like)
        .or_else(|| stream.get("nb_frames").and_then(parse_json_u64_like))?;
    if frame_count == 0 {
        return None;
    }

    let frame_rate = stream
        .get("avg_frame_rate")
        .and_then(parse_frame_rate)
        .or_else(|| stream.get("r_frame_rate").and_then(parse_frame_rate))?;
    if frame_rate <= 0.0 {
        return None;
    }

    Some(frame_count as f64 / frame_rate).filter(|duration| duration.is_finite())
}

fn estimate_video_duration_from_packets(value: &serde_json::Value) -> Option<f64> {
    let packets = value.get("packets")?.as_array()?;
    let mut max_end = 0.0_f64;
    for packet in packets {
        let Some(timestamp) = packet
            .get("pts_time")
            .and_then(parse_json_number_like)
            .or_else(|| packet.get("dts_time").and_then(parse_json_number_like))
        else {
            continue;
        };
        let duration = packet
            .get("duration_time")
            .and_then(parse_json_number_like)
            .unwrap_or(0.0)
            .max(0.0);
        let end = timestamp.max(0.0) + duration;
        if end.is_finite() {
            max_end = max_end.max(end);
        }
    }
    Some(max_end).filter(|duration| *duration > 0.0)
}

fn parse_json_number_like(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .filter(|value| value.is_finite())
}

fn parse_json_u64_like(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<u64>().ok()))
}

fn parse_frame_rate(value: &serde_json::Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number).filter(|rate| rate.is_finite() && *rate > 0.0);
    }

    let text = value.as_str()?.trim();
    if let Some((numerator, denominator)) = text.split_once('/') {
        let numerator = numerator.trim().parse::<f64>().ok()?;
        let denominator = denominator.trim().parse::<f64>().ok()?;
        if denominator == 0.0 {
            return None;
        }
        return Some(numerator / denominator).filter(|rate| rate.is_finite() && *rate > 0.0);
    }

    text.parse::<f64>()
        .ok()
        .filter(|rate| rate.is_finite() && *rate > 0.0)
}

fn sanitize_video_time(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[derive(Debug, Clone, Copy)]
struct NormalizedVideoExtractRequest {
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    point_extract: bool,
}

fn normalize_video_extract_request(
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    duration_seconds: f64,
) -> NormalizedVideoExtractRequest {
    let duration = sanitize_video_time(duration_seconds);
    let requested_point = start_seconds.is_finite()
        && end_seconds.is_finite()
        && (end_seconds - start_seconds).abs() < 0.001;
    let start = sanitize_video_time(start_seconds).clamp(0.0, duration);

    if requested_point {
        return NormalizedVideoExtractRequest {
            frame_count: 1,
            start_seconds: start,
            end_seconds: start,
            point_extract: true,
        };
    }

    let frame_count = frame_count.clamp(2, 240);
    let mut end = sanitize_video_time(end_seconds);
    if end <= start {
        end = duration;
    }

    if duration > 0.03 {
        let last_safe_time = duration - 0.03;
        end = if start <= last_safe_time {
            end.clamp(start, last_safe_time)
        } else {
            start
        };
    } else {
        end = start;
    }

    NormalizedVideoExtractRequest {
        frame_count,
        start_seconds: start,
        end_seconds: end.max(start),
        point_extract: false,
    }
}

fn create_video_sample_times(count: usize, start: f64, end: f64) -> Vec<f64> {
    if count <= 1 || end <= start {
        return vec![start];
    }
    let span = end - start;
    (0..count)
        .map(|index| start + span * (index as f64 / (count - 1) as f64))
        .collect()
}

fn build_video_frame_filter(
    crop_region: Option<&VideoExtractRegion>,
    max_extract_edge: Option<u32>,
    source_width: u32,
    source_height: u32,
) -> VideoFrameFilter {
    let mut filters = Vec::new();
    let mut width = source_width.max(1);
    let mut height = source_height.max(1);

    if let Some(region) = crop_region
        .and_then(|region| normalize_video_extract_region(region, source_width, source_height))
        .filter(|region| {
            region.x > 0
                || region.y > 0
                || region.width < source_width
                || region.height < source_height
        })
    {
        filters.push(format!(
            "crop={}:{}:{}:{}",
            region.width, region.height, region.x, region.y
        ));
        width = region.width;
        height = region.height;
    }

    if let Some(max_edge) = max_extract_edge
        .map(|value| value.clamp(1, 4096))
        .filter(|value| width.max(height) > *value)
    {
        let (scaled_width, scaled_height) = fit_dimensions(width, height, max_edge);
        filters.push(format!(
            "scale={scaled_width}:{scaled_height}:flags=lanczos"
        ));
        width = scaled_width;
        height = scaled_height;
    }

    filters.push("format=rgba".into());
    VideoFrameFilter {
        value: filters.join(","),
        width,
        height,
    }
}

fn normalize_video_extract_region(
    region: &VideoExtractRegion,
    source_width: u32,
    source_height: u32,
) -> Option<NormalizedVideoExtractRegion> {
    if source_width == 0 || source_height == 0 {
        return None;
    }
    let x = sanitize_video_time(region.x)
        .round()
        .clamp(0.0, source_width.saturating_sub(1) as f64) as u32;
    let y = sanitize_video_time(region.y)
        .round()
        .clamp(0.0, source_height.saturating_sub(1) as f64) as u32;
    let max_width = source_width.saturating_sub(x).max(1);
    let max_height = source_height.saturating_sub(y).max(1);
    let width = sanitize_video_time(region.width)
        .round()
        .clamp(1.0, max_width as f64) as u32;
    let height = sanitize_video_time(region.height)
        .round()
        .clamp(1.0, max_height as f64) as u32;
    Some(NormalizedVideoExtractRegion {
        x,
        y,
        width,
        height,
    })
}

fn fit_dimensions(width: u32, height: u32, max_edge: u32) -> (u32, u32) {
    let edge = width.max(height).max(1);
    if edge <= max_edge {
        return (width.max(1), height.max(1));
    }
    let scale = max_edge as f64 / edge as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

fn extract_single_video_frame(
    ffmpeg_command: &str,
    video_path: &str,
    time_seconds: f64,
    output_path: &Path,
    filter: &str,
    log_dir: Option<&Path>,
) -> Result<VideoFrameDimensions, String> {
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建抽帧目录失败: {}", e))?;
    }

    let mut errors = Vec::new();
    for attempt_time in create_frame_extract_attempt_times(time_seconds) {
        append_optional_video_sprite_log(
            log_dir,
            &format!(
                "ffmpeg attempt | requested={:.3}s attempt={:.3}s output={}",
                time_seconds.max(0.0),
                attempt_time,
                output_path.display()
            ),
        );
        match extract_single_video_frame_bytes(
            ffmpeg_command,
            video_path,
            attempt_time,
            filter,
            log_dir,
        ) {
            Ok(bytes) => {
                let image = image::load_from_memory(&bytes)
                    .map_err(|e| format!("ffmpeg 输出不是有效 PNG 图片: {}", e))?;
                std::fs::write(output_path, &bytes)
                    .map_err(|e| format!("写入帧文件失败: {}", e))?;
                if !output_path.is_file() {
                    return Err(format!(
                        "ffmpeg 未生成帧文件: time={:.3}, path={}",
                        attempt_time,
                        output_path.display()
                    ));
                }
                append_optional_video_sprite_log(
                    log_dir,
                    &format!(
                        "ffmpeg attempt ok | requested={:.3}s attempt={:.3}s bytes={} output={}",
                        time_seconds.max(0.0),
                        attempt_time,
                        bytes.len(),
                        output_path.display()
                    ),
                );
                return Ok(VideoFrameDimensions {
                    width: image.width(),
                    height: image.height(),
                });
            }
            Err(err) => {
                append_optional_video_sprite_log(
                    log_dir,
                    &format!(
                        "ffmpeg attempt failed | requested={:.3}s attempt={:.3}s error={}",
                        time_seconds.max(0.0),
                        attempt_time,
                        err
                    ),
                );
                errors.push(format!("{:.3}s: {}", attempt_time, err));
            }
        }
    }

    Err(format!(
        "ffmpeg 未生成帧文件: 原始时间 {:.3}s，尝试 {}",
        time_seconds.max(0.0),
        errors.join("；")
    ))
}

fn extract_single_video_frame_bytes(
    ffmpeg_command: &str,
    video_path: &str,
    time_seconds: f64,
    filter: &str,
    log_dir: Option<&Path>,
) -> Result<Vec<u8>, String> {
    append_optional_video_sprite_log(
        log_dir,
        &format!(
            "ffmpeg command | bin={} ss={:.3}s filter={} input={}",
            summarize_log_text(ffmpeg_command),
            time_seconds.max(0.0),
            summarize_log_text(filter),
            video_path
        ),
    );
    let output = Command::new(ffmpeg_command)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostdin")
        .arg("-y")
        .arg("-ss")
        .arg(format!("{:.3}", time_seconds.max(0.0)))
        .arg("-i")
        .arg(video_path)
        .arg("-map")
        .arg("0:v:0")
        .arg("-frames:v")
        .arg("1")
        .arg("-an")
        .arg("-sn")
        .arg("-dn")
        .arg("-vf")
        .arg(filter)
        .arg("-f")
        .arg("image2pipe")
        .arg("-vcodec")
        .arg("png")
        .arg("-")
        .output()
        .map_err(|e| {
            format!(
                "无法运行 ffmpeg({})，请安装 ffmpeg 或在设置中填写 FFmpeg 路径: {}",
                ffmpeg_command, e
            )
        })?;

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    append_optional_video_sprite_log(
        log_dir,
        &format!(
            "ffmpeg result | ss={:.3}s status={} stdout_bytes={} stderr={}",
            time_seconds.max(0.0),
            output.status,
            output.stdout.len(),
            summarize_log_text(&stderr)
        ),
    );
    if !output.status.success() {
        return Err(format!("ffmpeg 抽帧失败: {}", stderr));
    }
    if output.stdout.is_empty() {
        if stderr.is_empty() {
            return Err("ffmpeg 没有输出图片数据".into());
        }
        return Err(format!("ffmpeg 没有输出图片数据: {}", stderr));
    }
    Ok(output.stdout)
}

fn append_optional_video_sprite_log(log_dir: Option<&Path>, message: &str) {
    if let Some(log_dir) = log_dir {
        append_video_sprite_log_to_dir(log_dir, message);
    }
}

fn append_video_sprite_log(state: &AppState, message: &str) {
    append_video_sprite_log_to_dir(&state.log_dir, message);
}

fn append_video_sprite_log_to_dir(log_dir: &Path, message: &str) {
    let line = format!(
        "{} {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        message
    );
    eprintln!("[video-sprite] {message}");
    if std::fs::create_dir_all(log_dir).is_err() {
        return;
    }
    let path = log_dir.join("video-sprite.log");
    let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) else {
        return;
    };
    let _ = writeln!(file, "{line}");
}

fn summarize_log_text(value: &str) -> String {
    const MAX_CHARS: usize = 600;
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_CHARS {
        normalized
    } else {
        format!(
            "{}...",
            normalized.chars().take(MAX_CHARS).collect::<String>()
        )
    }
}

fn create_frame_extract_attempt_times(time_seconds: f64) -> Vec<f64> {
    let base = time_seconds.max(0.0);
    let candidates = [
        base,
        (base - 0.05).max(0.0),
        (base - 0.2).max(0.0),
        (base - 0.5).max(0.0),
        0.0,
    ];
    let mut times = Vec::new();
    for candidate in candidates {
        if !times
            .iter()
            .any(|value: &f64| (*value - candidate).abs() < 0.001)
        {
            times.push(candidate);
        }
    }
    times
}

fn extract_base64_image_data(data_url: &str) -> Result<&str, String> {
    let trimmed = data_url.trim();
    if trimmed.is_empty() {
        return Err("图片数据为空".into());
    }
    if let Some((meta, payload)) = trimmed.split_once(',') {
        if !meta.contains(";base64") {
            return Err("仅支持 base64 图片数据".into());
        }
        if payload.trim().is_empty() {
            return Err("图片数据为空".into());
        }
        return Ok(payload);
    }
    Ok(trimmed)
}

fn sanitize_sprite_sheet_prefix(name: &str) -> String {
    let stem = Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| name.to_string());
    let sanitized: String = stem
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let sanitized = sanitized
        .trim_matches(|ch: char| ch == '.' || ch == '_' || ch == '-' || ch.is_whitespace())
        .to_string();

    if sanitized.is_empty() {
        "video_sprite_sheet".into()
    } else {
        sanitized
    }
}

fn create_temp_frame_dir(state: &AppState) -> Result<PathBuf, String> {
    let root = state
        .workbench_records_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("sprite-animte"))
        .join("temp_frames");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时帧目录失败: {}", e))?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("frames_{}_{:04}", timestamp, nonce % 10_000));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时帧批次目录失败: {}", e))?;
    cleanup_old_temp_frame_dirs(&root);
    Ok(dir)
}

fn create_temp_video_frame_dir_for_records_path(
    workbench_records_path: &Path,
) -> Result<PathBuf, String> {
    let root = workbench_records_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::temp_dir().join("sprite-animte"))
        .join("temp_video_frames");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时视频帧目录失败: {}", e))?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_VIDEO_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("video_frames_{}_{:04}", timestamp, nonce % 10_000));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时视频帧批次目录失败: {}", e))?;
    cleanup_old_temp_video_frame_dirs(&root);
    Ok(dir)
}

fn save_temp_frame(
    frame: &image::DynamicImage,
    output_dir: &Path,
    index: usize,
) -> Result<String, String> {
    let filepath = output_dir.join(format!("frame_{:04}.png", index));
    frame
        .save(&filepath)
        .map_err(|e| format!("保存临时帧失败: {}", e))?;
    Ok(filepath.to_string_lossy().to_string())
}

fn cleanup_old_temp_frame_dirs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    dirs.sort_by_key(|(modified, _)| *modified);

    const MAX_TEMP_FRAME_BATCHES: usize = 24;
    if dirs.len() <= MAX_TEMP_FRAME_BATCHES {
        return;
    }
    let remove_count = dirs.len() - MAX_TEMP_FRAME_BATCHES;
    for (_, path) in dirs.into_iter().take(remove_count) {
        let _ = std::fs::remove_dir_all(path);
    }
}

fn cleanup_old_temp_video_frame_dirs(root: &Path) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    let mut dirs: Vec<_> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            if !metadata.is_dir() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((modified, entry.path()))
        })
        .collect();
    dirs.sort_by_key(|(modified, _)| *modified);

    const MAX_TEMP_VIDEO_FRAME_BATCHES: usize = 12;
    if dirs.len() <= MAX_TEMP_VIDEO_FRAME_BATCHES {
        return;
    }
    let remove_count = dirs.len() - MAX_TEMP_VIDEO_FRAME_BATCHES;
    for (_, path) in dirs.into_iter().take(remove_count) {
        let _ = std::fs::remove_dir_all(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, Rgba, RgbaImage};
    use std::process::Command;

    fn marked_image() -> DynamicImage {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
        img.put_pixel(3, 3, Rgba([0, 0, 200, 255]));
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_crop_frame_with_padding_keeps_negative_crop_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: -1,
            y: -1,
            width: 4,
            height: 4,
            anchor_x: None,
        };

        let frame = crop_frame_with_padding(&img, &crop).unwrap().to_rgba8();

        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 4);
        assert_eq!(frame.get_pixel(0, 0).0, [0, 0, 0, 0]);
        assert_eq!(frame.get_pixel(1, 1).0, [200, 0, 0, 255]);
    }

    #[test]
    fn test_crop_frame_with_padding_keeps_overflow_crop_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: 2,
            y: 2,
            width: 4,
            height: 4,
            anchor_x: None,
        };

        let frame = crop_frame_with_padding(&img, &crop).unwrap().to_rgba8();

        assert_eq!(frame.width(), 4);
        assert_eq!(frame.height(), 4);
        assert_eq!(frame.get_pixel(1, 1).0, [0, 0, 200, 255]);
        assert_eq!(frame.get_pixel(3, 3).0, [0, 0, 0, 0]);
    }

    #[test]
    fn test_crop_frame_with_padding_rejects_zero_size() {
        let img = marked_image();
        let crop = CropFrameRequest {
            index: 0,
            x: 0,
            y: 0,
            width: 0,
            height: 4,
            anchor_x: None,
        };

        assert!(crop_frame_with_padding(&img, &crop).is_err());
    }

    #[test]
    fn test_create_video_sample_times_includes_endpoints() {
        let times = create_video_sample_times(4, 1.0, 2.5);

        assert_eq!(times.len(), 4);
        assert!((times[0] - 1.0).abs() < f64::EPSILON);
        assert!((times[3] - 2.5).abs() < f64::EPSILON);
        assert!((times[1] - 1.5).abs() < 0.0001);
        assert!((times[2] - 2.0).abs() < 0.0001);
    }

    #[test]
    fn test_normalize_video_extract_request_keeps_point_extract_single_frame() {
        let request = normalize_video_extract_request(2, 0.0, 0.0, 16.533);

        assert!(request.point_extract);
        assert_eq!(request.frame_count, 1);
        assert!((request.start_seconds - 0.0).abs() < f64::EPSILON);
        assert!((request.end_seconds - 0.0).abs() < f64::EPSILON);

        let times = create_video_sample_times(
            request.frame_count,
            request.start_seconds,
            request.end_seconds,
        );
        assert_eq!(times, vec![0.0]);
    }

    #[test]
    fn test_normalize_video_extract_request_uses_range_defaults_for_non_point() {
        let request = normalize_video_extract_request(1, 4.0, 0.0, 10.0);

        assert!(!request.point_extract);
        assert_eq!(request.frame_count, 2);
        assert!((request.start_seconds - 4.0).abs() < f64::EPSILON);
        assert!((request.end_seconds - 9.97).abs() < 0.0001);
    }

    #[test]
    fn test_normalize_video_extract_request_handles_start_at_duration() {
        let request = normalize_video_extract_request(4, 10.0, 0.0, 10.0);

        assert!(!request.point_extract);
        assert_eq!(request.frame_count, 4);
        assert!((request.start_seconds - 10.0).abs() < f64::EPSILON);
        assert!((request.end_seconds - 10.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_build_video_frame_filter_crops_and_caps_edge() {
        let filter = build_video_frame_filter(
            Some(&VideoExtractRegion {
                x: 10.2,
                y: 20.6,
                width: 1000.0,
                height: 500.0,
            }),
            Some(256),
            1920,
            1080,
        );

        assert_eq!(
            filter.value,
            "crop=1000:500:10:21,scale=256:128:flags=lanczos,format=rgba"
        );
        assert_eq!(filter.width, 256);
        assert_eq!(filter.height, 128);
    }

    #[test]
    fn test_build_video_frame_filter_omits_full_size_noop_crop() {
        let filter = build_video_frame_filter(
            Some(&VideoExtractRegion {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 480.0,
            }),
            Some(1024),
            640,
            480,
        );

        assert_eq!(filter.value, "format=rgba");
        assert_eq!(filter.width, 640);
        assert_eq!(filter.height, 480);
    }

    #[test]
    fn test_probe_duration_estimates_from_counted_webm_frames() {
        let value = serde_json::json!({
            "format": {},
            "streams": [
                {
                    "width": 2880,
                    "height": 1800,
                    "avg_frame_rate": "30/1",
                    "r_frame_rate": "30/1",
                    "nb_read_frames": "443"
                }
            ]
        });
        let stream = value["streams"].as_array().unwrap().first().unwrap();

        let duration = extract_video_duration_from_probe_value(&value, stream).unwrap();

        assert!((duration - 14.766_666).abs() < 0.001);
    }

    #[test]
    fn test_probe_duration_estimates_from_packet_timestamps() {
        let value = serde_json::json!({
            "packets": [
                {"pts_time": "0.000000", "duration_time": "0.033000"},
                {"pts_time": "14.734000", "duration_time": "0.033000"}
            ]
        });

        let duration = estimate_video_duration_from_packets(&value).unwrap();

        assert!((duration - 14.767).abs() < 0.001);
    }

    #[test]
    fn test_parse_frame_rate_supports_fractional_ffprobe_values() {
        assert!(
            (parse_frame_rate(&serde_json::json!("30000/1001")).unwrap() - 29.970_029).abs()
                < 0.001
        );
        assert_eq!(parse_frame_rate(&serde_json::json!("0/0")), None);
    }

    #[test]
    fn test_compute_video_extract_concurrency_is_bounded() {
        assert_eq!(compute_video_extract_concurrency(0), 1);
        assert_eq!(compute_video_extract_concurrency(1), 1);
        assert!(compute_video_extract_concurrency(8) <= 4);
        assert!(compute_video_extract_concurrency(8) >= 1);
    }

    #[test]
    fn test_video_tool_commands_use_configured_paths() {
        let mut config = UserConfig::default();
        config.ffmpeg_path = " /opt/video/bin/ffmpeg ".into();
        config.ffprobe_path = " /opt/video/bin/ffprobe ".into();

        let tools = video_tool_commands_from_config(&config);

        assert_eq!(tools.ffmpeg, "/opt/video/bin/ffmpeg");
        assert_eq!(tools.ffprobe, "/opt/video/bin/ffprobe");
    }

    #[test]
    fn test_video_tool_commands_derive_ffprobe_from_ffmpeg_dir() {
        let dir = std::env::temp_dir().join(format!(
            "sprite-anime-video-tools-{}",
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let ffmpeg_path = dir.join(if cfg!(windows) {
            "ffmpeg.exe"
        } else {
            "ffmpeg"
        });
        let ffprobe_path = dir.join(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        });
        std::fs::write(&ffmpeg_path, "").unwrap();
        std::fs::write(&ffprobe_path, "").unwrap();

        let mut config = UserConfig::default();
        config.ffmpeg_path = ffmpeg_path.to_string_lossy().to_string();
        let tools = video_tool_commands_from_config(&config);

        assert_eq!(tools.ffmpeg, ffmpeg_path.to_string_lossy().to_string());
        assert_eq!(tools.ffprobe, ffprobe_path.to_string_lossy().to_string());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_probe_and_extract_video_frame_with_ffmpeg_when_available() {
        if !command_is_available("ffmpeg") || !command_is_available("ffprobe") {
            eprintln!("skipping ffmpeg extraction test because ffmpeg/ffprobe is unavailable");
            return;
        }

        let dir = std::env::temp_dir().join(format!(
            "sprite-anime-video-test-{}",
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let video_path = dir.join("sample.mp4");
        let frame_path = dir.join("frame.png");
        let late_frame_path = dir.join("late-frame.png");
        let cropped_frame_path = dir.join("cropped-frame.png");

        let output = Command::new("ffmpeg")
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-y")
            .arg("-f")
            .arg("lavfi")
            .arg("-i")
            .arg("testsrc=size=32x24:rate=2:duration=1")
            .arg("-pix_fmt")
            .arg("yuv420p")
            .arg(&video_path)
            .output()
            .unwrap();
        if !output.status.success() {
            let _ = std::fs::remove_dir_all(&dir);
            panic!(
                "failed to create sample video: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let probe = probe_video_file_inner(&video_path.to_string_lossy(), "ffprobe").unwrap();
        assert_eq!(probe.width, 32);
        assert_eq!(probe.height, 24);
        assert!(probe.duration_seconds > 0.0);

        extract_single_video_frame(
            "ffmpeg",
            &video_path.to_string_lossy(),
            0.0,
            &frame_path,
            "format=rgba",
            None,
        )
        .unwrap();
        let frame = image::open(&frame_path).unwrap();
        assert_eq!(frame.width(), 32);
        assert_eq!(frame.height(), 24);

        extract_single_video_frame(
            "ffmpeg",
            &video_path.to_string_lossy(),
            999.0,
            &late_frame_path,
            "format=rgba",
            None,
        )
        .unwrap();
        let late_frame = image::open(&late_frame_path).unwrap();
        assert_eq!(late_frame.width(), 32);
        assert_eq!(late_frame.height(), 24);

        extract_single_video_frame(
            "ffmpeg",
            &video_path.to_string_lossy(),
            0.0,
            &cropped_frame_path,
            "crop=16:12:0:0,scale=8:6:flags=lanczos,format=rgba",
            None,
        )
        .unwrap();
        let cropped_frame = image::open(&cropped_frame_path).unwrap();
        assert_eq!(cropped_frame.width(), 8);
        assert_eq!(cropped_frame.height(), 6);

        let _ = std::fs::remove_dir_all(&dir);
    }

    fn command_is_available(command: &str) -> bool {
        Command::new(command)
            .arg("-version")
            .output()
            .is_ok_and(|output| output.status.success())
    }
}
