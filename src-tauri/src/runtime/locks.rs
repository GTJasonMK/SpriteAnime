use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use super::{AppError, AppErrorKind, AppResult};

#[derive(Debug, Clone, Copy)]
pub enum LockDomain {
    Config,
    Workbench,
    Workspace,
    Redraw,
    Assets,
    Tools,
}

impl LockDomain {
    fn file_name(self) -> &'static str {
        match self {
            Self::Config => "config.lock",
            Self::Workbench => "workbench.lock",
            Self::Workspace => "workspace.lock",
            Self::Redraw => "redraw.lock",
            Self::Assets => "assets.lock",
            Self::Tools => "tools.lock",
        }
    }
}

pub struct DataLock {
    _file: File,
}

impl DataLock {
    pub fn shared(locks_dir: &Path, domain: LockDomain) -> AppResult<Self> {
        Self::acquire(locks_dir, domain, false)
    }

    pub fn exclusive(locks_dir: &Path, domain: LockDomain) -> AppResult<Self> {
        Self::acquire(locks_dir, domain, true)
    }

    fn acquire(locks_dir: &Path, domain: LockDomain, exclusive: bool) -> AppResult<Self> {
        std::fs::create_dir_all(locks_dir)
            .map_err(|error| AppError::filesystem(format!("创建锁目录失败: {error}")))?;
        let path = locks_dir.join(domain.file_name());
        let file = open_lock_file(&path)?;
        let result = if exclusive {
            FileExt::try_lock_exclusive(&file)
        } else {
            FileExt::try_lock_shared(&file)
        };
        result.map_err(|error| {
            AppError::new(
                AppErrorKind::Busy,
                "data_store_busy",
                format!(
                    "共享数据正在被另一个 SpriteAnime 进程使用：{} ({error})",
                    path.display()
                ),
                "请等待另一个桌面应用或 CLI 操作完成后重试。",
            )
        })?;
        Ok(Self { _file: file })
    }
}

fn open_lock_file(path: &PathBuf) -> AppResult<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| AppError::filesystem(format!("打开锁文件失败: {error}")))
}
