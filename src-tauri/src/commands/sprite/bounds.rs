use tauri::{command, State};

use crate::config::AppState;
use crate::image_processor::{self, SpriteBackgroundMode, SpriteLayoutV1, SpriteRegion};

#[command]
#[allow(clippy::too_many_arguments)]
pub fn detect_sprite_layout(
    state: State<'_, AppState>,
    image_path: String,
    rows: u32,
    cols: u32,
    region: SpriteRegion,
    cell_rects: Vec<SpriteRegion>,
    grid_signature: String,
    background_mode: String,
    threshold: u8,
    allow_expand: bool,
) -> Result<SpriteLayoutV1, String> {
    let _lock =
        crate::runtime::DataLock::shared(&state.locks_dir, crate::runtime::LockDomain::Assets)
            .map_err(|error| error.to_string())?;
    let image = image_processor::load_image(&image_path)?.to_rgba8();
    let background_mode = match background_mode.as_str() {
        "auto" => SpriteBackgroundMode::Auto,
        "white" => SpriteBackgroundMode::White,
        _ => return Err(format!("自动边界背景模式无效：{background_mode}")),
    };
    image_processor::detect_sprite_layout_with_cells(
        &image,
        rows,
        cols,
        region,
        cell_rects,
        grid_signature,
        background_mode,
        threshold,
        allow_expand,
    )
}
