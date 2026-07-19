use image::RgbaImage;
use serde::{Deserialize, Serialize};

mod detection;
use detection::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteRegion {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteFrameBounds {
    pub index: usize,
    pub cell_x: i32,
    pub cell_y: i32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub anchor_x: f32,
    pub empty: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteLayoutV1 {
    pub schema_version: u32,
    pub rows: u32,
    pub cols: u32,
    pub image_width: u32,
    pub image_height: u32,
    pub region: SpriteRegion,
    pub grid_signature: String,
    pub allow_expand: bool,
    pub expand_pixels: u32,
    pub frame_bounds: Vec<SpriteFrameBounds>,
    pub fixed_offset_x: i32,
    pub fixed_offset_y: i32,
    pub fixed_width: u32,
    pub fixed_height: u32,
    pub empty_count: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum SpriteBackgroundMode {
    Auto,
    White,
}

pub fn detect_sprite_layout(
    image: &RgbaImage,
    rows: u32,
    cols: u32,
    region: Option<SpriteRegion>,
    background_mode: SpriteBackgroundMode,
    threshold: u8,
    allow_expand: bool,
) -> Result<SpriteLayoutV1, String> {
    if rows == 0 || cols == 0 || rows > 20 || cols > 20 {
        return Err("精灵图行列数必须为 1 到 20".into());
    }
    let region = normalize_region(image, region)?;
    let cells = grid_cells(region, rows, cols)?;
    detect_sprite_layout_with_cells(
        image,
        rows,
        cols,
        region,
        cells,
        format!(
            "{rows}x{cols}:{}:{}:{}:{}",
            region.x, region.y, region.width, region.height
        ),
        background_mode,
        threshold,
        allow_expand,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn detect_sprite_layout_with_cells(
    image: &RgbaImage,
    rows: u32,
    cols: u32,
    region: SpriteRegion,
    cells: Vec<SpriteRegion>,
    grid_signature: String,
    background_mode: SpriteBackgroundMode,
    threshold: u8,
    allow_expand: bool,
) -> Result<SpriteLayoutV1, String> {
    normalize_region(image, Some(region))?;
    validate_cells(image, rows, cols, &cells)?;
    let first_cell = cells
        .first()
        .ok_or_else(|| "网格单元不能为空".to_string())?;
    let (min_width, min_height) = cells.iter().skip(1).fold(
        (first_cell.width, first_cell.height),
        |(min_width, min_height), cell| (min_width.min(cell.width), min_height.min(cell.height)),
    );
    if min_width == 0 || min_height == 0 {
        return Err("网格太密，单帧尺寸无效".into());
    }
    let expand_pixels = if allow_expand {
        ((min_width.min(min_height) as f32 * 0.18).round() as u32).clamp(4, 96)
    } else {
        0
    };
    let background = match background_mode {
        SpriteBackgroundMode::White => [255, 255, 255, 255],
        SpriteBackgroundMode::Auto => estimate_background(image, region),
    };
    let foreground = foreground_mask(image, background, threshold);
    let min_component = minimum_component_pixels(&cells)?;
    let mut frame_bounds = Vec::with_capacity(cells.len());

    for (index, cell) in cells.iter().copied().enumerate() {
        let detected = detect_owned_bounds(
            &foreground,
            image.width(),
            image.height(),
            &cells,
            index,
            expand_pixels,
            min_component,
        );
        frame_bounds.push(match detected {
            Some(bounds) => SpriteFrameBounds {
                index,
                cell_x: cell.x,
                cell_y: cell.y,
                cell_width: cell.width,
                cell_height: cell.height,
                x: bounds.x,
                y: bounds.y,
                width: bounds.width,
                height: bounds.height,
                anchor_x: bounds.x as f32 + bounds.width as f32 / 2.0,
                empty: false,
            },
            None => SpriteFrameBounds {
                index,
                cell_x: cell.x,
                cell_y: cell.y,
                cell_width: cell.width,
                cell_height: cell.height,
                x: cell.x,
                y: cell.y,
                width: cell.width,
                height: cell.height,
                anchor_x: cell.x as f32 + cell.width as f32 / 2.0,
                empty: true,
            },
        });
    }
    let (fixed_offset_x, fixed_offset_y, fixed_width, fixed_height, empty_count) =
        fixed_bounds(&frame_bounds, min_width, min_height, expand_pixels);
    Ok(SpriteLayoutV1 {
        schema_version: 1,
        rows,
        cols,
        image_width: image.width(),
        image_height: image.height(),
        region,
        grid_signature,
        allow_expand,
        expand_pixels,
        frame_bounds,
        fixed_offset_x,
        fixed_offset_y,
        fixed_width,
        fixed_height,
        empty_count,
    })
}

fn validate_cells(
    image: &RgbaImage,
    rows: u32,
    cols: u32,
    cells: &[SpriteRegion],
) -> Result<(), String> {
    if rows == 0 || cols == 0 || rows > 20 || cols > 20 {
        return Err("精灵图行列数必须为 1 到 20".into());
    }
    if cells.len() != (rows * cols) as usize {
        return Err("网格单元数量与行列设置不一致".into());
    }
    for cell in cells {
        normalize_region(image, Some(*cell))?;
    }
    Ok(())
}

fn normalize_region(
    image: &RgbaImage,
    region: Option<SpriteRegion>,
) -> Result<SpriteRegion, String> {
    let region = region.unwrap_or(SpriteRegion {
        x: 0,
        y: 0,
        width: image.width(),
        height: image.height(),
    });
    if region.x < 0
        || region.y < 0
        || region.width == 0
        || region.height == 0
        || region.x as u64 + u64::from(region.width) > u64::from(image.width())
        || region.y as u64 + u64::from(region.height) > u64::from(image.height())
    {
        return Err("精灵图检测区域超出图片范围".into());
    }
    Ok(region)
}

fn grid_cells(region: SpriteRegion, rows: u32, cols: u32) -> Result<Vec<SpriteRegion>, String> {
    let mut cells = Vec::with_capacity((rows * cols) as usize);
    for row in 0..rows {
        for col in 0..cols {
            let left =
                region.x + (u64::from(region.width) * u64::from(col) / u64::from(cols)) as i32;
            let right =
                region.x + (u64::from(region.width) * u64::from(col + 1) / u64::from(cols)) as i32;
            let top =
                region.y + (u64::from(region.height) * u64::from(row) / u64::from(rows)) as i32;
            let bottom =
                region.y + (u64::from(region.height) * u64::from(row + 1) / u64::from(rows)) as i32;
            if right <= left || bottom <= top {
                return Err("网格太密，单帧尺寸无效".into());
            }
            cells.push(SpriteRegion {
                x: left,
                y: top,
                width: (right - left) as u32,
                height: (bottom - top) as u32,
            });
        }
    }
    Ok(cells)
}

fn estimate_background(image: &RgbaImage, region: SpriteRegion) -> [u8; 4] {
    let patch = 5u32.min(region.width).min(region.height);
    let starts = [
        (region.x as u32, region.y as u32),
        (region.x as u32 + region.width - patch, region.y as u32),
        (region.x as u32, region.y as u32 + region.height - patch),
        (
            region.x as u32 + region.width - patch,
            region.y as u32 + region.height - patch,
        ),
    ];
    let mut sums = [0u64; 4];
    let mut count = 0u64;
    for (start_x, start_y) in starts {
        for y in start_y..start_y + patch {
            for x in start_x..start_x + patch {
                for (sum, channel) in sums.iter_mut().zip(image.get_pixel(x, y).0) {
                    *sum += u64::from(channel);
                }
                count += 1;
            }
        }
    }
    sums.map(|sum| (sum / count.max(1)) as u8)
}

fn foreground_mask(image: &RgbaImage, background: [u8; 4], threshold: u8) -> Vec<bool> {
    image
        .pixels()
        .map(|pixel| {
            let [r, g, b, a] = pixel.0;
            if a <= 16 {
                return false;
            }
            if background[3] <= 16 {
                return true;
            }
            let distance = ((i32::from(r) - i32::from(background[0])).pow(2)
                + (i32::from(g) - i32::from(background[1])).pow(2)
                + (i32::from(b) - i32::from(background[2])).pow(2))
                as f64;
            distance.sqrt() > f64::from(threshold)
                || i32::from(a).abs_diff(i32::from(background[3])) > u32::from(threshold)
        })
        .collect()
}

#[cfg(test)]
mod tests;
