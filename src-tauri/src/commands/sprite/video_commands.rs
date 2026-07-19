use tauri::ipc::Channel;
use tauri::{command, State};

use crate::config::AppState;
use crate::logger::summarize_log_text;

use super::extraction::extract_video_frames_with_ffmpeg_blocking;
use super::probe::{probe_video_file_inner, video_tool_commands_from_state};
use super::storage::{append_video_sprite_log, append_video_sprite_log_to_dir};
use super::types::{VideoExtractEvent, VideoExtractRegion, VideoFramesResult, VideoProbeResult};

/// 用 ffprobe 读取视频元数据。
#[command]
pub async fn probe_video_file(
    state: State<'_, AppState>,
    video_path: String,
) -> Result<VideoProbeResult, String> {
    let tools = video_tool_commands_from_state(&state)?;
    let log_dir = state.log_dir.clone();
    append_video_sprite_log_to_dir(
        &log_dir,
        &format!(
            "probe start | path={} ffprobe={}",
            video_path,
            summarize_log_text(&tools.ffprobe)
        ),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let result = probe_video_file_inner(&video_path, &tools.ffprobe);
        match result {
            Ok(probe) => {
                append_video_sprite_log_to_dir(
                    &log_dir,
                    &format!(
                        "probe ok | path={} duration={:.3}s size={}x{}",
                        video_path, probe.duration_seconds, probe.width, probe.height
                    ),
                )?;
                Ok(probe)
            }
            Err(err) => {
                append_video_sprite_log_to_dir(
                    &log_dir,
                    &format!("probe failed | path={video_path} error={err}"),
                )
                .map_err(|log_err| format!("{err}; 写入视频日志失败: {log_err}"))?;
                Err(err)
            }
        }
    })
    .await
    .map_err(|e| format!("视频元数据任务执行失败: {e}"))?
}

/// 用 ffmpeg 从视频中按时间均匀抽取 PNG 帧。
#[command]
#[allow(clippy::too_many_arguments)]
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
    let tools = video_tool_commands_from_state(&state)?;
    let log_dir = state.log_dir.clone();
    let app_data_dir = state.app_data_dir.clone();
    append_video_sprite_log_to_dir(
        &log_dir,
        &format!(
            "extract request | path={} frame_count={} start={:.3} end={:.3}",
            video_path, frame_count, start_seconds, end_seconds
        ),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        extract_video_frames_with_ffmpeg_blocking(
            log_dir,
            app_data_dir,
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
pub fn log_video_sprite_message(state: State<'_, AppState>, message: String) -> Result<(), String> {
    append_video_sprite_log(
        &state,
        &format!("frontend | {}", summarize_log_text(&message)),
    )
}
