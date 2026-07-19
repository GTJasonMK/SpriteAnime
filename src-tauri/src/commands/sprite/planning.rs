use std::process::Command;

use super::types::{NormalizedVideoExtractRegion, VideoExtractRegion, VideoFrameFilter};

#[derive(Debug, Clone, Copy)]
pub(super) struct NormalizedVideoExtractRequest {
    pub(super) frame_count: usize,
    pub(super) start_seconds: f64,
    pub(super) end_seconds: f64,
}

pub(super) fn normalize_video_extract_request(
    frame_count: usize,
    start_seconds: f64,
    end_seconds: f64,
    duration_seconds: f64,
) -> Result<NormalizedVideoExtractRequest, String> {
    if !(2..=240).contains(&frame_count) {
        return Err(format!(
            "抽帧数量必须在 2 到 240 之间，实际为 {frame_count}"
        ));
    }
    if !duration_seconds.is_finite() || duration_seconds <= 0.0 {
        return Err("视频时长无效".into());
    }
    if !start_seconds.is_finite()
        || !end_seconds.is_finite()
        || start_seconds < 0.0
        || start_seconds >= duration_seconds
        || end_seconds <= start_seconds
        || end_seconds > duration_seconds
    {
        return Err(format!(
            "抽帧时间范围无效：start={start_seconds}, end={end_seconds}, duration={duration_seconds}"
        ));
    }
    let last_safe_time = if duration_seconds > 0.03 {
        duration_seconds - 0.03
    } else {
        duration_seconds
    };
    let end_seconds = end_seconds.min(last_safe_time);
    if end_seconds <= start_seconds {
        return Err("抽帧时间范围距离视频末尾过近，请扩大范围或提前起始时间".into());
    }
    Ok(NormalizedVideoExtractRequest {
        frame_count,
        start_seconds,
        end_seconds,
    })
}

pub(super) fn create_video_sample_times(count: usize, start: f64, end: f64) -> Vec<f64> {
    if count <= 1 || end <= start {
        return vec![start];
    }
    let span = end - start;
    (0..count)
        .map(|index| start + span * (index as f64 / (count - 1) as f64))
        .collect()
}

pub(super) fn build_video_frame_filter(
    crop_region: Option<&VideoExtractRegion>,
    max_extract_edge: Option<u32>,
    source_width: u32,
    source_height: u32,
) -> Result<VideoFrameFilter, String> {
    if source_width == 0 || source_height == 0 {
        return Err("视频源尺寸必须大于 0".into());
    }
    if max_extract_edge.is_some_and(|value| !(1..=4096).contains(&value)) {
        return Err("抽帧最大边长必须在 1 到 4096 之间".into());
    }
    let mut filters = Vec::new();
    let mut width = source_width;
    let mut height = source_height;

    if let Some(region) = crop_region
        .map(|region| normalize_video_extract_region(region, source_width, source_height))
        .transpose()?
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

    if let Some(max_edge) = max_extract_edge.filter(|value| width.max(height) > *value) {
        let (scaled_width, scaled_height) = fit_dimensions(width, height, max_edge);
        filters.push(format!(
            "scale={scaled_width}:{scaled_height}:flags=lanczos"
        ));
        width = scaled_width;
        height = scaled_height;
    }

    filters.push("format=rgba".into());
    Ok(VideoFrameFilter {
        value: filters.join(","),
        width,
        height,
    })
}

fn normalize_video_extract_region(
    region: &VideoExtractRegion,
    source_width: u32,
    source_height: u32,
) -> Result<NormalizedVideoExtractRegion, String> {
    if source_width == 0 || source_height == 0 {
        return Err("视频源尺寸必须大于 0".into());
    }
    if [region.x, region.y, region.width, region.height]
        .iter()
        .any(|value| !value.is_finite())
    {
        return Err("视频裁切区域包含非有限数值".into());
    }
    let x = region.x.round();
    let y = region.y.round();
    let width = region.width.round();
    let height = region.height.round();
    if x < 0.0
        || y < 0.0
        || width < 1.0
        || height < 1.0
        || x + width > f64::from(source_width)
        || y + height > f64::from(source_height)
    {
        return Err(format!(
            "视频裁切区域越界：x={x}, y={y}, width={width}, height={height}, source={source_width}x{source_height}"
        ));
    }
    Ok(NormalizedVideoExtractRegion {
        x: x as u32,
        y: y as u32,
        width: width as u32,
        height: height as u32,
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

pub(super) fn run_ffmpeg_output(
    ffmpeg_command: &str,
    args: &[String],
) -> Result<std::process::Output, String> {
    Command::new(ffmpeg_command)
        .args(args)
        .output()
        .map_err(|e| {
            format!(
                "无法运行 ffmpeg({})，请安装 ffmpeg 或在设置中填写 FFmpeg 路径: {}",
                ffmpeg_command, e
            )
        })
}

pub(super) fn ffmpeg_stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}
