use std::collections::VecDeque;

use image::{DynamicImage, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EraseOperationsV1 {
    pub schema_version: u32,
    pub operations: Vec<EraseOperation>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EraseOperation {
    pub x: u32,
    pub y: u32,
    pub tolerance: u8,
    pub radius: u8,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EraseReason {
    Erased,
    Outside,
    NoSeed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EraseOperationResult {
    pub erased_pixels: u32,
    pub reason: EraseReason,
}

#[derive(Debug, Clone)]
pub struct EraseImageResult {
    pub image: DynamicImage,
    pub erased_pixels: u32,
    pub operations: Vec<EraseOperationResult>,
}

pub fn apply_erase_operations(
    image: &DynamicImage,
    request: &EraseOperationsV1,
) -> Result<EraseImageResult, String> {
    if request.schema_version != 1 {
        return Err(format!(
            "擦除操作 schemaVersion 必须为 1，实际为 {}",
            request.schema_version
        ));
    }
    if request.operations.is_empty() {
        return Err("擦除操作列表为空".into());
    }
    let mut rgba = image.to_rgba8();
    let mut total = 0u32;
    let mut results = Vec::with_capacity(request.operations.len());
    for (index, operation) in request.operations.iter().enumerate() {
        if operation.tolerance == 0 || operation.tolerance > 120 {
            return Err(format!("第{}个擦除操作容差必须为 1 到 120", index + 1));
        }
        if operation.radius > 8 {
            return Err(format!("第{}个擦除操作半径不能超过 8", index + 1));
        }
        let result = erase_connected_region(&mut rgba, operation);
        total += result.erased_pixels;
        results.push(result);
    }
    Ok(EraseImageResult {
        image: DynamicImage::ImageRgba8(rgba),
        erased_pixels: total,
        operations: results,
    })
}

pub fn erase_connected_region(
    image: &mut RgbaImage,
    operation: &EraseOperation,
) -> EraseOperationResult {
    let (width, height) = image.dimensions();
    if operation.x >= width || operation.y >= height {
        return empty_result(EraseReason::Outside);
    }
    let search_radius = u32::from(operation.radius).saturating_add(1).max(2);
    let Some((seed_x, seed_y)) = find_seed(image, operation.x, operation.y, search_radius) else {
        return empty_result(EraseReason::NoSeed);
    };
    let target = image.get_pixel(seed_x, seed_y).0;
    let mut visited = vec![false; width as usize * height as usize];
    let mut queue = VecDeque::new();
    let seed_index = pixel_index(width, seed_x, seed_y);
    visited[seed_index] = true;
    queue.push_back((seed_x, seed_y));
    let mut matched = Vec::new();

    while let Some((x, y)) = queue.pop_front() {
        let pixel = image.get_pixel(x, y).0;
        if !matches_target(pixel, target, operation.tolerance) {
            continue;
        }
        matched.push((x, y));
        for offset_y in -1i64..=1 {
            for offset_x in -1i64..=1 {
                if offset_x == 0 && offset_y == 0 {
                    continue;
                }
                let next_x = i64::from(x) + offset_x;
                let next_y = i64::from(y) + offset_y;
                if next_x < 0
                    || next_y < 0
                    || next_x >= i64::from(width)
                    || next_y >= i64::from(height)
                {
                    continue;
                }
                let next_x = next_x as u32;
                let next_y = next_y as u32;
                let index = pixel_index(width, next_x, next_y);
                if !visited[index] {
                    visited[index] = true;
                    queue.push_back((next_x, next_y));
                }
            }
        }
    }

    let mut erased = 0;
    for (x, y) in matched {
        erased += u32::from(erase_alpha(image, x, y));
    }
    erased += erase_disk(image, seed_x, seed_y, u32::from(operation.radius));
    EraseOperationResult {
        erased_pixels: erased,
        reason: EraseReason::Erased,
    }
}

fn find_seed(image: &RgbaImage, x: u32, y: u32, radius: u32) -> Option<(u32, u32)> {
    if image.get_pixel(x, y).0[3] > 0 {
        return Some((x, y));
    }
    let (width, height) = image.dimensions();
    let mut best: Option<(u32, u32, i64)> = None;
    let radius = i64::from(radius);
    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            let distance = offset_x * offset_x + offset_y * offset_y;
            if distance > radius * radius {
                continue;
            }
            let next_x = i64::from(x) + offset_x;
            let next_y = i64::from(y) + offset_y;
            if next_x < 0 || next_y < 0 || next_x >= i64::from(width) || next_y >= i64::from(height)
            {
                continue;
            }
            let next_x = next_x as u32;
            let next_y = next_y as u32;
            if image.get_pixel(next_x, next_y).0[3] == 0 {
                continue;
            }
            if best.as_ref().is_none_or(|item| distance < item.2) {
                best = Some((next_x, next_y, distance));
            }
        }
    }
    best.map(|(x, y, _)| (x, y))
}

fn matches_target(pixel: [u8; 4], target: [u8; 4], tolerance: u8) -> bool {
    if pixel[3] == 0 {
        return false;
    }
    let distance = (i32::from(pixel[0]) - i32::from(target[0])).pow(2)
        + (i32::from(pixel[1]) - i32::from(target[1])).pow(2)
        + (i32::from(pixel[2]) - i32::from(target[2])).pow(2);
    distance <= i32::from(tolerance).pow(2) * 3
}

fn erase_disk(image: &mut RgbaImage, x: u32, y: u32, radius: u32) -> u32 {
    let (width, height) = image.dimensions();
    let radius = i64::from(radius);
    let mut erased = 0;
    for offset_y in -radius..=radius {
        for offset_x in -radius..=radius {
            if offset_x * offset_x + offset_y * offset_y > radius * radius {
                continue;
            }
            let next_x = i64::from(x) + offset_x;
            let next_y = i64::from(y) + offset_y;
            if next_x >= 0 && next_y >= 0 && next_x < i64::from(width) && next_y < i64::from(height)
            {
                erased += u32::from(erase_alpha(image, next_x as u32, next_y as u32));
            }
        }
    }
    erased
}

fn erase_alpha(image: &mut RgbaImage, x: u32, y: u32) -> bool {
    let pixel = image.get_pixel_mut(x, y);
    if pixel.0[3] == 0 {
        false
    } else {
        pixel.0[3] = 0;
        true
    }
}

fn pixel_index(width: u32, x: u32, y: u32) -> usize {
    (y as usize * width as usize) + x as usize
}

fn empty_result(reason: EraseReason) -> EraseOperationResult {
    EraseOperationResult {
        erased_pixels: 0,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn erases_connected_matching_region() {
        let mut image = RgbaImage::from_pixel(3, 3, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(2, 2, image::Rgba([0, 0, 0, 255]));
        let result = erase_connected_region(
            &mut image,
            &EraseOperation {
                x: 0,
                y: 0,
                tolerance: 1,
                radius: 0,
            },
        );
        assert_eq!(result.erased_pixels, 8);
        assert_eq!(image.get_pixel(2, 2).0[3], 255);
    }

    #[test]
    fn transparent_click_finds_nearby_seed() {
        let mut image = RgbaImage::from_pixel(4, 3, image::Rgba([0, 0, 0, 0]));
        image.put_pixel(2, 1, image::Rgba([255, 255, 255, 255]));
        image.put_pixel(3, 1, image::Rgba([252, 252, 252, 255]));
        let result = erase_connected_region(
            &mut image,
            &EraseOperation {
                x: 1,
                y: 1,
                tolerance: 20,
                radius: 1,
            },
        );
        assert_eq!(result.erased_pixels, 2);
        assert_eq!(image.get_pixel(3, 1).0[3], 0);
    }

    #[test]
    fn transparent_image_reports_no_seed() {
        let mut image = RgbaImage::from_pixel(3, 3, image::Rgba([0, 0, 0, 0]));
        let result = erase_connected_region(
            &mut image,
            &EraseOperation {
                x: 1,
                y: 1,
                tolerance: 20,
                radius: 1,
            },
        );
        assert_eq!(result.reason, EraseReason::NoSeed);
        assert_eq!(result.erased_pixels, 0);
    }
}
