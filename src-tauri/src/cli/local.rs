use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use clap::{Subcommand, ValueEnum};
use sha2::{Digest, Sha256};

use crate::asset_library::AssetCategory;
use crate::commands::{filesystem, tools};
use crate::config::AppState;
use crate::runtime::{AppError, AppResult, DataLock, LockDomain};
use crate::services::records::{PromptHistoryService, WorkbenchService};
use crate::workbench::WorkbenchRecord;

use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum HistoryCommand {
    List {
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Clear {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkbenchCommand {
    List {
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    Add {
        path: PathBuf,
        #[arg(long)]
        label: String,
        #[arg(long, default_value = "")]
        prompt: String,
        #[arg(long)]
        model: String,
        #[arg(long)]
        duration_seconds: Option<f64>,
    },
    Remove {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    Clear {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum AssetsCommand {
    ImportImage {
        path: PathBuf,
    },
    ImportVideo {
        path: PathBuf,
    },
    List {
        #[arg(long)]
        category: Option<AssetKind>,
    },
    Open {
        path: PathBuf,
    },
    Reveal {
        path: PathBuf,
    },
    CleanupTemp {
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum AssetKind {
    GeneratedImages,
    ImportedImages,
    MattedImages,
    OriginalVideos,
    GeneratedVideos,
    VideoSpriteSheets,
    ExportedFrameSets,
    ExportedGifs,
}

impl From<AssetKind> for AssetCategory {
    fn from(value: AssetKind) -> Self {
        match value {
            AssetKind::GeneratedImages => Self::GeneratedImages,
            AssetKind::ImportedImages => Self::ImportedImages,
            AssetKind::MattedImages => Self::MattedImages,
            AssetKind::OriginalVideos => Self::OriginalVideos,
            AssetKind::GeneratedVideos => Self::GeneratedVideos,
            AssetKind::VideoSpriteSheets => Self::VideoSpriteSheets,
            AssetKind::ExportedFrameSets => Self::ExportedFrameSets,
            AssetKind::ExportedGifs => Self::ExportedGifs,
        }
    }
}

#[derive(Debug, Subcommand)]
pub enum ToolsCommand {
    Check,
    Install {
        #[arg(long, default_value = "")]
        proxy: String,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    Show,
    Validate,
}

#[derive(Debug, Subcommand)]
pub enum LogsCommand {
    Path,
    Tail {
        #[arg(long, default_value = "video-sprite.log")]
        file: String,
        #[arg(long, default_value_t = 100)]
        lines: usize,
    },
}

pub fn execute_history(state: &AppState, command: HistoryCommand) -> AppResult<CommandResult> {
    let service = PromptHistoryService::new(state);
    match command {
        HistoryCommand::List { limit } => {
            CommandResult::serializable("history.list", service.list(limit)?)
        }
        HistoryCommand::Clear { yes } => {
            require_yes(yes, "清空提示词历史不可撤销")?;
            service.clear()?;
            CommandResult::serializable("history.clear", serde_json::json!({"cleared": true}))
        }
    }
}

pub fn execute_workbench(state: &AppState, command: WorkbenchCommand) -> AppResult<CommandResult> {
    let service = WorkbenchService::new(state);
    match command {
        WorkbenchCommand::List { limit } => {
            CommandResult::serializable("workbench.list", service.list(limit)?)
        }
        WorkbenchCommand::Add {
            path,
            label,
            prompt,
            model,
            duration_seconds,
        } => {
            if !path.is_file() {
                return Err(AppError::validation(format!(
                    "工作台图片不存在：{}",
                    path.display()
                )));
            }
            let now = chrono::Local::now().to_rfc3339();
            let path = absolute_path(path)?;
            let record = WorkbenchRecord {
                id: workbench_id(&path),
                path: path.to_string_lossy().to_string(),
                label,
                prompt,
                model,
                duration_seconds,
                created_at: now.clone(),
                updated_at: now,
            };
            let records = service.upsert(vec![record])?;
            CommandResult::serializable("workbench.add", records)
        }
        WorkbenchCommand::Remove { id, yes } => {
            require_yes(yes, "移除工作台记录不可撤销")?;
            let records = service.delete(&id)?;
            CommandResult::serializable("workbench.remove", records)
        }
        WorkbenchCommand::Clear { yes } => {
            require_yes(yes, "清空工作台记录不可撤销")?;
            service.clear()?;
            CommandResult::serializable("workbench.clear", serde_json::json!({"cleared": true}))
        }
    }
}

pub fn execute_assets(state: &AppState, command: AssetsCommand) -> AppResult<CommandResult> {
    match command {
        AssetsCommand::ImportImage { path } => import_asset(
            state,
            path,
            AssetCategory::ImportedImages,
            "图片素材",
            "assets.import-image",
        ),
        AssetsCommand::ImportVideo { path } => import_asset(
            state,
            path,
            AssetCategory::OriginalVideos,
            "视频素材",
            "assets.import-video",
        ),
        AssetsCommand::List { category } => {
            let _lock = DataLock::shared(&state.locks_dir, LockDomain::Assets)?;
            let root = category
                .map(|kind| {
                    state
                        .default_save_dir
                        .join(AssetCategory::from(kind).dir_name())
                })
                .unwrap_or_else(|| state.default_save_dir.clone());
            let files = list_files(&root)?;
            CommandResult::serializable(
                "assets.list",
                serde_json::json!({"root": root, "files": files}),
            )
        }
        AssetsCommand::Open { path } => {
            require_existing_path(&path)?;
            opener::open(&path)
                .map_err(|error| AppError::filesystem(format!("打开文件失败: {error}")))?;
            CommandResult::serializable("assets.open", serde_json::json!({"path": path}))
        }
        AssetsCommand::Reveal { path } => {
            require_existing_path(&path)?;
            let target = if path.is_dir() {
                path
            } else {
                path.parent()
                    .ok_or_else(|| AppError::validation("素材路径缺少父目录"))?
                    .to_path_buf()
            };
            opener::open(&target)
                .map_err(|error| AppError::filesystem(format!("打开文件管理器失败: {error}")))?;
            CommandResult::serializable("assets.reveal", serde_json::json!({"path": target}))
        }
        AssetsCommand::CleanupTemp { yes } => {
            require_yes(yes, "清理临时抽帧目录不可撤销")?;
            let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
            let root = state.app_data_dir.join("temp_video_frames");
            let result = filesystem::cleanup_dirs_in_root(&root).map_err(AppError::filesystem)?;
            CommandResult::serializable("assets.cleanup-temp", result)
        }
    }
}

pub async fn execute_tools(state: &AppState, command: ToolsCommand) -> AppResult<CommandResult> {
    match command {
        ToolsCommand::Check => {
            let _lock = DataLock::shared(&state.locks_dir, LockDomain::Tools)?;
            CommandResult::serializable("tools.check", tools::check_ffmpeg_tools_inner(state))
        }
        ToolsCommand::Install { proxy } => {
            let _tools_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Tools)?;
            let _config_lock = DataLock::exclusive(&state.locks_dir, LockDomain::Config)?;
            let result = tools::download_ffmpeg_tools_inner(state, proxy)
                .await
                .map_err(AppError::filesystem)?;
            CommandResult::serializable("tools.install", result)
        }
    }
}

pub fn execute_workspace(state: &AppState, command: WorkspaceCommand) -> AppResult<CommandResult> {
    let _lock = DataLock::shared(&state.locks_dir, LockDomain::Workspace)?;
    let snapshot = crate::workspace::read_snapshot(&state.workspace_path, &state.app_data_dir)
        .map_err(AppError::filesystem)?;
    match command {
        WorkspaceCommand::Show => CommandResult::serializable("workspace.show", snapshot),
        WorkspaceCommand::Validate => CommandResult::serializable(
            "workspace.validate",
            serde_json::json!({"valid": true, "exists": snapshot.is_some()}),
        ),
    }
}

pub fn execute_logs(state: &AppState, command: LogsCommand) -> AppResult<CommandResult> {
    match command {
        LogsCommand::Path => {
            CommandResult::serializable("logs.path", serde_json::json!({"path": state.log_dir}))
        }
        LogsCommand::Tail { file, lines } => {
            require_positive(lines, "日志行数")?;
            let name = Path::new(&file)
                .file_name()
                .filter(|name| *name == std::ffi::OsStr::new(&file))
                .ok_or_else(|| AppError::validation("日志文件名不能包含路径"))?;
            let path = state.log_dir.join(name);
            let lines = tail_lines(&path, lines)?;
            CommandResult::serializable(
                "logs.tail",
                serde_json::json!({"path": path, "lines": lines}),
            )
        }
    }
}

fn import_asset(
    state: &AppState,
    path: PathBuf,
    category: AssetCategory,
    context: &str,
    command: &'static str,
) -> AppResult<CommandResult> {
    let _lock = DataLock::exclusive(&state.locks_dir, LockDomain::Assets)?;
    let result = filesystem::import_file_to_library_inner(
        state,
        path.to_string_lossy().to_string(),
        category,
        context,
    )
    .map_err(AppError::filesystem)?;
    CommandResult::serializable(command, result)
}

fn list_files(root: &Path) -> AppResult<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for entry in std::fs::read_dir(&dir)
            .map_err(|error| AppError::filesystem(format!("读取素材目录失败: {error}")))?
        {
            let path = entry
                .map_err(|error| AppError::filesystem(format!("读取素材目录项失败: {error}")))?
                .path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn tail_lines(path: &Path, limit: usize) -> AppResult<Vec<String>> {
    let file = std::fs::File::open(path).map_err(|error| {
        AppError::filesystem(format!("打开日志失败：{} ({error})", path.display()))
    })?;
    let mut lines = VecDeque::with_capacity(limit);
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|error| AppError::filesystem(format!("读取日志失败: {error}")))?;
        if lines.len() == limit {
            lines.pop_front();
        }
        lines.push_back(line);
    }
    Ok(lines.into())
}

fn workbench_id(path: &Path) -> String {
    let digest = Sha256::digest(path.to_string_lossy().as_bytes());
    format!("cli-{:x}", digest)[..20].to_string()
}

fn absolute_path(path: PathBuf) -> AppResult<PathBuf> {
    path.canonicalize().map_err(|error| {
        AppError::filesystem(format!("读取文件路径失败：{} ({error})", path.display()))
    })
}

fn require_existing_path(path: &Path) -> AppResult<()> {
    if path.exists() {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "路径不存在：{}",
            path.display()
        )))
    }
}

fn require_positive(value: usize, label: &str) -> AppResult<()> {
    if value > 0 {
        Ok(())
    } else {
        Err(AppError::validation(format!("{label}必须大于 0")))
    }
}

fn require_yes(yes: bool, message: &str) -> AppResult<()> {
    if yes {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{message}；请添加 --yes 明确确认"
        )))
    }
}
