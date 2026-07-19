use std::path::{Path, PathBuf};
use tauri::ipc::Channel;

use crate::logger::summarize_log_text;

use super::planning::{
    build_video_frame_filter, create_video_sample_times, ffmpeg_stderr,
    normalize_video_extract_request, run_ffmpeg_output,
};
use super::probe::probe_video_file_inner;
use super::storage::{
    append_video_sprite_log_to_dir, build_video_batch_extract_error, create_temp_video_frame_dir,
};
use super::types::{
    VideoExtractEvent, VideoExtractRegion, VideoFrameFile, VideoFramesResult, VideoToolCommands,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn extract_video_frames_with_ffmpeg_blocking(
    log_dir: PathBuf,
    app_data_dir: PathBuf,
    tools: VideoToolCommands,
    channel: Channel<VideoExtractEvent>,
    video_path: String,
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    crop_region: Option<VideoExtractRegion>,
    max_extract_edge: Option<u32>,
) -> Result<VideoFramesResult, String> {
    let progress = VideoExtractChannelProgress { channel: &channel };
    extract_video_frames_blocking(
        log_dir,
        app_data_dir,
        tools,
        &progress,
        video_path,
        frame_count,
        start_seconds,
        end_seconds,
        crop_region,
        max_extract_edge,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn extract_video_frames_blocking(
    log_dir: PathBuf,
    app_data_dir: PathBuf,
    tools: VideoToolCommands,
    progress: &dyn crate::runtime::ProgressReporter,
    video_path: String,
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    crop_region: Option<VideoExtractRegion>,
    max_extract_edge: Option<u32>,
) -> Result<VideoFramesResult, String> {
    let mut output_dir_for_cleanup: Option<PathBuf> = None;
    let result: Result<VideoFramesResult, String> = (|| {
        progress
            .emit(crate::runtime::ProgressEvent::stage(
                "probing",
                "正在读取视频元数据",
            ))
            .map_err(|error| error.to_string())?;
        let probe = probe_video_file_inner(&video_path, &tools.ffprobe)?;
        let normalized = normalize_video_extract_request(
            frame_count,
            start_seconds,
            end_seconds,
            probe.duration_seconds,
        )?;

        let output_dir = create_temp_video_frame_dir(&app_data_dir)?;
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
        )?;
        append_video_sprite_log_to_dir(
            &log_dir,
            &format!(
                "extract normalized | mode={} duration={:.3}s size={}x{} frame_count={} start={:.3} end={:.3} filter={} output_size={}x{} ffmpeg={} output_dir={}",
                "range",
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
        )?;
        let frames = match extract_video_frames_batch(
            &log_dir,
            progress,
            &tools.ffmpeg,
            &video_path,
            &output_dir,
            &times,
            &frame_filter.value,
        ) {
            Ok(frames) => frames,
            Err(err) => {
                let error = build_video_batch_extract_error(&err, &tools.ffmpeg);
                append_video_sprite_log_to_dir(
                    &log_dir,
                    &format!("batch extract failed | error={err}"),
                )
                .map_err(|log_err| format!("{error}; 写入视频日志失败: {log_err}"))?;
                return Err(error);
            }
        };

        append_video_sprite_log_to_dir(
            &log_dir,
            &format!(
                "extract ok | frames={} output_dir={}",
                frames.len(),
                output_dir.display()
            ),
        )?;
        progress
            .emit(crate::runtime::ProgressEvent::counted(
                "completed",
                "视频抽帧完成",
                frames.len() as u64,
                frames.len() as u64,
            ))
            .map_err(|error| error.to_string())?;
        Ok(VideoFramesResult {
            frames,
            duration_seconds: probe.duration_seconds,
            width: probe.width,
            height: probe.height,
            output_dir: output_dir.to_string_lossy().to_string(),
        })
    })();

    if let Err(err) = result {
        let mut error = err;
        if let Some(output_dir) = output_dir_for_cleanup {
            match std::fs::remove_dir_all(&output_dir) {
                Ok(()) => {
                    if let Err(log_err) = append_video_sprite_log_to_dir(
                        &log_dir,
                        &format!(
                            "extract cleanup after error ok | output_dir={}",
                            output_dir.display()
                        ),
                    ) {
                        error.push_str(&format!("; 写入视频日志失败: {log_err}"));
                    }
                }
                Err(cleanup_err) => error.push_str(&format!(
                    "; 清理失败目录 {} 时出错: {cleanup_err}",
                    output_dir.display()
                )),
            }
        }
        return Err(error);
    }
    result
}

pub(super) fn extract_video_frames_batch(
    log_dir: &Path,
    progress: &dyn crate::runtime::ProgressReporter,
    ffmpeg_command: &str,
    video_path: &str,
    output_dir: &Path,
    times: &[f64],
    filter: &str,
) -> Result<Vec<VideoFrameFile>, String> {
    if times.len() <= 1 {
        return Err("批量抽帧至少需要 2 个时间点".into());
    }

    let start = times[0].max(0.0);
    let end = times[times.len() - 1].max(start);
    let span = (end - start).max(0.001);
    let fps = ((times.len() - 1) as f64 / span).max(0.001);
    let sampling_filter = format!("fps={fps:.6}:round=up");
    let pattern = output_dir.join("frame_%04d.png");
    progress
        .emit(crate::runtime::ProgressEvent::timed_counted(
            "extracting_frame",
            "正在批量抽帧",
            1,
            times.len() as u64,
            start,
        ))
        .map_err(|error| error.to_string())?;
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "ffmpeg batch command | bin={} start={:.3}s end={:.3}s frames={} sampling={} filter={} output={}",
            summarize_log_text(ffmpeg_command),
            start,
            end,
            times.len(),
            sampling_filter,
            summarize_log_text(filter),
            pattern.display()
        ),
    )?;

    let pattern_arg = pattern.to_string_lossy().to_string();
    let output = run_ffmpeg_output(
        ffmpeg_command,
        &[
            "-hide_banner".into(),
            "-loglevel".into(),
            "error".into(),
            "-nostdin".into(),
            "-y".into(),
            "-ss".into(),
            format!("{start:.3}"),
            "-i".into(),
            video_path.into(),
            "-map".into(),
            "0:v:0".into(),
            "-an".into(),
            "-sn".into(),
            "-dn".into(),
            "-vf".into(),
            format!("{sampling_filter},{filter}"),
            "-frames:v".into(),
            times.len().to_string(),
            "-start_number".into(),
            "0".into(),
            pattern_arg,
        ],
    )?;

    let stderr = ffmpeg_stderr(&output);
    append_video_sprite_log_to_dir(
        log_dir,
        &format!(
            "ffmpeg batch result | status={} stderr={}",
            output.status,
            summarize_log_text(&stderr)
        ),
    )?;
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
        )?;
        frames.push(VideoFrameFile {
            path: output_path.to_string_lossy().to_string(),
            time_seconds,
            width,
            height,
        });
    }
    Ok(frames)
}

