use serde::{Deserialize, Serialize};
use tauri::command;

/// 文件打开结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOpenResult {
    pub file_path: String,
    pub file_name: String,
    pub base64_data: String,
}

/// 使用系统对话框选择目录并返回路径
#[command]
pub async fn select_directory(app: tauri::AppHandle) -> Result<String, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app.dialog().file().blocking_pick_folder();

    match result {
        Some(path) => Ok(path.to_string()),
        None => Err("用户取消选择".into()),
    }
}

/// 使用系统对话框选择图片文件并返回路径
#[command]
pub async fn open_image_file(app: tauri::AppHandle) -> Result<FileOpenResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = app
        .dialog()
        .file()
        .add_filter("图片文件", &["png", "jpg", "jpeg", "webp", "gif", "bmp"])
        .blocking_pick_file();

    match result {
        Some(file_path) => {
            let path_str = file_path.to_string();
            let file_name = std::path::Path::new(&path_str)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            Ok(FileOpenResult {
                file_path: path_str,
                file_name,
                base64_data: String::new(),
            })
        }
        None => Err("用户取消选择".into()),
    }
}

/// 在系统文件管理器中打开目录
#[command]
pub fn reveal_in_explorer(path: String) -> Result<(), String> {
    opener::open(path).map_err(|e| format!("打开目录失败: {}", e))
}

/// 用系统默认应用打开图片文件
#[command]
pub fn open_image_file_path(path: String) -> Result<(), String> {
    opener::open(path).map_err(|e| format!("打开文件失败: {}", e))
}
