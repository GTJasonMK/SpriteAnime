mod error;
pub(crate) mod locks;
mod paths;
pub(crate) mod progress;

pub use error::*;
pub use locks::*;
pub use paths::*;
pub use progress::*;

use crate::config::{AppState, UserConfig};
use parking_lot::Mutex;
use std::path::PathBuf;

pub fn create_app_state(data_dir: Option<PathBuf>) -> AppResult<AppState> {
    let paths = AppPaths::discover(data_dir)?;
    paths.ensure_directories()?;
    let _config_lock = DataLock::exclusive(&paths.locks, LockDomain::Config)?;
    let create_config = !paths.config.is_file();
    let user_config = UserConfig::load(&paths.config).map_err(AppError::config)?;
    if create_config {
        user_config.save(&paths.config).map_err(AppError::config)?;
    }
    Ok(AppState {
        config: Mutex::new(user_config),
        app_data_dir: paths.data_dir,
        config_path: paths.config,
        log_dir: paths.logs,
        workbench_records_path: paths.workbench,
        workspace_path: paths.workspace,
        default_save_dir: paths.assets,
        locks_dir: paths.locks,
    })
}