struct VideoExtractChannelProgress<'a> {
    channel: &'a Channel<VideoExtractEvent>,
}

impl crate::runtime::ProgressReporter for VideoExtractChannelProgress<'_> {
    fn emit(&self, event: crate::runtime::ProgressEvent) -> crate::runtime::AppResult<()> {
        let frontend_event = match event.stage.as_str() {
            "probing" => VideoExtractEvent::Probing,
            "extracting_frame" => VideoExtractEvent::ExtractingFrame {
                index: required_event_count(event.current, "抽帧当前序号")?,
                total: required_event_count(event.total, "抽帧总数")?,
                time_seconds: required_event_time(event.time_seconds)?,
            },
            "completed" => VideoExtractEvent::Completed {
                frames: required_event_count(event.current, "抽帧完成数量")?,
            },
            other => {
                return Err(crate::runtime::AppError::internal(format!(
                    "未知视频抽帧进度阶段：{other}"
                )))
            }
        };
        self.channel.send(frontend_event).map_err(|error| {
            crate::runtime::AppError::internal(format!("发送视频抽帧进度失败: {error}"))
        })
    }
}

fn required_event_count(value: Option<u64>, label: &str) -> crate::runtime::AppResult<usize> {
    value
        .map(|value| value as usize)
        .ok_or_else(|| crate::runtime::AppError::internal(format!("{label}缺失")))
}

fn required_event_time(value: Option<f64>) -> crate::runtime::AppResult<f64> {
    value.ok_or_else(|| crate::runtime::AppError::internal("视频抽帧进度缺少时间"))
}
