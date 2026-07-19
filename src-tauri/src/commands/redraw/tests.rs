use super::*;
use image::{GenericImageView, RgbaImage};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn create_request(
    total_frames: u32,
    final_cols: u32,
    rows: u32,
    cols: u32,
) -> CreateRedrawRunRequest {
    CreateRedrawRunRequest {
        source_name: "source.mp4".into(),
        total_frames,
        final_cols,
        group_rows: rows,
        group_cols: cols,
        prompt: "像素角色奔跑".into(),
        negative_prompt: String::new(),
        style: "pixel-art".into(),
        resolution: "1K".into(),
        api: RedrawApiSnapshot {
            profile_id: "default".into(),
            api_base: "https://example.test/v1".into(),
            model: "image-test".into(),
            api_mode: "responses".into(),
        },
        extraction: RedrawExtractionSnapshot {
            start_seconds: 0.0,
            end_seconds: 1.0,
        },
    }
}

#[test]
fn manifest_builds_even_four_by_four_batches() {
    let manifest = build_manifest(create_request(16, 4, 2, 2)).unwrap();
    assert_eq!(manifest.final_rows, 4);
    assert_eq!(manifest.batches.len(), 4);
    assert_eq!(manifest.batches[3].global_start, 12);
    assert_eq!(manifest.batches[3].valid_count, 4);
}

#[test]
fn manifest_records_partial_batch_size() {
    let manifest = build_manifest(create_request(10, 4, 2, 2)).unwrap();
    assert_eq!(manifest.final_rows, 3);
    assert_eq!(manifest.batches.len(), 3);
    assert_eq!(manifest.batches[2].valid_count, 2);
}

#[test]
fn manifest_rejects_oversized_group() {
    let err = build_manifest(create_request(16, 4, 4, 4)).unwrap_err();
    assert!(err.contains("每组最多 9 帧"));
}

#[test]
fn manifest_rejects_text_only_image_generation_mode() {
    let mut request = create_request(16, 4, 2, 2);
    request.api.api_mode = "images_generations".into();
    let err = build_manifest(request).unwrap_err();
    assert!(err.contains("不支持 /images/generations"));
}

