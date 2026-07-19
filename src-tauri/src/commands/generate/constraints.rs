use tauri::command;

use crate::services::constraints::{self, ImageGenerationConstraints, VideoGenerationConstraints};

#[command]
pub fn build_sprite_image_prompt(
    prompt: String,
    constraints: ImageGenerationConstraints,
    rows: u32,
    cols: u32,
    has_reference: bool,
) -> Result<String, String> {
    constraints::build_sprite_image_prompt(&prompt, &constraints, rows, cols, has_reference)
}

#[command]
pub fn build_redraw_constraint_prompt(
    prompt: String,
    constraints: ImageGenerationConstraints,
) -> Result<String, String> {
    constraints::build_redraw_constraint_prompt(&prompt, &constraints)
}

#[command]
pub fn build_video_prompt(
    prompt: String,
    constraints: VideoGenerationConstraints,
    has_reference: bool,
) -> Result<String, String> {
    constraints::build_video_prompt(&prompt, &constraints, has_reference)
}
