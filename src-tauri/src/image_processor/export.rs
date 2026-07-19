use image::{DynamicImage, RgbaImage};

use super::io::load_image;

#[derive(Debug, Clone)]
pub struct ExportFrameSource {
    pub index: u32,
    pub path: String,
    pub anchor_x: f32,
}

/// 导出选中帧到指定目录。帧图片必须来自已拆分生成的临时帧路径。
pub fn export_frame_sources(
    frames_data: &[ExportFrameSource],
    output_dir: &str,
    base_name: &str,
) -> Result<Vec<String>, String> {
    let prepared = prepare_export_frames(frames_data, output_dir)?;
    let mut saved = Vec::new();
    for (i, frame) in prepared.frames.iter().enumerate() {
        let img = draw_aligned_export_frame(
            frame,
            prepared.canvas_w,
            prepared.canvas_h,
            prepared.anchor_canvas_x,
        );
        let filename = format!("{}_{}.png", base_name, i);
        let filepath = std::path::Path::new(output_dir).join(&filename);
        DynamicImage::ImageRgba8(img)
            .save(&filepath)
            .map_err(|e| format!("保存帧失败: {}", e))?;
        saved.push(filepath.to_string_lossy().to_string());
    }

    Ok(saved)
}

/// 将选中帧导出为循环 GIF。帧会被放入统一画布，并按定位针 X 对齐。
pub fn export_gif_sources(
    frames_data: &[ExportFrameSource],
    output_dir: &str,
    base_name: &str,
    fps: u32,
) -> Result<String, String> {
    if !(1..=60).contains(&fps) {
        return Err(format!("GIF FPS 必须在 1 到 60 之间，实际为 {fps}"));
    }
    let prepared = prepare_export_frames(frames_data, output_dir)?;

    let frame_ms = (1000.0 / fps as f32).round() as u32;
    let delay = image::Delay::from_numer_denom_ms(frame_ms, 1);
    let gif_frames = prepared.frames.iter().map(|frame| {
        image::Frame::from_parts(
            draw_aligned_export_frame(
                frame,
                prepared.canvas_w,
                prepared.canvas_h,
                prepared.anchor_canvas_x,
            ),
            0,
            0,
            delay,
        )
    });

    let filepath = std::path::Path::new(output_dir).join(format!("{}.gif", base_name));
    let file = std::fs::File::create(&filepath).map_err(|e| format!("创建GIF失败: {}", e))?;
    let mut encoder = image::codecs::gif::GifEncoder::new(file);
    encoder
        .set_repeat(image::codecs::gif::Repeat::Infinite)
        .map_err(|e| format!("设置GIF循环失败: {}", e))?;
    encoder
        .encode_frames(gif_frames)
        .map_err(|e| format!("编码GIF失败: {}", e))?;

    Ok(filepath.to_string_lossy().to_string())
}

struct DecodedExportFrame {
    image: RgbaImage,
    anchor_x: f32,
}

struct PreparedExportFrames {
    frames: Vec<DecodedExportFrame>,
    canvas_w: u32,
    canvas_h: u32,
    anchor_canvas_x: u32,
}

fn prepare_export_frames(
    frames_data: &[ExportFrameSource],
    output_dir: &str,
) -> Result<PreparedExportFrames, String> {
    if frames_data.is_empty() {
        return Err("没有可导出的帧".into());
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let frames = decode_export_frames(frames_data)?;
    let (canvas_w, canvas_h, anchor_canvas_x) = get_aligned_canvas_metrics(&frames);
    Ok(PreparedExportFrames {
        frames,
        canvas_w,
        canvas_h,
        anchor_canvas_x,
    })
}

fn decode_export_frames(
    frames_data: &[ExportFrameSource],
) -> Result<Vec<DecodedExportFrame>, String> {
    frames_data
        .iter()
        .map(|source| {
            let path = source.path.trim();
            if path.is_empty() {
                return Err(format!(
                    "第{}帧缺少临时帧路径。解决方法：请重新拆分帧后再导出，不要使用旧的 base64 帧数据。",
                    source.index + 1
                ));
            }
            let image = load_image(path)?.to_rgba8();
            if !source.anchor_x.is_finite() {
                return Err(format!("第{}帧定位针无效", source.index + 1));
            }
            let anchor_x = source.anchor_x.clamp(0.0, image.width() as f32);
            Ok(DecodedExportFrame { image, anchor_x })
        })
        .collect()
}

fn get_aligned_canvas_metrics(frames: &[DecodedExportFrame]) -> (u32, u32, u32) {
    let first = &frames[0];
    let mut anchor_canvas_x = first.anchor_x.ceil() as u32;
    let mut right_span = (first.image.width() as f32 - first.anchor_x)
        .ceil()
        .max(0.0) as u32;
    let mut canvas_h = first.image.height();
    for frame in &frames[1..] {
        anchor_canvas_x = anchor_canvas_x.max(frame.anchor_x.ceil() as u32);
        right_span = right_span.max(
            (frame.image.width() as f32 - frame.anchor_x)
                .ceil()
                .max(0.0) as u32,
        );
        canvas_h = canvas_h.max(frame.image.height());
    }
    let canvas_w = (anchor_canvas_x + right_span).max(1);

    (canvas_w, canvas_h, anchor_canvas_x)
}

fn draw_aligned_export_frame(
    frame: &DecodedExportFrame,
    canvas_w: u32,
    canvas_h: u32,
    anchor_canvas_x: u32,
) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, image::Rgba([0, 0, 0, 0]));
    let x = anchor_canvas_x as i64 - frame.anchor_x.round() as i64;
    let y = canvas_h.saturating_sub(frame.image.height()) as i64;
    image::imageops::overlay(&mut canvas, &frame.image, x, y);
    canvas
}

// ============================================================
// 单元测试
// ============================================================
