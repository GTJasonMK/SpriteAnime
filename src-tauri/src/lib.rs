mod api_client;
mod asset_library;
mod commands;
pub mod config;
mod events;
pub mod image_processor;
mod logger;
mod workbench;

use config::AppState;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use tauri::Manager;

const APP_DATA_DIR_NAME: &str = "SpriteAnimteData";
const ASSET_LIBRARY_DIR_NAME: &str = "assets";

/// 获取应用旁的数据目录，所有运行期数据都写入这里。
fn get_app_data_dir() -> Result<PathBuf, String> {
    let app_dir = get_app_root_dir()?.join(APP_DATA_DIR_NAME);
    std::fs::create_dir_all(&app_dir)
        .map_err(|e| format!("创建应用数据目录失败: {} ({e})", app_dir.to_string_lossy()))?;
    Ok(app_dir)
}

fn get_app_root_dir() -> Result<PathBuf, String> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage_path = absolutize_path(PathBuf::from(appimage))?;
        return appimage_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "无法获取 AppImage 所在目录".to_string());
    }

    let exe = std::env::current_exe().map_err(|e| format!("无法获取应用可执行文件路径: {e}"))?;
    app_root_from_exe_path(&exe)
}

fn app_root_from_exe_path(exe: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        if let Some(bundle_parent) = macos_bundle_parent(exe) {
            return Ok(bundle_parent);
        }
    }

    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "无法获取应用所在目录".to_string())
}

fn absolutize_path(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()
            .map_err(|e| format!("无法获取当前目录: {e}"))?
            .join(path))
    }
}

#[cfg(target_os = "macos")]
fn macos_bundle_parent(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("app"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}

/// 初始化应用状态
fn init_app_state() -> Result<AppState, String> {
    let app_dir = get_app_data_dir()?;
    let config_path = app_dir.join("config.json");
    let log_dir = app_dir.join("logs");
    let workbench_records_path = app_dir.join("workbench_records.json");
    let default_save_dir = app_dir.join(ASSET_LIBRARY_DIR_NAME);

    eprintln!("[app] 应用数据目录: {}", app_dir.display());
    eprintln!("[app] 素材库目录: {}", default_save_dir.display());
    eprintln!("[app] 配置文件: {}", config_path.display());

    // 加载配置
    let mut user_config = config::UserConfig::load(&config_path);
    user_config.use_portable_save_dir(&default_save_dir);
    eprintln!(
        "[app] 配置加载完成, api_key已设置: {}",
        !user_config.api_key.is_empty()
    );

    // 从配置中恢复提示词历史
    let prompt_history: VecDeque<String> = user_config.prompt_history.iter().cloned().collect();

    // 确保默认目录和素材库分类目录存在
    std::fs::create_dir_all(&log_dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    std::fs::create_dir_all(&default_save_dir).map_err(|e| format!("创建素材库目录失败: {e}"))?;
    asset_library::ensure_standard_dirs(&default_save_dir, &user_config.save_dir)?;
    user_config.save(&config_path)?;

    Ok(AppState {
        config: Mutex::new(user_config),
        prompt_history: Mutex::new(prompt_history),
        app_data_dir: app_dir,
        config_path,
        log_dir,
        workbench_records_path,
        default_save_dir,
    })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    eprintln!("[app] SpriteAnimte 启动...");
    let app_state = init_app_state().expect("初始化应用状态失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(app_state)
        .setup(|app| {
            let data_dir = app.state::<AppState>().app_data_dir.clone();
            app.state::<tauri::scope::Scopes>()
                .allow_directory(&data_dir, true)?;
            eprintln!("[app] 已授权本地素材预览目录: {}", data_dir.display());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::generate::get_presets,
            commands::generate::load_config,
            commands::generate::save_config,
            commands::generate::export_config,
            commands::generate::import_config,
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
            commands::generate::generate_video,
            commands::sprite::extract_sprite_frames,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_root_from_exe_path_uses_executable_parent() {
        let root = Path::new("/tmp/sprite-animte-portable");
        let exe = root.join("SpriteAnimte");
        assert_eq!(app_root_from_exe_path(&exe).unwrap(), root);
    }
}
