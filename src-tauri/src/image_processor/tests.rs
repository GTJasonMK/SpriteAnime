use super::*;
use base64::Engine;
use image::{DynamicImage, RgbaImage};
use std::path::Path;

/// 创建测试用的纯色 RGBA 图片
fn make_test_image(w: u32, h: u32) -> DynamicImage {
    let img = RgbaImage::from_pixel(w, h, image::Rgba([198, 97, 63, 255]));
    DynamicImage::ImageRgba8(img)
}

fn write_test_frame(dir: &Path, name: &str, img: &DynamicImage) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    img.save(&path).unwrap();
    path.to_string_lossy().to_string()
}

#[test]
fn test_resize_image_original() {
    let img = make_test_image(4096, 4096);
    let result = resize_image(&img, "原始").unwrap();
    // "原始" 不缩放
    assert_eq!(result.width(), 4096);
    assert_eq!(result.height(), 4096);
}

#[test]
fn test_resize_image_1k() {
    let img = make_test_image(4096, 2048);
    let result = resize_image(&img, "1K").unwrap();
    // 长边缩放到1024
    assert_eq!(result.width(), 1024);
    assert_eq!(result.height(), 512);
}

#[test]
fn test_resize_image_2k_already_smaller() {
    let img = make_test_image(512, 512);
    let result = resize_image(&img, "2K").unwrap();
    // 已小于目标，不缩放
    assert_eq!(result.width(), 512);
    assert_eq!(result.height(), 512);
}

#[test]
fn resize_image_rejects_unknown_resolution() {
    let err = resize_image(&make_test_image(32, 32), "auto").unwrap_err();
    assert_eq!(err, "图片分辨率无效：auto");
}

#[test]
fn image_data_url_payload_requires_current_data_url_contract() {
    assert_eq!(
        require_image_data_url_payload("data:image/png;base64, YWJj ", "图片数据").unwrap(),
        "YWJj"
    );
    assert!(require_image_data_url_payload("YWJj", "图片数据")
        .unwrap_err()
        .contains("必须是 base64 图片 data URL"));
    assert!(
        require_image_data_url_payload("data:text/plain;base64,YWJj", "图片数据")
            .unwrap_err()
            .contains("必须是 base64 图片 data URL")
    );
}

