use super::extraction::*;
use super::*;
use super::{planning::*, probe::*, storage::*};
use image::{DynamicImage, Rgba, RgbaImage};
use std::path::Path;
use std::process::Command;

struct TestProgress;

impl crate::runtime::ProgressReporter for TestProgress {
    fn emit(&self, _event: crate::runtime::ProgressEvent) -> crate::runtime::AppResult<()> {
        Ok(())
    }
}

fn marked_image() -> DynamicImage {
    let mut img = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
    img.put_pixel(0, 0, Rgba([200, 0, 0, 255]));
    img.put_pixel(3, 3, Rgba([0, 0, 200, 255]));
    DynamicImage::ImageRgba8(img)
}

#[test]
fn required_export_asset_name_rejects_empty_sanitized_name() {
    let err = required_export_asset_name(" --- ___ ... ", "导出序列帧文件夹名称").unwrap_err();

    assert!(err.contains("导出序列帧文件夹名称清洗后为空"));
    assert!(err.contains("包含有效字符"));
    assert!(err.contains("非法路径字符"));
}

#[test]
fn required_export_asset_name_accepts_and_sanitizes_valid_name() {
    let name = required_export_asset_name(" walk cycle?.png ", "导出 GIF 文件名").unwrap();

    assert_eq!(name, "walk cycle");
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
        anchor_x: 2.0,
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
        anchor_x: 2.0,
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
        anchor_x: 2.0,
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
fn test_batch_extract_rounds_up_eof_sample_to_requested_count() {
    if !command_is_available("ffmpeg") {
        eprintln!("skipping batch extraction test because ffmpeg is unavailable");
        return;
    }

    let root = std::env::temp_dir().join(format!(
        "sprite-anime-video-batch-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let output_dir = root.join("frames");
    std::fs::create_dir_all(&output_dir).unwrap();
    let video_path = root.join("sample.mp4");
    let create = Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=32x24:rate=24:duration=4.042",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(&video_path)
        .output()
        .unwrap();
    assert!(
        create.status.success(),
        "failed to create sample video: {}",
        String::from_utf8_lossy(&create.stderr)
    );

    let times = create_video_sample_times(25, 0.0, 4.012);
    let frames = extract_video_frames_batch(
        &root,
        &TestProgress,
        "ffmpeg",
        &video_path.to_string_lossy(),
        &output_dir,
        &times,
        "format=rgba",
    )
    .unwrap();

    assert_eq!(frames.len(), 25);
    assert!(frames.iter().all(|frame| Path::new(&frame.path).is_file()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn test_normalize_video_extract_request_accepts_current_range_contract() {
    let request = normalize_video_extract_request(4, 1.0, 10.0, 10.0).unwrap();

    assert_eq!(request.frame_count, 4);
    assert!((request.start_seconds - 1.0).abs() < f64::EPSILON);
    assert!((request.end_seconds - 9.97).abs() < 0.0001);
}

#[test]
fn test_normalize_video_extract_request_rejects_invalid_count_and_range() {
    assert!(normalize_video_extract_request(1, 1.0, 2.0, 10.0).is_err());
    assert!(normalize_video_extract_request(4, 2.0, 2.0, 10.0).is_err());
    assert!(normalize_video_extract_request(4, -1.0, 2.0, 10.0).is_err());
    assert!(normalize_video_extract_request(4, 1.0, 11.0, 10.0).is_err());
}

#[test]
fn test_normalize_video_extract_request_rejects_too_short_tail_range() {
    let err = normalize_video_extract_request(4, 9.98, 10.0, 10.0).unwrap_err();

    assert!(err.contains("距离视频末尾过近"));
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
    )
    .unwrap();

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
    )
    .unwrap();

    assert_eq!(filter.value, "format=rgba");
    assert_eq!(filter.width, 640);
    assert_eq!(filter.height, 480);
}

#[test]
fn test_probe_duration_uses_format_duration() {
    let value = serde_json::json!({
        "format": {
            "duration": "14.767000"
        },
        "streams": [
            {
                "width": 2880,
                "height": 1800
            }
        ]
    });
    let stream = value["streams"].as_array().unwrap().first().unwrap();

    let duration = extract_required_video_duration(&value, stream).unwrap();

    assert!((duration - 14.767).abs() < 0.001);
}

#[test]
fn test_probe_duration_uses_stream_duration() {
    let value = serde_json::json!({
        "format": {},
        "streams": [
            {
                "width": 2880,
                "height": 1800,
                "duration": "8.250000"
            }
        ]
    });
    let stream = value["streams"].as_array().unwrap().first().unwrap();

    let duration = extract_required_video_duration(&value, stream).unwrap();

    assert!((duration - 8.25).abs() < 0.001);
}

#[test]
fn test_probe_duration_rejects_missing_duration_metadata() {
    let value = serde_json::json!({
        "format": {},
        "streams": [
            {
                "width": 2880,
                "height": 1800,
                "avg_frame_rate": "30/1",
                "nb_frames": "443"
            }
        ]
    });
    let stream = value["streams"].as_array().unwrap().first().unwrap();

    let err = extract_required_video_duration(&value, stream).unwrap_err();

    assert!(err.contains("视频时长元数据缺失或无效"));
    assert!(err.contains("ffmpeg -i input -c copy output.mp4"));
}

#[test]
fn test_probe_dimension_rejects_missing_width_metadata() {
    let stream = serde_json::json!({
        "height": 1800,
        "duration": "14.767000"
    });

    let err = extract_required_video_dimension(&stream, "width").unwrap_err();

    assert!(err.contains("视频宽度元数据缺失"));
    assert!(err.contains("stream.width"));
    assert!(err.contains("重新封装后再导入"));
}

#[test]
fn test_probe_dimension_rejects_zero_height_metadata() {
    let stream = serde_json::json!({
        "width": 2880,
        "height": 0,
        "duration": "14.767000"
    });

    let err = extract_required_video_dimension(&stream, "height").unwrap_err();

    assert!(err.contains("视频高度元数据无效"));
    assert!(err.contains("stream.height"));
    assert!(err.contains("大于 0 的整数"));
    assert!(err.contains("重新导出或重新封装"));
}

#[test]
fn test_probe_video_file_with_ffmpeg_when_available() {
    if !command_is_available("ffmpeg") || !command_is_available("ffprobe") {
        eprintln!("skipping ffmpeg extraction test because ffmpeg/ffprobe is unavailable");
        return;
    }

    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-video-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let video_path = dir.join("sample.mp4");
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

    let _ = std::fs::remove_dir_all(&dir);
}

fn command_is_available(command: &str) -> bool {
    Command::new(command)
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}
