use image::{DynamicImage, RgbaImage};
use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::Ordering;

use super::SAVE_COUNTER;

#[derive(Debug, Clone, Copy)]
pub struct TransparentBackgroundOptions {
    pub tolerance: u8,
    pub feather_radius: u8,
    pub color_key_mode: ColorKeyMode,
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

/// 将纯色或高对比背景转为透明。
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

pub fn save_transparent_copy_to_dir(
    img: &DynamicImage,
    source: &Path,
    output_dir: &Path,
) -> Result<String, String> {
    let stem = required_transparent_source_stem(source)?;
    std::fs::create_dir_all(output_dir).map_err(|e| format!("创建输出目录失败: {}", e))?;

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

fn required_transparent_source_stem(source: &Path) -> Result<String, String> {
    source
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "透明背景保存缺少源文件名：{}。解决方法：请先将图片保存为带文件名的本地文件，再重新抠图保存。",
                source.display()
            )
        })
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