#[test]
fn test_save_image_generates_unique_paths_for_rapid_saves() {
    while chrono::Local::now().timestamp_subsec_millis() > 900 {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-save-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let img = make_test_image(16, 16);
    let first = save_image(&img, &dir.to_string_lossy(), "test", 1).unwrap();
    let second = save_image(&img, &dir.to_string_lossy(), "test", 1).unwrap();

    let _ = std::fs::remove_dir_all(&dir);
    assert_ne!(first, second);
}

#[test]
fn save_transparent_copy_to_dir_rejects_source_without_file_stem() {
    let img = make_test_image(16, 16);
    let output_dir = std::env::temp_dir().join(format!(
        "sprite-anime-transparent-missing-stem-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));

    let err = save_transparent_copy_to_dir(&img, Path::new("/"), &output_dir).unwrap_err();

    assert!(err.contains("透明背景保存缺少源文件名"));
    assert!(err.contains("带文件名的本地文件"));
    assert!(err.contains("重新抠图保存"));
}

#[test]
fn test_image_to_base64_and_back() {
    let img = make_test_image(64, 64);
    let b64 = image_to_base64(&img).unwrap();
    assert!(!b64.is_empty());
    // 往返转换
    let decoded = base64_to_image(&b64).unwrap();
    assert_eq!(decoded.width(), 64);
    assert_eq!(decoded.height(), 64);
}

#[test]
fn test_export_frames_uses_folder_name_style_sequence() {
    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-export-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let img = make_test_image(8, 8);
    let first_path = write_test_frame(&dir, "frame0.png", &img);
    let second_path = write_test_frame(&dir, "frame1.png", &img);
    let frames = vec![
        ExportFrameSource {
            index: 0,
            path: first_path,
            anchor_x: 4.0,
        },
        ExportFrameSource {
            index: 1,
            path: second_path,
            anchor_x: 4.0,
        },
    ];

    let saved = export_frame_sources(&frames, &dir.to_string_lossy(), "walk_cycle").unwrap();

    assert_eq!(saved.len(), 2);
    assert!(dir.join("walk_cycle_0.png").exists());
    assert!(dir.join("walk_cycle_1.png").exists());
    assert!(saved[0].ends_with("walk_cycle_0.png"));
    assert!(saved[1].ends_with("walk_cycle_1.png"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_frames_uses_anchor_aligned_canvas() {
    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-export-anchor-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let red =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, image::Rgba([220, 42, 42, 255])));
    let blue =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 6, image::Rgba([42, 80, 220, 255])));
    let red_path = write_test_frame(&dir, "red.png", &red);
    let blue_path = write_test_frame(&dir, "blue.png", &blue);
    let frames = vec![
        ExportFrameSource {
            index: 0,
            path: red_path,
            anchor_x: 1.0,
        },
        ExportFrameSource {
            index: 1,
            path: blue_path,
            anchor_x: 6.0,
        },
    ];

    let saved = export_frame_sources(&frames, &dir.to_string_lossy(), "aligned").unwrap();
    let first = image::open(&saved[0]).unwrap().to_rgba8();
    let second = image::open(&saved[1]).unwrap().to_rgba8();

    assert_eq!(first.dimensions(), (9, 6));
    assert_eq!(second.dimensions(), (9, 6));
    assert_eq!(first.get_pixel(5, 2).0, [220, 42, 42, 255]);
    assert_eq!(first.get_pixel(4, 2).0[3], 0);
    assert_eq!(second.get_pixel(0, 0).0, [42, 80, 220, 255]);
    assert_eq!(second.get_pixel(8, 0).0[3], 0);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_gif_creates_looping_animation_file() {
    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-gif-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let first = make_test_image(8, 8);
    let second =
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, image::Rgba([42, 80, 220, 255])));
    let first_path = write_test_frame(&dir, "first.png", &first);
    let second_path = write_test_frame(&dir, "second.png", &second);
    let frames = vec![
        ExportFrameSource {
            index: 0,
            path: first_path,
            anchor_x: 4.0,
        },
        ExportFrameSource {
            index: 1,
            path: second_path,
            anchor_x: 4.0,
        },
    ];

    let saved = export_gif_sources(&frames, &dir.to_string_lossy(), "walk_cycle", 12).unwrap();
    let metadata = std::fs::metadata(dir.join("walk_cycle.gif")).unwrap();

    assert!(saved.ends_with("walk_cycle.gif"));
    assert!(metadata.len() > 0);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_export_frames_rejects_missing_frame_path() {
    let dir = std::env::temp_dir().join(format!(
        "sprite-anime-export-missing-path-test-{}",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ));
    let frames = vec![ExportFrameSource {
        index: 0,
        path: String::new(),
        anchor_x: 0.0,
    }];

    let err = export_frame_sources(&frames, &dir.to_string_lossy(), "missing").unwrap_err();

    assert!(err.contains("缺少临时帧路径"));
    assert!(err.contains("重新拆分帧"));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_bytes_to_image_valid() {
    let img = make_test_image(32, 32);
    let b64 = image_to_base64(&img).unwrap();
    let data = base64::engine::general_purpose::STANDARD
        .decode(&b64)
        .unwrap();
    let decoded = bytes_to_image(&data).unwrap();
    assert_eq!(decoded.width(), 32);
}

#[test]
fn test_bytes_to_image_invalid() {
    let result = bytes_to_image(b"not an image");
    assert!(result.is_err());
}

#[test]
fn test_make_background_transparent_keeps_internal_white_pixels() {
    let mut img = RgbaImage::from_pixel(7, 7, image::Rgba([255, 255, 255, 255]));

    for y in 2..=4 {
        for x in 2..=4 {
            img.put_pixel(x, y, image::Rgba([220, 42, 42, 255]));
        }
    }
    img.put_pixel(3, 3, image::Rgba([255, 255, 255, 255]));

    let result = make_background_transparent(
        &DynamicImage::ImageRgba8(img),
        TransparentBackgroundOptions {
            tolerance: 8,
            feather_radius: 0,
            color_key_mode: ColorKeyMode::Auto,
        },
    );
    let output = result.image.to_rgba8();

    assert_eq!(result.background_rgb, [255, 255, 255]);
    assert_eq!(output.get_pixel(0, 0).0[3], 0);
    assert_eq!(output.get_pixel(2, 2).0[3], 255);
    assert_eq!(output.get_pixel(3, 3).0, [255, 255, 255, 255]);
}

#[test]
fn test_make_background_transparent_detects_non_white_background() {
    let mut img = RgbaImage::from_pixel(8, 8, image::Rgba([27, 220, 111, 255]));
    for y in 3..=4 {
        for x in 3..=4 {
            img.put_pixel(x, y, image::Rgba([42, 80, 220, 255]));
        }
    }

    let result = make_background_transparent(
        &DynamicImage::ImageRgba8(img),
        TransparentBackgroundOptions {
            tolerance: 10,
            feather_radius: 0,
            color_key_mode: ColorKeyMode::Auto,
        },
    );
    let output = result.image.to_rgba8();

    assert_eq!(result.background_rgb, [27, 220, 111]);
    assert_eq!(output.get_pixel(0, 0).0[3], 0);
    assert_eq!(output.get_pixel(3, 3).0[3], 255);
    assert_eq!(result.transparent_pixels, 60);
}

#[test]
fn test_make_background_transparent_removes_chroma_background_holes() {
    let green = image::Rgba([27, 220, 111, 255]);
    let blue = image::Rgba([42, 80, 220, 255]);
    let mut img = RgbaImage::from_pixel(7, 7, green);

    for y in 2..=4 {
        for x in 2..=4 {
            img.put_pixel(x, y, blue);
        }
    }
    img.put_pixel(3, 3, green);

    let result = make_background_transparent(
        &DynamicImage::ImageRgba8(img),
        TransparentBackgroundOptions {
            tolerance: 10,
            feather_radius: 0,
            color_key_mode: ColorKeyMode::Auto,
        },
    );
    let output = result.image.to_rgba8();

    assert_eq!(output.get_pixel(3, 3).0[3], 0);
    assert_eq!(output.get_pixel(2, 2).0[3], 255);
}

#[test]
fn test_make_background_transparent_uses_tolerance_for_near_background() {
    let mut img = RgbaImage::from_pixel(5, 5, image::Rgba([255, 255, 255, 255]));
    img.put_pixel(1, 1, image::Rgba([248, 249, 255, 255]));
    img.put_pixel(2, 2, image::Rgba([20, 20, 20, 255]));

    let result = make_background_transparent(
        &DynamicImage::ImageRgba8(img),
        TransparentBackgroundOptions {
            tolerance: 12,
            feather_radius: 0,
            color_key_mode: ColorKeyMode::Auto,
        },
    );
    let output = result.image.to_rgba8();

    assert_eq!(output.get_pixel(1, 1).0[3], 0);
    assert_eq!(output.get_pixel(2, 2).0[3], 255);
}
