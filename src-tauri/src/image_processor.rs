use base64::Engine;
use image::DynamicImage;
use image::RgbaImage;
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub struct TransparentBackgroundOptions {
    pub tolerance: u8,
    pub feather_radius: u8,
    pub color_key_mode: ColorKeyMode,
}

impl Default for TransparentBackgroundOptions {
    fn default() -> Self {
        Self {
            tolerance: 36,
            feather_radius: 1,
            color_key_mode: ColorKeyMode::Auto,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorKeyMode {
    EdgeOnly,
    Auto,
    Global,
}

#[derive(Debug, Clone)]
pub struct TransparentBackgroundResult {
    pub image: DynamicImage,
    pub background_rgb: [u8; 3],
    pub transparent_pixels: u32,
}

/// 调整图片分辨率
pub fn resize_image(img: &DynamicImage, resolution: &str) -> DynamicImage {
    if resolution == "原始" {
        return img.clone();
    }

    let target: u32 = match resolution {
        "1K" => 1024,
        "2K" => 2048,
        _ => return img.clone(),
    };

    let (w, h) = (img.width(), img.height());
    let max_edge = w.max(h);

    // 已经小于等于目标尺寸
    if max_edge <= target {
        return img.clone();
    }

    // 按长边缩放
    let (new_w, new_h) = if w >= h {
        (target, (h as f64 * target as f64 / w as f64) as u32)
    } else {
        ((w as f64 * target as f64 / h as f64) as u32, target)
    };

    img.resize_exact(new_w, new_h, image::imageops::FilterType::Lanczos3)
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

/// 将纯色或高对比背景转为透明。纯白/灰阶背景默认只处理与画布边缘
/// 连通的区域，避免误删角色内部高光；高饱和背景会自动启用全图颜色键。
pub fn make_background_transparent(
    img: &DynamicImage,
    options: TransparentBackgroundOptions,
) -> TransparentBackgroundResult {
    let mut rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return TransparentBackgroundResult {
            image: DynamicImage::ImageRgba8(rgba),
            background_rgb: [255, 255, 255],
            transparent_pixels: 0,
        };
    }

    let background_rgb = estimate_background_rgb(&rgba);
    let mut mask = detect_edge_connected_background(&rgba, background_rgb, options.tolerance);
    if should_apply_global_color_key(background_rgb, options.color_key_mode) {
        extend_mask_with_global_color_key(&rgba, background_rgb, options.tolerance, &mut mask);
    }
    let transparent_pixels = mask.iter().filter(|item| **item).count() as u32;

    apply_transparency_mask(&mut rgba, &mask, background_rgb, options);

    TransparentBackgroundResult {
        image: DynamicImage::ImageRgba8(rgba),
        background_rgb,
        transparent_pixels,
    }
}

/// 将处理后的透明背景图片保存到源文件同目录，始终输出 PNG。
pub fn save_transparent_copy(img: &DynamicImage, source_path: &str) -> Result<String, String> {
    let source = Path::new(source_path);
    let output_dir = source.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建输出目录失败: {}", e))?;

    let stem = source
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "image".into());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let filename = format!(
        "{}_transparent_{}_{:04}.png",
        stem,
        timestamp,
        nonce % 10_000
    );
    let filepath = output_dir.join(filename);

    img.save(&filepath)
        .map_err(|e| format!("保存透明背景图片失败: {}", e))?;

    Ok(filepath.to_string_lossy().to_string())
}

fn estimate_background_rgb(img: &RgbaImage) -> [u8; 3] {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return [255, 255, 255];
    }

    let patch = (w.min(h) / 32).clamp(2, 18);
    let mut samples = Vec::new();
    collect_corner_samples(img, patch, &mut samples);
    collect_edge_samples(img, &mut samples);

    if samples.is_empty() {
        return [255, 255, 255];
    }

    #[derive(Default)]
    struct Bucket {
        count: u32,
        r_sum: u32,
        g_sum: u32,
        b_sum: u32,
    }

    let mut buckets: HashMap<(u8, u8, u8), Bucket> = HashMap::new();
    for [r, g, b] in samples {
        let key = (r / 16, g / 16, b / 16);
        let bucket = buckets.entry(key).or_default();
        bucket.count += 1;
        bucket.r_sum += u32::from(r);
        bucket.g_sum += u32::from(g);
        bucket.b_sum += u32::from(b);
    }

    let Some(bucket) = buckets.values().max_by_key(|bucket| bucket.count) else {
        return [255, 255, 255];
    };

    [
        (bucket.r_sum / bucket.count) as u8,
        (bucket.g_sum / bucket.count) as u8,
        (bucket.b_sum / bucket.count) as u8,
    ]
}

fn collect_corner_samples(img: &RgbaImage, patch: u32, samples: &mut Vec<[u8; 3]>) {
    let (w, h) = img.dimensions();
    let x_ranges = [(0, patch.min(w)), (w.saturating_sub(patch), w)];
    let y_ranges = [(0, patch.min(h)), (h.saturating_sub(patch), h)];

    for (x_start, x_end) in x_ranges {
        for (y_start, y_end) in y_ranges {
            for y in y_start..y_end {
                for x in x_start..x_end {
                    push_sample(img, x, y, samples);
                }
            }
        }
    }
}

fn collect_edge_samples(img: &RgbaImage, samples: &mut Vec<[u8; 3]>) {
    let (w, h) = img.dimensions();
    let step = ((w.max(h) / 256).max(1)) as usize;

    for x in (0..w).step_by(step) {
        push_sample(img, x, 0, samples);
        if h > 1 {
            push_sample(img, x, h - 1, samples);
        }
    }
    for y in (0..h).step_by(step) {
        push_sample(img, 0, y, samples);
        if w > 1 {
            push_sample(img, w - 1, y, samples);
        }
    }
}

fn push_sample(img: &RgbaImage, x: u32, y: u32, samples: &mut Vec<[u8; 3]>) {
    let pixel = img.get_pixel(x, y).0;
    if pixel[3] < 8 {
        return;
    }
    samples.push([pixel[0], pixel[1], pixel[2]]);
}

fn detect_edge_connected_background(
    img: &RgbaImage,
    background_rgb: [u8; 3],
    tolerance: u8,
) -> Vec<bool> {
    let (w, h) = img.dimensions();
    let len = (w as usize).saturating_mul(h as usize);
    let mut seen = vec![false; len];
    let mut mask = vec![false; len];
    let mut queue = VecDeque::new();

    {
        let mut flood = BackgroundFlood {
            img,
            background_rgb,
            tolerance,
            seen: &mut seen,
            mask: &mut mask,
            queue: &mut queue,
        };

        for x in 0..w {
            flood.try_enqueue(x, 0);
            if h > 1 {
                flood.try_enqueue(x, h - 1);
            }
        }
        for y in 0..h {
            flood.try_enqueue(0, y);
            if w > 1 {
                flood.try_enqueue(w - 1, y);
            }
        }

        const NEIGHBORS: [(i32, i32); 8] = [
            (-1, -1),
            (0, -1),
            (1, -1),
            (-1, 0),
            (1, 0),
            (-1, 1),
            (0, 1),
            (1, 1),
        ];

        while let Some((x, y)) = flood.queue.pop_front() {
            for (dx, dy) in NEIGHBORS {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                flood.try_enqueue(nx as u32, ny as u32);
            }
        }
    }

    mask
}

struct BackgroundFlood<'a> {
    img: &'a RgbaImage,
    background_rgb: [u8; 3],
    tolerance: u8,
    seen: &'a mut [bool],
    mask: &'a mut [bool],
    queue: &'a mut VecDeque<(u32, u32)>,
}

