mod api_client;
mod commands;
pub mod config;
mod events;
pub mod image_processor;
mod logger;
mod workbench;

use config::AppState;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::PathBuf;

/// 获取用户数据目录（~/.local/share/sprite-animte/），避免Tauri监视源目录触发重载
fn get_app_dir() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".into());
    let base = PathBuf::from(home)
        .join(".local")
        .join("share")
        .join("sprite-animte");
    std::fs::create_dir_all(&base).ok();
    base
}

/// 初始化应用状态
fn init_app_state() -> AppState {
    let app_dir = get_app_dir();
    let config_path = app_dir.join("config.json");
    let log_dir = app_dir.join("logs");
    let workbench_records_path = app_dir.join("workbench_records.json");
    let default_save_dir = get_default_pictures_dir().unwrap_or_else(|| app_dir.join("output"));

    eprintln!("[app] 应用数据目录: {}", app_dir.display());
    eprintln!("[app] 配置文件: {}", config_path.display());

    // 加载配置
    let user_config = config::UserConfig::load(&config_path);
    eprintln!(
        "[app] 配置加载完成, api_key已设置: {}",
        !user_config.api_key.is_empty()
    );

    // 从配置中恢复提示词历史
    let prompt_history: VecDeque<String> = user_config.prompt_history.iter().cloned().collect();

    // 确保默认目录存在
    let _ = std::fs::create_dir_all(&log_dir);
    let _ = std::fs::create_dir_all(&default_save_dir);

    AppState {
        config: Mutex::new(user_config),
        prompt_history: Mutex::new(prompt_history),
        config_path,
        log_dir,
        workbench_records_path,
        default_save_dir,
    }
}

/// 获取默认图片保存目录（优先Pictures，回退到项目output目录）
fn get_default_pictures_dir() -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()?;
    let pictures = PathBuf::from(home).join("Pictures").join("SpriteAnimte");
    let _ = std::fs::create_dir_all(&pictures);
    Some(pictures)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[app] SpriteAnimte 启动...");
    let app_state = init_app_state();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            commands::generate::get_presets,
            commands::generate::load_config,
            commands::generate::save_config,
            commands::generate::check_generation_api,
            commands::generate::check_prompt_optimizer_api,
            commands::generate::get_prompt_history,
            commands::generate::add_prompt_history,
            commands::generate::read_workbench_records,
            commands::generate::upsert_workbench_records,
            commands::generate::delete_workbench_record,
            commands::generate::clear_workbench_records,
            commands::generate::apply_canvas_background_transparent,
            commands::generate::save_matted_image_data_url,
            commands::generate::read_image_as_base64,
            commands::generate::read_file_as_base64,
            commands::generate::optimize_prompt,
            commands::generate::generate_image,
            commands::sprite::extract_sprite_frames,
            commands::sprite::export_frames,
            commands::sprite::export_gif,
            commands::sprite::save_sprite_sheet_data_url,
            commands::sprite::probe_video_file,
            commands::sprite::extract_video_frames_with_ffmpeg,
            commands::sprite::log_video_sprite_message,
            commands::filesystem::select_directory,
            commands::filesystem::open_image_file,
            commands::filesystem::open_video_file,
            commands::filesystem::prepare_video_file_for_playback,
            commands::filesystem::cleanup_prepared_video_file,
            commands::filesystem::cleanup_video_frame_batch_dir,
            commands::filesystem::cleanup_video_sprite_temp_files,
            commands::filesystem::reveal_in_explorer,
            commands::filesystem::open_image_file_path,
        ])
        .run(tauri::generate_context!())
        .expect("启动应用失败");
}
