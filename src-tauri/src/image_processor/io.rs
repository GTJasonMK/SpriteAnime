use base64::Engine;
use image::DynamicImage;
use std::io::Cursor;
use std::sync::atomic::Ordering;

use super::SAVE_COUNTER;

pub fn require_image_data_url_payload<'a>(
    data_url: &'a str,
    context: &str,
) -> Result<&'a str, String> {
    let value = data_url.trim();
    let (metadata, payload) = value
        .split_once(',')
        .ok_or_else(|| format!("{context}必须是 base64 图片 data URL"))?;
    if !metadata.starts_with("data:image/") || !metadata.ends_with(";base64") {
        return Err(format!("{context}必须是 base64 图片 data URL"));
    }
    let payload = payload.trim();
    if payload.is_empty() {
        return Err(format!("{context}为空"));
    }
    Ok(payload)
}

pub fn resize_image(img: &DynamicImage, resolution: &str) -> Result<DynamicImage, String> {
    if resolution == "原始" {
        return Ok(img.clone());
    }

    let target: u32 = match resolution {
        "1K" => 1024,
        "2K" => 2048,
        _ => return Err(format!("图片分辨率无效：{resolution}")),
    };

    let (w, h) = (img.width(), img.height());
    let max_edge = w.max(h);

    // 已经小于等于目标尺寸
    if max_edge <= target {
        return Ok(img.clone());
    }

    // 按长边缩放
    let (new_w, new_h) = if w >= h {
        (target, (h as f64 * target as f64 / w as f64) as u32)
    } else {
        ((w as f64 * target as f64 / h as f64) as u32, target)
    };

    Ok(img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3))
}

/// 将字节数据转换为图片
pub fn bytes_to_image(data: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory(data).map_err(|e| format!("转换图片失败: {}", e))
}

/// 保存图片到指定目录
pub fn save_image(
    img: &DynamicImage,
    save_dir: &str,
    prefix: &str,
    index: u32,
) -> Result<String, String> {
    std::fs::create_dir_all(save_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let now = chrono::Local::now();
    let timestamp = now.format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let filename = format!(
        "{}_{}_{}_{:04}.png",
        prefix,
        timestamp,
        index,
        nonce % 10_000
    );
    let filepath = std::path::Path::new(save_dir).join(&filename);

    img.save(&filepath)
        .map_err(|e| format!("保存图片失败: {}", e))?;

    Ok(filepath.to_string_lossy().to_string())
}

/// 从文件加载图片。
pub fn load_image(path: &str) -> Result<DynamicImage, String> {
    let img = image::open(path).map_err(|e| format!("加载图片失败: {}", e))?;
    if img.color() != image::ColorType::Rgba8 {
        Ok(DynamicImage::ImageRgba8(img.to_rgba8()))
    } else {
        Ok(img)
    }
}

/// 将图片编码为 PNG 格式的 base64 字符串
pub fn image_to_base64(img: &DynamicImage) -> Result<String, String> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("编码图片失败: {}", e))?;
    Ok(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

/// 将 base64 字符串解码为图片
pub fn base64_to_image(b64: &str) -> Result<DynamicImage, String> {
    let data = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| format!("Base64解码失败: {}", e))?;
    bytes_to_image(&data)
}
