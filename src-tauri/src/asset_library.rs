use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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

const STANDARD_CATEGORIES: [AssetCategory; 8] = [
    AssetCategory::GeneratedImages,
    AssetCategory::ImportedImages,
    AssetCategory::MattedImages,
    AssetCategory::OriginalVideos,
    AssetCategory::GeneratedVideos,
    AssetCategory::VideoSpriteSheets,
    AssetCategory::ExportedFrameSets,
    AssetCategory::ExportedGifs,
];

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

pub fn root_dir(default_save_dir: &Path, configured_save_dir: &str) -> PathBuf {
    let _ = configured_save_dir;
    default_save_dir.to_path_buf()
}

pub fn category_dir(
    default_save_dir: &Path,
    configured_save_dir: &str,
    category: AssetCategory,
) -> Result<PathBuf, String> {
    let dir = root_dir(default_save_dir, configured_save_dir).join(category.dir_name());
    std::fs::create_dir_all(&dir).map_err(|e| format!("创建素材目录失败: {}", e))?;
    Ok(dir)
}

pub fn ensure_standard_dirs(
    default_save_dir: &Path,
    configured_save_dir: &str,
) -> Result<Vec<PathBuf>, String> {
    STANDARD_CATEGORIES
        .into_iter()
        .map(|category| category_dir(default_save_dir, configured_save_dir, category))
        .collect()
}

pub fn copy_file_to_category(
    source_path: &str,
    default_save_dir: &Path,
    configured_save_dir: &str,
    category: AssetCategory,
) -> Result<PathBuf, String> {
    let source = Path::new(source_path);
    if !source.is_file() {
        return Err("源文件不存在或不是普通文件".into());
    }

    let output_dir = category_dir(default_save_dir, configured_save_dir, category)?;
    if is_already_inside_dir(source, &output_dir) {
        return Ok(source.to_path_buf());
    }

    let stem = source
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "asset".into());
    let extension = source
        .extension()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty());
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let nonce = ASSET_COPY_COUNTER.fetch_add(1, Ordering::Relaxed) % 10_000;
    let file_name = match extension {
        Some(extension) => format!(
            "{}_{}_{:04}.{}",
            sanitize_path_segment(&stem),
            timestamp,
            nonce,
            sanitize_path_segment(&extension)
        ),
        None => format!(
            "{}_{}_{:04}",
            sanitize_path_segment(&stem),
            timestamp,
            nonce
        ),
    };
    let dest = output_dir.join(file_name);
    std::fs::copy(source, &dest).map_err(|e| format!("复制素材文件失败: {}", e))?;
    Ok(dest)
}

fn is_already_inside_dir(source: &Path, dir: &Path) -> bool {
    let Some(source_parent) = source.parent() else {
        return false;
    };
    let Ok(source_parent) = source_parent.canonicalize() else {
        return false;
    };
    let Ok(dir) = dir.canonicalize() else {
        return false;
    };
    source_parent == dir
}

fn sanitize_path_segment(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch == '_' {
                Some(ch)
            } else if ch == '.' || ch.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .collect();
    let collapsed = sanitized
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    if collapsed.is_empty() {
        "asset".into()
    } else {
        collapsed.chars().take(48).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_dir_always_uses_portable_default() {
        let root = std::env::temp_dir().join("sprite-asset-root");
        let default = std::env::temp_dir().join("sprite-default-root");
        let dir = root_dir(&default, &root.to_string_lossy());
        assert_eq!(dir, default);
    }

    #[test]
    fn sanitize_path_segment_keeps_safe_ascii() {
        assert_eq!(sanitize_path_segment("My Asset-01.png"), "my_asset-01_png");
        assert_eq!(sanitize_path_segment("角色"), "asset");
    }
}
