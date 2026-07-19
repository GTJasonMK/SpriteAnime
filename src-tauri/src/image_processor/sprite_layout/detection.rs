use std::collections::VecDeque;

use super::{SpriteFrameBounds, SpriteRegion};

pub(super) fn minimum_component_pixels(cells: &[SpriteRegion]) -> Result<usize, String> {
    let first = cells
        .first()
        .ok_or_else(|| "网格单元不能为空".to_string())?;
    let first_area = u64::from(first.width) * u64::from(first.height);
    let area = cells.iter().skip(1).fold(first_area, |area, cell| {
        area.min(u64::from(cell.width) * u64::from(cell.height))
    });
    Ok(((area as f64 * 0.00006).round() as usize).clamp(5, 24))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn detect_owned_bounds(
    foreground: &[bool],
    image_width: u32,
    image_height: u32,
    cells: &[SpriteRegion],
    frame_index: usize,
    expand: u32,
    min_component: usize,
) -> Option<SpriteRegion> {
    let cell = cells[frame_index];
    let left = (cell.x - expand as i32).max(0);
    let top = (cell.y - expand as i32).max(0);
    let right = (cell.x + cell.width as i32 + expand as i32).min(image_width as i32);
    let bottom = (cell.y + cell.height as i32 + expand as i32).min(image_height as i32);
    let mut visited = vec![false; ((right - left) * (bottom - top)) as usize];
    let window_width = (right - left) as usize;
    let mut union = None;
    for y in top..bottom {
        for x in left..right {
            let local = (y - top) as usize * window_width + (x - left) as usize;
            if visited[local]
                || !foreground[y as usize * image_width as usize + x as usize]
                || !can_start_component(x, y, cells, frame_index)
            {
                continue;
            }
            let component = collect_component(
                foreground,
                image_width,
                cells,
                frame_index,
                left,
                top,
                right,
                bottom,
                x,
                y,
                &mut visited,
            );
            if component.1 >= min_component && component.2 {
                union = Some(union_region(union, component.0));
            }
        }
    }
    union
}

#[allow(clippy::too_many_arguments)]
fn collect_component(
    foreground: &[bool],
    image_width: u32,
    cells: &[SpriteRegion],
    frame_index: usize,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    start_x: i32,
    start_y: i32,
    visited: &mut [bool],
) -> (SpriteRegion, usize, bool) {
    let window_width = (right - left) as usize;
    let mut queue = VecDeque::from([(start_x, start_y)]);
    visited[(start_y - top) as usize * window_width + (start_x - left) as usize] = true;
    let mut min_x = start_x;
    let mut min_y = start_y;
    let mut max_x = start_x;
    let mut max_y = start_y;
    let mut count = 0;
    let mut owned = 0;
    let mut owner_counts = vec![0usize; cells.len()];
    while let Some((x, y)) = queue.pop_front() {
        count += 1;
        if contains(cells[frame_index], x, y) {
            owned += 1;
        }
        if let Some(owner) = owner_index(x, y, cells) {
            owner_counts[owner] += 1;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        enqueue_neighbors(
            foreground,
            image_width,
            left,
            top,
            right,
            bottom,
            x,
            y,
            window_width,
            visited,
            &mut queue,
        );
    }
    let dominant = owner_counts
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| **count)
        .map(|(index, _)| index);
    (
        SpriteRegion {
            x: min_x,
            y: min_y,
            width: (max_x - min_x + 1) as u32,
            height: (max_y - min_y + 1) as u32,
        },
        count,
        owned > 0 && dominant == Some(frame_index),
    )
}

#[allow(clippy::too_many_arguments)]
fn enqueue_neighbors(
    foreground: &[bool],
    image_width: u32,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
    x: i32,
    y: i32,
    window_width: usize,
    visited: &mut [bool],
    queue: &mut VecDeque<(i32, i32)>,
) {
    for dy in -1..=1 {
        for dx in -1..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let nx = x + dx;
            let ny = y + dy;
            if nx < left || ny < top || nx >= right || ny >= bottom {
                continue;
            }
            let local = (ny - top) as usize * window_width + (nx - left) as usize;
            if !visited[local] && foreground[ny as usize * image_width as usize + nx as usize] {
                visited[local] = true;
                queue.push_back((nx, ny));
            }
        }
    }
}

fn owner_index(x: i32, y: i32, cells: &[SpriteRegion]) -> Option<usize> {
    cells.iter().position(|cell| contains(*cell, x, y))
}

fn can_start_component(x: i32, y: i32, cells: &[SpriteRegion], frame_index: usize) -> bool {
    owner_index(x, y, cells).is_none_or(|owner| owner == frame_index)
}

fn contains(region: SpriteRegion, x: i32, y: i32) -> bool {
    x >= region.x
        && y >= region.y
        && x < region.x + region.width as i32
        && y < region.y + region.height as i32
}

fn union_region(current: Option<SpriteRegion>, next: SpriteRegion) -> SpriteRegion {
    let Some(current) = current else {
        return next;
    };
    let left = current.x.min(next.x);
    let top = current.y.min(next.y);
    let right = (current.x + current.width as i32).max(next.x + next.width as i32);
    let bottom = (current.y + current.height as i32).max(next.y + next.height as i32);
    SpriteRegion {
        x: left,
        y: top,
        width: (right - left) as u32,
        height: (bottom - top) as u32,
    }
}

pub(super) fn fixed_bounds(
    frames: &[SpriteFrameBounds],
    cell_width: u32,
    cell_height: u32,
    expand: u32,
) -> (i32, i32, u32, u32, usize) {
    let non_empty = frames
        .iter()
        .filter(|frame| !frame.empty)
        .collect::<Vec<_>>();
    let empty_count = frames.len() - non_empty.len();
    if non_empty.is_empty() {
        return (0, 0, cell_width, cell_height, empty_count);
    }
    let first = non_empty[0];
    let initial = (
        first.x - first.cell_x,
        first.y - first.cell_y,
        first.x - first.cell_x + first.width as i32,
        first.y - first.cell_y + first.height as i32,
    );
    let (min_x, min_y, max_right, max_bottom) =
        non_empty.iter().skip(1).fold(initial, |bounds, frame| {
            (
                bounds.0.min(frame.x - frame.cell_x),
                bounds.1.min(frame.y - frame.cell_y),
                bounds.2.max(frame.x - frame.cell_x + frame.width as i32),
                bounds.3.max(frame.y - frame.cell_y + frame.height as i32),
            )
        });
    let min_x = min_x - 2;
    let min_y = min_y - 2;
    let max_right = max_right + 2;
    let max_bottom = max_bottom + 2;
    let min_x = min_x.clamp(-(expand as i32), cell_width as i32 - 1);
    let min_y = min_y.clamp(-(expand as i32), cell_height as i32 - 1);
    let right = max_right.clamp(min_x + 1, cell_width as i32 + expand as i32);
    let bottom = max_bottom.clamp(min_y + 1, cell_height as i32 + expand as i32);
    (
        min_x,
        min_y,
        (right - min_x) as u32,
        (bottom - min_y) as u32,
        empty_count,
    )
}
