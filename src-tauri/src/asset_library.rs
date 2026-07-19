use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::path_safety::sanitize_file_name_component;

static ASSET_COPY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy)]
pub enum AssetCategory {
    GeneratedImages,
    ImportedImages,
    MattedImages,
    OriginalVideos,
    GeneratedVideos,
    VideoSpriteSheets,
    ExportedFrameSets,
    ExportedGifs,
}

impl AssetCategory {
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::GeneratedImages => "generated-images",
            Self::ImportedImages => "imported-images",
            Self::MattedImages => "matted-images",
            Self::OriginalVideos => "original-videos",
            Self::GeneratedVideos => "generated-videos",
            Self::VideoSpriteSheets => "video-sprite-sheets",
            Self::ExportedFrameSets => "exported-frame-sets",
            Self::ExportedGifs => "exported-gifs",
        }
    }
}

pub fn category_dir(default_save_dir: &Path, category: AssetCategory) -> Result<PathBuf, String> {
    let dir = default_save_dir.join(category.dir_name());
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建素材目录失败: {}", e))?;
    Ok(dir)
}

pub fn copy_file_to_category(
    source_path: &str,
    default_save_dir: &Path,
    category: AssetCategory,
) -> Result<PathBuf, String> {
    let source = Path::new(source_path);
    if !source.is_file() {
        return Err("源文件不存在或不是普通文件".into());
    }

    let output_dir = category_dir(default_save_dir, category)?;
    if is_already_inside_dir(source, &output_dir)? {
        return Ok(source.to_path_buf());
    }

    let stem = required_source_file_stem(source)?;
    let safe_stem = required_sanitized_path_segment(&stem, "素材文件名")?;
    let extension = source
        .extension()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| "素材文件缺少扩展名".to_string())?;
    let safe_extension = required_sanitized_path_segment(&extension, "素材扩展名")?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = ASSET_COPY_COUNTER.fetch_add(1, Ordering::Relaxed) % 10_000;
    let file_name = format!(
        "{}_{}_{:04}.{}",
        safe_stem, timestamp, nonce, safe_extension
    );
    let dest = output_dir.join(file_name);
    std::fs::copy(source, &dest).map_err(|e| format!("复制素材文件失败: {}", e))?;
    Ok(dest)
}

fn is_already_inside_dir(source: &Path, dir: &Path) -> Result<bool, String> {
    let source_parent = source
        .parent()
        .ok_or_else(|| "源文件缺少父目录".to_string())?
        .canonicalize()
        .map_err(|e| format!("读取源文件父目录失败: {e}"))?;
    let dir = dir
        .canonicalize()
        .map_err(|e| format!("读取素材目录失败: {e}"))?;
    Ok(source_parent == dir)
}

fn required_source_file_stem(source: &Path) -> Result<String, String> {
    source
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| {
            "素材文件缺少可用文件名。解决方法：请把源文件重命名为包含文件名的本地文件后重新导入。"
                .into()
        })
}

fn required_sanitized_path_segment(value: &str, label: &str) -> Result<String, String> {
    let sanitized: String = sanitize_file_name_component(value)
        .chars()
        .take(48)
        .collect();
    if sanitized.is_empty() {
        Err(format!(
            "{label}清洗后为空。解决方法：请把文件名改为包含有效字符的名称后重新导入。"
        ))
    } else {
        Ok(sanitized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_sanitizer_keeps_unicode_asset_names() {
        assert_eq!(
            required_sanitized_path_segment("角色:奔跑", "素材文件名").unwrap(),
            "角色_奔跑"
        );
    }

    #[test]
    fn required_sanitized_path_segment_rejects_empty_result_with_resolution_steps() {
        let err = required_sanitized_path_segment(" ._-_ ", "素材文件名").unwrap_err();

        assert!(err.contains("素材文件名清洗后为空"));
        assert!(err.contains("有效字符"));
        assert!(err.contains("重新导入"));
    }

    #[test]
    fn required_source_file_stem_rejects_missing_file_stem_with_resolution_steps() {
        let err = required_source_file_stem(Path::new("/")).unwrap_err();

        assert!(err.contains("素材文件缺少可用文件名"));
        assert!(err.contains("重命名"));
    }
}
