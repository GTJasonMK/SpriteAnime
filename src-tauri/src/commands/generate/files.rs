use tauri::command;

use crate::image_processor;

/// 读取图片并返回 PNG base64，供前端抠图画布编辑。
#[command]
pub async fn read_image_as_base64(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = image_processor::load_image(&path)?;
        image_processor::image_to_base64(&img)
    })
    .await
    .map_err(|e| format!("读取图片任务执行失败: {e}"))?
}

/// 直接读取文件字节并返回 base64，用于已经是 PNG 的临时帧，避免图片解码再编码。
#[command]
pub async fn read_file_as_base64(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    })
    .await
    .map_err(|e| format!("读取文件任务执行失败: {e}"))?
}
