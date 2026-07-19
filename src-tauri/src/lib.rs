mod api_client;
mod asset_library;
mod commands;
mod config;
mod events;
mod image_processor;
mod logger;
mod path_safety;
mod runtime;
mod services;
mod workbench;
mod workspace;

pub mod cli;

use config::AppState;
use tauri::Manager;

/// 初始化应用状态
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app_state = runtime::create_app_state(None).expect("初始化应用状态失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            let data_dir = app.state::<AppState>().app_data_dir.clone();
            app.state::<tauri::scope::Scopes>()
                .allow_directory(&data_dir, true)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generate::get_presets,
            commands::generate::build_sprite_image_prompt,
            commands::generate::build_redraw_constraint_prompt,
            commands::generate::build_video_prompt,
            commands::generate::load_config,
            commands::generate::save_config,
            commands::generate::export_config,
            commands::generate::import_config,
            commands::generate::check_generation_api,
            commands::generate::check_prompt_optimizer_api,
            commands::generate::add_prompt_history,
            commands::generate::read_workbench_records,
            commands::generate::upsert_workbench_records,
            commands::generate::delete_workbench_record,
            commands::generate::clear_workbench_records,
            commands::generate::apply_canvas_background_transparent,
            commands::generate::apply_canvas_connected_erase,
            commands::generate::save_matted_image_data_url,
            commands::generate::read_image_as_base64,
            commands::generate::read_file_as_base64,
            commands::generate::optimize_prompt,
            commands::generate::generate_image,
            commands::generate::generate_video,
            commands::redraw::create_video_sprite_redraw_run,
            commands::redraw::save_video_sprite_redraw_batch_input,
            commands::redraw::load_active_video_sprite_redraw_run,
            commands::redraw::begin_video_sprite_redraw_batch,
            commands::redraw::complete_video_sprite_redraw_batch,
            commands::redraw::fail_video_sprite_redraw_batch,
            commands::redraw::pause_video_sprite_redraw_run,
            commands::redraw::update_video_sprite_redraw_final_cols,
            commands::redraw::finalize_video_sprite_redraw_run,
            commands::redraw::discard_video_sprite_redraw_run,
            commands::sprite::extract_sprite_frames,
            commands::sprite::detect_sprite_layout,
            commands::sprite::export_frames,
            commands::sprite::export_gif,
            commands::sprite::save_sprite_sheet_data_url,
            commands::sprite::probe_video_file,
            commands::sprite::extract_video_frames_with_ffmpeg,
            commands::sprite::log_video_sprite_message,
            commands::tools::check_ffmpeg_tools,
            commands::tools::download_ffmpeg_tools,
            commands::filesystem::open_image_file,
            commands::filesystem::open_video_file,
            commands::filesystem::import_image_to_library,
            commands::filesystem::import_video_to_library,
            commands::filesystem::cleanup_video_frame_batch_dir,
            commands::filesystem::cleanup_video_sprite_temp_files,
            commands::filesystem::reveal_in_explorer,
            commands::filesystem::open_image_file_path,
            workspace::read_workspace_snapshot,
            workspace::save_workspace_snapshot,
            workspace::save_workspace_image_data_url,
            workspace::reset_workspace,
            workspace::reveal_workspace_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}

pub async fn run_cli() -> i32 {
    cli::run().await
}