impl BackgroundFlood<'_> {
    fn try_enqueue(&mut self, x: u32, y: u32) {
        let index = pixel_index(self.img.width(), x, y);
        if self.seen[index] {
            return;
        }
        self.seen[index] = true;

        if pixel_matches_background(
            self.img.get_pixel(x, y).0,
            self.background_rgb,
            self.tolerance,
        ) {
            self.mask[index] = true;
            self.queue.push_back((x, y));
        }
    }
}

fn extend_mask_with_global_color_key(
    img: &RgbaImage,
    background_rgb: [u8; 3],
    tolerance: u8,
    mask: &mut [bool],
) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            let index = pixel_index(w, x, y);
            if mask[index] {
                continue;
            }
            if pixel_matches_background(img.get_pixel(x, y).0, background_rgb, tolerance) {
                mask[index] = true;
            }
        }
    }
}

fn should_apply_global_color_key(background_rgb: [u8; 3], mode: ColorKeyMode) -> bool {
    match mode {
        ColorKeyMode::EdgeOnly => return false,
        ColorKeyMode::Global => return true,
        ColorKeyMode::Auto => {}
    }

    let [r, g, b] = background_rgb;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    max >= 120 && max.saturating_sub(min) >= 56
}

fn pixel_matches_background(pixel: [u8; 4], background_rgb: [u8; 3], tolerance: u8) -> bool {
    if pixel[3] < 8 {
        return true;
    }
    rgb_distance_squared([pixel[0], pixel[1], pixel[2]], background_rgb)
        <= u32::from(tolerance).pow(2) * 3
}

