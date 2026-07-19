use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::config::AppState;
use crate::logger::{append_text_log, summarize_log_text};
use crate::path_safety::sanitize_file_name_component;

static TEMP_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
static TEMP_VIDEO_FRAME_COUNTER: AtomicU64 = AtomicU64::new(0);
const VIDEO_SPRITE_LOG_FILE: &str = "video-sprite.log";

pub(super) fn build_video_batch_extract_error(err: &str, ffmpeg_command: &str) -> String {
    format!(
        "ffmpeg 批量抽帧失败: {err}。解决方法：请在设置中检测 FFmpeg/FFprobe，确认 `{}` 可运行并支持 fps、crop、scale、format 滤镜；如果使用自定义路径，请改为完整可执行文件路径；然后重新选择视频并再次抽帧。详细日志见 logs/{}。",
        summarize_log_text(ffmpeg_command),
        VIDEO_SPRITE_LOG_FILE
    )
}

pub(super) fn append_video_sprite_log(state: &AppState, message: &str) -> Result<(), String> {
    append_video_sprite_log_to_dir(&state.log_dir, message)
}

pub(super) fn append_video_sprite_log_to_dir(log_dir: &Path, message: &str) -> Result<(), String> {
    append_text_log(log_dir, VIDEO_SPRITE_LOG_FILE, message)
}

pub(super) fn required_export_asset_name(name: &str, context: &str) -> Result<String, String> {
    let stem = Path::new(name)
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .ok_or_else(|| format!("{context}缺少有效文件名"))?;
    let sanitized = sanitize_file_name_component(&stem);

    if sanitized.is_empty() {
        Err(format!(
            "{context}清洗后为空。解决方法：请输入包含有效字符的导出名称，不要只使用空格、点号、横线、下划线或非法路径字符。"
        ))
    } else {
        Ok(sanitized)
    }
}

pub(super) fn unique_timestamped_name(base_name: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f");
    format!("{base_name}_{timestamp}")
}

pub(super) fn create_temp_frame_dir(state: &AppState) -> Result<PathBuf, String> {
    let root = state.app_data_dir.join("temp_frames");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时帧目录失败: {}", e))?;
    cleanup_old_temp_dirs(&root, 23)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("frames_{}_{:04}", timestamp, nonce % 10_000));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时帧批次目录失败: {}", e))?;
    Ok(dir)
}

pub(super) fn create_temp_video_frame_dir(app_data_dir: &Path) -> Result<PathBuf, String> {
    let root = app_data_dir.join("temp_video_frames");
    std::fs::create_dir_all(&root).map_err(|e| format!("创建临时视频帧目录失败: {}", e))?;
    cleanup_old_temp_dirs(&root, 11)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = TEMP_VIDEO_FRAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = root.join(format!("video_frames_{}_{:04}", timestamp, nonce % 10_000));
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时视频帧批次目录失败: {}", e))?;
    Ok(dir)
}

pub(super) fn save_temp_frame(
    frame: &image::DynamicImage,
    output_dir: &Path,
    index: usize,
) -> Result<String, String> {
    let filepath = output_dir.join(format!("frame_{:04}.png", index));
    frame
        .save(&filepath)
        .map_err(|e| format!("保存临时帧失败: {}", e))?;
    Ok(filepath.to_string_lossy().to_string())
}

fn cleanup_old_temp_dirs(root: &Path, max_keep: usize) -> Result<(), String> {
    let mut dirs = Vec::new();
    for entry in std::fs::read_dir(root).map_err(|e| format!("读取临时帧目录失败: {e}"))? {
        let entry = entry.map_err(|e| format!("读取临时帧目录项失败: {e}"))?;
        let metadata = entry
            .metadata()
            .map_err(|e| format!("读取临时帧目录项元数据失败: {e}"))?;
        if metadata.is_dir() {
            let modified = metadata
                .modified()
                .map_err(|e| format!("读取临时帧目录修改时间失败: {e}"))?;
            dirs.push((modified, entry.path()));
        }
    }
    dirs.sort_by_key(|(modified, _)| *modified);

    if dirs.len() <= max_keep {
        return Ok(());
    }
    let remove_count = dirs.len() - max_keep;
    for (_, path) in dirs.into_iter().take(remove_count) {
        std::fs::remove_dir_all(&path)
            .map_err(|e| format!("删除旧临时帧目录 {} 失败: {e}", path.display()))?;
    }
    Ok(())
}
