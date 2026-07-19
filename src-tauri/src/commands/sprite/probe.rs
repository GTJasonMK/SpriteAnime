use std::path::Path;
use std::process::Command;

use crate::commands::tools::configured_ffmpeg_tools;
use crate::config::AppState;

use super::types::{VideoProbeResult, VideoToolCommands};

pub(super) fn video_tool_commands_from_state(
    state: &AppState,
) -> Result<VideoToolCommands, String> {
    let config = state.config.lock();
    configured_ffmpeg_tools(&config)
}

pub(crate) fn probe_video_file_inner(
    video_path: &str,
    ffprobe_command: &str,
) -> Result<VideoProbeResult, String> {
    if video_path.trim().is_empty() {
        return Err("视频路径为空".into());
    }
    if !Path::new(video_path).is_file() {
        return Err("视频文件不存在".into());
    }

    let value = run_ffprobe_json(
        ffprobe_command,
        &[
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=width,height,duration,nb_frames,avg_frame_rate,r_frame_rate:format=duration",
            "-of",
            "json",
            video_path,
        ],
        &format!(
            "无法运行 ffprobe({})，请安装 ffmpeg 或在设置中填写 FFmpeg/FFprobe 路径",
            ffprobe_command
        ),
        &format!("ffprobe({}) 读取视频失败", ffprobe_command),
        "解析 ffprobe 输出失败",
    )?;
    let stream = value
        .get("streams")
        .and_then(|streams| streams.as_array())
        .and_then(|streams| streams.first())
        .ok_or_else(|| "视频中没有可用的视频流".to_string())?;
    let width = extract_required_video_dimension(stream, "width")?;
    let height = extract_required_video_dimension(stream, "height")?;
    let duration = extract_required_video_duration(&value, stream)?;

    Ok(VideoProbeResult {
        duration_seconds: duration,
        width,
        height,
    })
}

pub(super) fn extract_required_video_dimension(
    stream: &serde_json::Value,
    field_name: &str,
) -> Result<u32, String> {
    let label = match field_name {
        "width" => "宽度",
        "height" => "高度",
        _ => field_name,
    };
    let value = stream.get(field_name).ok_or_else(|| {
        format!(
            "视频{label}元数据缺失（ffprobe 字段 stream.{field_name}）。解决方法：请确认视频文件包含有效的视频流尺寸；可用 ffmpeg -i input -c copy output.mp4 重新封装后再导入，或换用可被 ffprobe 读取宽高的视频文件。"
        )
    })?;
    let dimension = value
        .as_u64()
        .filter(|dimension| *dimension > 0 && *dimension <= u64::from(u32::MAX));
    dimension.map(|dimension| dimension as u32).ok_or_else(|| {
        format!(
            "视频{label}元数据无效（ffprobe 字段 stream.{field_name} 必须是大于 0 的整数）。解决方法：请用 ffmpeg 重新导出或重新封装视频，确保 ffprobe 能读取有效宽高后再导入。"
        )
    })
}

pub(super) fn extract_required_video_duration(
    value: &serde_json::Value,
    stream: &serde_json::Value,
) -> Result<f64, String> {
    value
        .pointer("/format/duration")
        .and_then(parse_json_number_like)
        .or_else(|| stream.get("duration").and_then(parse_json_number_like))
        .filter(|duration| *duration > 0.0)
        .ok_or_else(|| {
            "视频时长元数据缺失或无效。解决方法：请用 ffmpeg 重新封装视频（例如 ffmpeg -i input -c copy output.mp4）后再导入，或换用带有有效 duration 元数据的视频文件。".into()
        })
}

fn run_ffprobe_json(
    ffprobe_command: &str,
    args: &[&str],
    run_error: &str,
    status_error: &str,
    parse_error: &str,
) -> Result<serde_json::Value, String> {
    let output = Command::new(ffprobe_command)
        .args(args)
        .output()
        .map_err(|e| format!("{}: {}", run_error, e))?;

    if !output.status.success() {
        return Err(format!(
            "{}: {}",
            status_error,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    serde_json::from_slice(&output.stdout).map_err(|e| format!("{}: {}", parse_error, e))
}

fn parse_json_number_like(value: &serde_json::Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
        .filter(|value| value.is_finite())
}