fn apply_transparency_mask(
    img: &mut RgbaImage,
    mask: &[bool],
    background_rgb: [u8; 3],
    options: TransparentBackgroundOptions,
) {
    let (w, h) = img.dimensions();
    for y in 0..h {
        for x in 0..w {
            if mask[pixel_index(w, x, y)] {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 0]));
            }
        }
    }

    if options.feather_radius == 0 {
        return;
    }

    let radius = options.feather_radius.min(3) as i32;
    let tolerance = f32::from(options.tolerance);
    let feather_span = (tolerance * 1.35).max(20.0);
    let feather_max = tolerance + feather_span;
    let original = img.clone();

    for y in 0..h {
        for x in 0..w {
            let index = pixel_index(w, x, y);
            if mask[index] || !has_mask_neighbor(mask, w, h, x, y, radius) {
                continue;
            }

            let mut pixel = original.get_pixel(x, y).0;
            if pixel[3] == 0 {
                continue;
            }

            let distance = rgb_distance([pixel[0], pixel[1], pixel[2]], background_rgb);
            if distance <= tolerance || distance > feather_max {
                continue;
            }

            let alpha_ratio = ((distance - tolerance) / feather_span).clamp(0.0, 1.0);
            let softened_alpha = (alpha_ratio * f32::from(pixel[3])).round() as u8;
            pixel[3] = pixel[3].min(softened_alpha.max(24));
            img.put_pixel(x, y, image::Rgba(pixel));
        }
    }
}

fn has_mask_neighbor(mask: &[bool], w: u32, h: u32, x: u32, y: u32, radius: i32) -> bool {
    let xi = x as i32;
    let yi = y as i32;
    for ny in (yi - radius)..=(yi + radius) {
        for nx in (xi - radius)..=(xi + radius) {
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            if nx == xi && ny == yi {
                continue;
            }
            if mask[pixel_index(w, nx as u32, ny as u32)] {
                return true;
            }
        }
    }
    false
}

fn pixel_index(width: u32, x: u32, y: u32) -> usize {
    (y as usize) * (width as usize) + (x as usize)
}

fn rgb_distance_squared(a: [u8; 3], b: [u8; 3]) -> u32 {
    let dr = i32::from(a[0]) - i32::from(b[0]);
    let dg = i32::from(a[1]) - i32::from(b[1]);
    let db = i32::from(a[2]) - i32::from(b[2]);
    (dr * dr + dg * dg + db * db) as u32
}

fn rgb_distance(a: [u8; 3], b: [u8; 3]) -> f32 {
    (rgb_distance_squared(a, b) as f32).sqrt()
}

/// 从文件加载图片
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