#[test]
fn first_batch_execution_uses_only_target_grid() {
    let root = temp_dir("first-execution");
    std::fs::create_dir_all(root.join("inputs")).unwrap();
    let mut manifest = build_manifest(create_request(8, 4, 2, 2)).unwrap();
    let input = root.join("inputs/batch_000.png");
    DynamicImage::new_rgba8(8, 8).save(&input).unwrap();
    manifest.batches[0].input_path = input.to_string_lossy().to_string();
    manifest.batches[0].status = "pending".into();

    let (prompt, references) = batch_execution_parameters(&root, &manifest, 0).unwrap();

    assert_eq!(references, vec![input.to_string_lossy().to_string()]);
    assert!(prompt.contains("第一张参考图是本批需要重绘的目标网格"));
    assert!(prompt.contains("建立后续批次必须沿用"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn later_batch_execution_uses_previous_generated_last_frame() {
    let root = temp_dir("later-execution");
    std::fs::create_dir_all(root.join("inputs")).unwrap();
    std::fs::create_dir_all(root.join("frames")).unwrap();
    let mut manifest = build_manifest(create_request(8, 4, 2, 2)).unwrap();
    let input = root.join("inputs/batch_001.png");
    let anchor = root.join("frames/frame_0003.png");
    DynamicImage::new_rgba8(8, 8).save(&input).unwrap();
    manifest.batches[0].status = "succeeded".into();
    manifest.batches[0].valid_count = 4;
    manifest.batches[0].frame_paths = (0..4)
        .map(|index| root.join(format!("frames/frame_{index:04}.png")))
        .map(|path| {
            DynamicImage::new_rgba8(8, 8).save(&path).unwrap();
            path.to_string_lossy().to_string()
        })
        .collect();
    manifest.batches[1].input_path = input.to_string_lossy().to_string();
    manifest.batches[1].status = "pending".into();

    let (prompt, references) = batch_execution_parameters(&root, &manifest, 1).unwrap();

    assert_eq!(references[0], input.to_string_lossy());
    assert_eq!(references[1], anchor.to_string_lossy());
    assert!(prompt.contains("第二张参考图是上一批已经生成的最后一帧"));
    assert!(prompt.contains("不得把第二张参考图作为额外格子输出"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn later_batch_execution_requires_successful_predecessor() {
    let root = temp_dir("ordered-execution");
    std::fs::create_dir_all(root.join("inputs")).unwrap();
    let mut manifest = build_manifest(create_request(8, 4, 2, 2)).unwrap();
    let input = root.join("inputs/batch_001.png");
    DynamicImage::new_rgba8(8, 8).save(&input).unwrap();
    manifest.batches[1].input_path = input.to_string_lossy().to_string();
    manifest.batches[1].status = "pending".into();

    let err = batch_execution_parameters(&root, &manifest, 1).unwrap_err();

    assert!(err.contains("第1批尚未成功"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn batch_execution_rejects_already_completed_batch() {
    let root = temp_dir("completed-execution");
    std::fs::create_dir_all(root.join("inputs")).unwrap();
    let mut manifest = build_manifest(create_request(8, 4, 2, 2)).unwrap();
    let input = root.join("inputs/batch_000.png");
    DynamicImage::new_rgba8(8, 8).save(&input).unwrap();
    manifest.batches[0].input_path = input.to_string_lossy().to_string();
    manifest.batches[0].status = "succeeded".into();

    let err = batch_execution_parameters(&root, &manifest, 0).unwrap_err();

    assert!(err.contains("不允许开始生成"));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn generated_grid_center_crops_to_group_aspect_ratio() {
    for (rows, cols, input, expected, expected_cell) in [
        (2, 2, (1024, 512), (512, 512), (256, 256)),
        (2, 3, (1024, 1024), (1023, 682), (341, 341)),
        (4, 1, (1024, 1024), (256, 1024), (256, 256)),
    ] {
        let manifest = build_manifest(create_request(8, 4, rows, cols)).unwrap();
        let image = DynamicImage::new_rgba8(input.0, input.1);
        let (normalized, cell_width, cell_height) =
            normalize_generated_grid(&manifest, &image).unwrap();

        assert_eq!(normalized.dimensions(), expected);
        assert_eq!((cell_width, cell_height), expected_cell);
    }
}

#[test]
fn split_discards_partial_batch_padding_cells() {
    let root = temp_dir("split-padding");
    std::fs::create_dir_all(root.join("frames")).unwrap();
    let manifest = build_manifest(create_request(10, 4, 2, 2)).unwrap();
    let batch = manifest.batches[2].clone();
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        512,
        512,
        image::Rgba([80, 120, 200, 255]),
    ));

    let paths = split_valid_batch_frames(&root, &image, &manifest, &batch).unwrap();

    assert_eq!(paths.len(), 2);
    assert!(paths.iter().all(|path| Path::new(path).is_file()));
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn final_compose_leaves_unused_cells_transparent() {
    let root = temp_dir("final-transparent");
    let frames_dir = root.join("frames");
    std::fs::create_dir_all(&frames_dir).unwrap();
    let mut manifest = build_manifest(create_request(3, 2, 1, 2)).unwrap();
    for index in 0..3_u32 {
        let path = frames_dir.join(format!("frame_{index}.png"));
        DynamicImage::ImageRgba8(RgbaImage::from_pixel(
            8,
            8,
            image::Rgba([200, 60 + index as u8, 90, 255]),
        ))
        .save(&path)
        .unwrap();
        let batch_index = (index / 2) as usize;
        manifest.batches[batch_index]
            .frame_paths
            .push(path.to_string_lossy().to_string());
    }
    for batch in &mut manifest.batches {
        batch.status = "succeeded".into();
        batch.cell_width = Some(8);
        batch.cell_height = Some(8);
    }

    let image = compose_final_image(&manifest, &root).unwrap();

    assert_eq!(image.dimensions(), (16, 16));
    assert_eq!(image.get_pixel(12, 12).0[3], 0);
    assert_eq!(image.get_pixel(4, 12).0[3], 255);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn manifest_round_trip_preserves_batch_state() {
    let root = temp_dir("manifest-round-trip");
    let manifest = build_manifest(create_request(10, 4, 2, 2)).unwrap();

    save_manifest(&root, &manifest).unwrap();
    let loaded = load_manifest_if_exists(&root).unwrap().unwrap();

    assert_eq!(loaded.id, manifest.id);
    assert_eq!(loaded.batches.len(), 3);
    assert_eq!(loaded.batches[2].valid_count, 2);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn run_file_validation_rejects_paths_outside_expected_root() {
    let root = temp_dir("path-root");
    let outside = temp_dir("path-outside");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    let file = outside.join("frame.png");
    std::fs::write(&file, b"not-an-image").unwrap();

    let err = validate_file_inside(&root, &file, "重绘拆分帧").unwrap_err();

    assert!(err.contains("拒绝读取运行目录之外"));
    let _ = std::fs::remove_dir_all(root);
    let _ = std::fs::remove_dir_all(outside);
}

#[test]
fn long_batch_errors_are_truncated_with_visible_marker() {
    let error = "错".repeat(MAX_BATCH_ERROR_CHARS + 10);
    let truncated = truncate_with_marker(&error, MAX_BATCH_ERROR_CHARS);
    assert!(truncated.ends_with("…（错误信息已截断）"));
    assert_eq!(
        truncated
            .chars()
            .take(MAX_BATCH_ERROR_CHARS)
            .collect::<String>(),
        "错".repeat(MAX_BATCH_ERROR_CHARS)
    );
}

fn temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sprite-redraw-{name}-{}-{stamp}",
        std::process::id()
    ))
}
