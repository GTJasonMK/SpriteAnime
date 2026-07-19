use super::*;

#[test]
fn detects_content_inside_each_grid_cell() {
    let mut image = RgbaImage::from_pixel(12, 6, image::Rgba([255, 255, 255, 255]));
    for y in 1..5 {
        for x in 1..4 {
            image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
        for x in 7..10 {
            image.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
    }
    let layout =
        detect_sprite_layout(&image, 1, 2, None, SpriteBackgroundMode::White, 10, false).unwrap();
    assert_eq!(layout.frame_bounds.len(), 2);
    assert_eq!(layout.empty_count, 0);
    assert_eq!(layout.frame_bounds[0].x, 1);
    assert_eq!(layout.frame_bounds[1].x, 7);
}

#[test]
fn rejects_mismatched_custom_cell_count() {
    let image = RgbaImage::new(8, 4);
    let error = detect_sprite_layout_with_cells(
        &image,
        1,
        2,
        SpriteRegion {
            x: 0,
            y: 0,
            width: 8,
            height: 4,
        },
        Vec::new(),
        "grid".into(),
        SpriteBackgroundMode::Auto,
        32,
        false,
    )
    .unwrap_err();
    assert!(error.contains("网格单元数量"));
}

#[test]
fn rejects_zero_sized_custom_grid_before_cell_reduction() {
    let image = RgbaImage::new(8, 4);
    let error = detect_sprite_layout_with_cells(
        &image,
        0,
        0,
        SpriteRegion {
            x: 0,
            y: 0,
            width: 8,
            height: 4,
        },
        Vec::new(),
        "grid".into(),
        SpriteBackgroundMode::Auto,
        32,
        false,
    )
    .unwrap_err();
    assert!(error.contains("行列数必须为 1 到 20"));
}