/// 导出选中帧到指定目录，优先从临时帧路径读取，兼容 base64 帧数据。
pub fn export_frame_sources(
    frames_data: &[(u32, String, String, Option<f32>)],
    output_dir: &str,
    base_name: &str,
) -> Result<Vec<String>, String> {
    if frames_data.is_empty() {
        return Err("没有可导出的帧".into());
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let base_name = sanitize_export_name(base_name);
    let decoded = decode_export_frames(frames_data)?;
    let (canvas_w, canvas_h, anchor_canvas_x) = get_aligned_canvas_metrics(&decoded);
    let mut saved = Vec::new();
    for (i, frame) in decoded.iter().enumerate() {
        let img = draw_aligned_export_frame(frame, canvas_w, canvas_h, anchor_canvas_x);
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
    frames_data: &[(u32, String, String, Option<f32>)],
    output_dir: &str,
    base_name: &str,
    fps: u32,
) -> Result<String, String> {
    if frames_data.is_empty() {
        return Err("没有可导出的帧".into());
    }

    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建目录失败: {}", e))?;

    let decoded = decode_export_frames(frames_data)?;
    let (canvas_w, canvas_h, anchor_canvas_x) = get_aligned_canvas_metrics(&decoded);

    let frame_ms = (1000.0 / fps.clamp(1, 60) as f32).round().max(10.0) as u32;
    let delay = image::Delay::from_numer_denom_ms(frame_ms, 1);
    let gif_frames = decoded.iter().map(|frame| {
        image::Frame::from_parts(
            draw_aligned_export_frame(frame, canvas_w, canvas_h, anchor_canvas_x),
            0,
            0,
            delay,
        )
    });

    let base_name = strip_gif_extension(&sanitize_export_name(base_name));
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

fn decode_export_frames(
    frames_data: &[(u32, String, String, Option<f32>)],
) -> Result<Vec<DecodedExportFrame>, String> {
    frames_data
        .iter()
        .map(|(_index, path, b64, anchor_x)| {
            let image = if path.trim().is_empty() {
                base64_to_image(b64)?.to_rgba8()
            } else {
                load_image(path)?.to_rgba8()
            };
            let anchor_x = anchor_x
                .filter(|value| value.is_finite())
                .unwrap_or(image.width() as f32 / 2.0)
                .clamp(0.0, image.width() as f32);
            Ok(DecodedExportFrame { image, anchor_x })
        })
        .collect()
}

fn get_aligned_canvas_metrics(frames: &[DecodedExportFrame]) -> (u32, u32, u32) {
    let anchor_canvas_x = frames
        .iter()
        .map(|frame| frame.anchor_x.ceil() as u32)
        .max()
        .unwrap_or(1);
    let right_span = frames
        .iter()
        .map(|frame| {
            (frame.image.width() as f32 - frame.anchor_x)
                .ceil()
                .max(0.0) as u32
        })
        .max()
        .unwrap_or(1);
    let canvas_w = (anchor_canvas_x + right_span).max(1);
    let canvas_h = frames
        .iter()
        .map(|frame| frame.image.height())
        .max()
        .unwrap_or(1)
        .max(1);

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

fn sanitize_export_name(name: &str) -> String {
    let sanitized: String = name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();
    let sanitized = sanitized
        .trim_matches(|ch: char| ch == '.' || ch == '_' || ch == '-' || ch.is_whitespace())
        .to_string();

    if sanitized.is_empty() {
        "sprite".into()
    } else {
        sanitized
    }
}

fn strip_gif_extension(name: &str) -> String {
    name.strip_suffix(".gif")
        .or_else(|| name.strip_suffix(".GIF"))
        .unwrap_or(name)
        .to_string()
}

// ============================================================
// 单元测试
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbaImage};

    /// 创建测试用的纯色 RGBA 图片
    fn make_test_image(w: u32, h: u32) -> DynamicImage {
        let img = RgbaImage::from_pixel(w, h, image::Rgba([198, 97, 63, 255]));
        DynamicImage::ImageRgba8(img)
    }

    #[test]
    fn test_resize_image_original() {
        let img = make_test_image(4096, 4096);
        let result = resize_image(&img, "原始");
        // "原始" 不缩放
        assert_eq!(result.width(), 4096);
        assert_eq!(result.height(), 4096);
    }

    #[test]
    fn test_resize_image_1k() {
        let img = make_test_image(4096, 2048);
        let result = resize_image(&img, "1K");
        // 长边缩放到1024
        assert_eq!(result.width(), 1024);
        assert_eq!(result.height(), 512);
    }

    #[test]
    fn test_resize_image_2k_already_smaller() {
        let img = make_test_image(512, 512);
        let result = resize_image(&img, "2K");
        // 已小于目标，不缩放
        assert_eq!(result.width(), 512);
        assert_eq!(result.height(), 512);
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
                .unwrap_or_default()
        ));
        let img = make_test_image(16, 16);
        let first = save_image(&img, &dir.to_string_lossy(), "test", 1).unwrap();
        let second = save_image(&img, &dir.to_string_lossy(), "test", 1).unwrap();

        let _ = std::fs::remove_dir_all(&dir);
        assert_ne!(first, second);
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
                .unwrap_or_default()
        ));
        let img = make_test_image(8, 8);
        let b64 = image_to_base64(&img).unwrap();
        let frames = vec![
            (0, String::new(), b64.clone(), Some(4.0)),
            (1, String::new(), b64, Some(4.0)),
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
                .unwrap_or_default()
        ));
        let red =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, image::Rgba([220, 42, 42, 255])));
        let blue =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 6, image::Rgba([42, 80, 220, 255])));
        let frames = vec![
            (0, String::new(), image_to_base64(&red).unwrap(), Some(1.0)),
            (1, String::new(), image_to_base64(&blue).unwrap(), Some(6.0)),
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
                .unwrap_or_default()
        ));
        let first = make_test_image(8, 8);
        let second =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(8, 8, image::Rgba([42, 80, 220, 255])));
        let frames = vec![
            (
                0,
                String::new(),
                image_to_base64(&first).unwrap(),
                Some(4.0),
            ),
            (
                1,
                String::new(),
                image_to_base64(&second).unwrap(),
                Some(4.0),
            ),
        ];

        let saved = export_gif_sources(&frames, &dir.to_string_lossy(), "walk_cycle", 12).unwrap();
        let metadata = std::fs::metadata(dir.join("walk_cycle.gif")).unwrap();

        assert!(saved.ends_with("walk_cycle.gif"));
        assert!(metadata.len() > 0);

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
}
