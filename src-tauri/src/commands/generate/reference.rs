use std::io::Cursor;

use super::prompt::{
    REFERENCE_IMAGE_JPEG_QUALITY, REFERENCE_IMAGE_MAX_EDGE, REFERENCE_IMAGE_MAX_PNG_BYTES,
};
use super::types::ReferenceImagePayload;

pub(super) fn build_reference_generation_error(
    err: String,
    reference_images: &[ReferenceImagePayload],
) -> String {
    if reference_images.is_empty() {
        return err;
    }
    let encodings = reference_images
        .iter()
        .enumerate()
        .map(|(index, image)| format!("第{}张 {}", index + 1, image.label))
        .collect::<Vec<_>>()
        .join("；");
    format!(
        "参考图生图请求失败：{err}。参考图编码：{encodings}。解决方法：请确认当前生图模型支持参考图、多模态和多图输入；如果报文过大，请在导入前手动缩小参考图或改用无透明通道的 JPEG/PNG；如果只是想纯文本生图，请移除参考图后重试。"
    )
}

pub(super) fn load_reference_image_payload(path: &str) -> Result<ReferenceImagePayload, String> {
    let img = image::open(path).map_err(|e| format!("加载参考图失败: {}", e))?;
    let original_width = img.width();
    let original_height = img.height();
    let has_transparency = image_has_transparency(&img);

    if has_transparency {
        let png_image = resize_reference_image_to(img, REFERENCE_IMAGE_MAX_EDGE);
        let png = encode_reference_png(&png_image)?;
        if png.len() <= REFERENCE_IMAGE_MAX_PNG_BYTES {
            return Ok(build_reference_payload(
                original_width,
                original_height,
                &png_image,
                "image/png",
                png,
                "png",
            ));
        }
        return Err(format!(
            "参考图包含透明通道，压缩后的 PNG 仍过大({:.1} KiB)。解决方法：请先缩小参考图、裁掉无关空白，或导出为无透明通道的图片后再导入。",
            png.len() as f64 / 1024.0
        ));
    }

    let resized = resize_reference_image_to(img, REFERENCE_IMAGE_MAX_EDGE);
    let jpeg = encode_reference_jpeg(&resized, REFERENCE_IMAGE_JPEG_QUALITY)?;
    Ok(build_reference_payload(
        original_width,
        original_height,
        &resized,
        "image/jpeg",
        jpeg,
        &format!("jpeg-q{REFERENCE_IMAGE_JPEG_QUALITY}"),
    ))
}

fn build_reference_payload(
    original_width: u32,
    original_height: u32,
    img: &image::DynamicImage,
    mime: &'static str,
    bytes: Vec<u8>,
    mode: &str,
) -> ReferenceImagePayload {
    let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let label = format!(
        "{mode} {}x{} -> {}x{} {} {:.1} KiB",
        original_width,
        original_height,
        img.width(),
        img.height(),
        mime,
        base64.len() as f64 / 1024.0
    );
    ReferenceImagePayload {
        data_url: format!("data:{mime};base64,{base64}"),
        bytes,
        mime,
        label,
    }
}

fn resize_reference_image_to(img: image::DynamicImage, max_edge_limit: u32) -> image::DynamicImage {
    let width = img.width();
    let height = img.height();
    let max_edge = width.max(height);
    if max_edge <= max_edge_limit {
        return img;
    }

    let new_width = ((width as u64 * max_edge_limit as u64) / max_edge as u64).max(1) as u32;
    let new_height = ((height as u64 * max_edge_limit as u64) / max_edge as u64).max(1) as u32;
    img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
}

fn image_has_transparency(img: &image::DynamicImage) -> bool {
    img.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255)
}

fn encode_reference_png(img: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("编码参考图 PNG 失败: {}", e))?;
    Ok(buf.into_inner())
}

fn encode_reference_jpeg(img: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = flatten_reference_to_rgb(img);
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(&image::DynamicImage::ImageRgb8(rgb))
        .map_err(|e| format!("编码参考图 JPEG 失败: {}", e))?;
    Ok(buf)
}

fn flatten_reference_to_rgb(img: &image::DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel.0[3]) / 255.0;
        let inv_alpha = 1.0 - alpha;
        let r = (f32::from(pixel.0[0]) * alpha + 255.0 * inv_alpha).round() as u8;
        let g = (f32::from(pixel.0[1]) * alpha + 255.0 * inv_alpha).round() as u8;
        let b = (f32::from(pixel.0[2]) * alpha + 255.0 * inv_alpha).round() as u8;
        rgb.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    rgb
}

/// 根据分辨率和宽高比计算生成尺寸（格式: "WxH"）
pub(super) fn compute_image_size(resolution: &str, ratio: (u32, u32)) -> Result<String, String> {
    let base: u32 = match resolution {
        "2K" => 2048,
        "原始" | "1K" => 1024,
        other => return Err(format!("图片分辨率无效：{other}")),
    };
    let (rw, rh) = ratio;
    if rw >= rh {
        let h = (base as f64 * rh as f64 / rw as f64).round() as u32;
        Ok(format!("{}x{}", base, h))
    } else {
        let w = (base as f64 * rw as f64 / rh as f64).round() as u32;
        Ok(format!("{}x{}", w, base))
    }
}
