use std::path::{Path, PathBuf};

use super::{AppError, AppResult};

const APP_DATA_DIR_NAME: &str = "SpriteAnimteData";

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub data_dir: PathBuf,
    pub config: PathBuf,
    pub workspace: PathBuf,
    pub workbench: PathBuf,
    pub logs: PathBuf,
    pub assets: PathBuf,
    pub locks: PathBuf,
}

impl AppPaths {
    pub fn discover(override_dir: Option<PathBuf>) -> AppResult<Self> {
        let data_dir = match override_dir {
            Some(path) => absolutize(path)?,
            None => match std::env::var_os("SPRITE_ANIME_DATA_DIR") {
                Some(path) => absolutize(PathBuf::from(path))?,
                None => executable_root()?.join(APP_DATA_DIR_NAME),
            },
        };
        Ok(Self::from_data_dir(data_dir))
    }

    pub fn from_data_dir(data_dir: PathBuf) -> Self {
        Self {
            config: data_dir.join("config.json"),
            workspace: data_dir.join("workspace.json"),
            workbench: data_dir.join("workbench_records.json"),
            logs: data_dir.join("logs"),
            assets: data_dir.join("assets"),
            locks: data_dir.join(".locks"),
            data_dir,
        }
    }

    pub fn ensure_directories(&self) -> AppResult<()> {
        for path in [&self.data_dir, &self.logs, &self.assets, &self.locks] {
            std::fs::create_dir_all(path).map_err(|error| {
                AppError::filesystem(format!("创建目录失败：{} ({error})", path.display()))
            })?;
        }
        Ok(())
    }
}

fn executable_root() -> AppResult<PathBuf> {
    if let Some(appimage) = std::env::var_os("APPIMAGE") {
        let appimage = absolutize(PathBuf::from(appimage))?;
        return parent_path(&appimage, "无法获取 AppImage 所在目录");
    }
    let executable = std::env::current_exe()
        .map_err(|error| AppError::filesystem(format!("无法获取应用可执行文件路径: {error}")))?;
    #[cfg(target_os = "macos")]
    if let Some(bundle_parent) = macos_bundle_parent(&executable) {
        return Ok(bundle_parent);
    }
    parent_path(&executable, "无法获取应用所在目录")
}

fn absolutize(path: PathBuf) -> AppResult<PathBuf> {
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()
        .map_err(|error| AppError::filesystem(format!("无法获取当前目录: {error}")))?
        .join(path))
}

fn parent_path(path: &Path, message: &str) -> AppResult<PathBuf> {
    path.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::filesystem(message))
}

#[cfg(target_os = "macos")]
fn macos_bundle_parent(executable: &Path) -> Option<PathBuf> {
    executable
        .ancestors()
        .find(|path| path.extension().and_then(|ext| ext.to_str()) == Some("app"))
        .and_then(Path::parent)
        .map(Path::to_path_buf)
}
