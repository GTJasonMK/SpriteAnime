use crate::config::AppState;
use crate::runtime::{AppError, AppResult, DataLock, LockDomain};
use crate::workbench::{WorkbenchRecord, WorkbenchStore};

use super::config::ConfigService;

const MAX_PROMPT_HISTORY: usize = 100;

pub struct PromptHistoryService<'a> {
    config: ConfigService<'a>,
}

impl<'a> PromptHistoryService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self {
            config: ConfigService::new(state),
        }
    }

    pub fn list(&self, limit: usize) -> AppResult<Vec<String>> {
        if limit == 0 {
            return Err(AppError::validation("历史记录数量必须大于 0"));
        }
        Ok(self
            .config
            .load()?
            .prompt_history
            .into_iter()
            .take(limit)
            .collect())
    }

    pub fn add(&self, prompt: &str) -> AppResult<Vec<String>> {
        let prompt = prompt.trim();
        if prompt.is_empty() {
            return Err(AppError::validation("提示词历史不能添加空提示词"));
        }
        self.config.update(|config| {
            config.prompt_history.retain(|item| item != prompt);
            config.prompt_history.push_front(prompt.to_string());
            config.prompt_history.truncate(MAX_PROMPT_HISTORY);
            Ok(config.prompt_history.iter().cloned().collect())
        })
    }

    pub fn clear(&self) -> AppResult<()> {
        self.config.update(|config| {
            config.prompt_history.clear();
            Ok(())
        })
    }
}

pub struct WorkbenchService<'a> {
    state: &'a AppState,
}

impl<'a> WorkbenchService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub fn list(&self, limit: usize) -> AppResult<Vec<WorkbenchRecord>> {
        let _lock = DataLock::shared(&self.state.locks_dir, LockDomain::Workbench)?;
        self.store()
            .read_recent(limit)
            .map_err(AppError::filesystem)
    }

    pub fn upsert(&self, records: Vec<WorkbenchRecord>) -> AppResult<Vec<WorkbenchRecord>> {
        let _lock = DataLock::exclusive(&self.state.locks_dir, LockDomain::Workbench)?;
        self.store()
            .upsert_many(records)
            .map_err(AppError::filesystem)
    }

    pub fn delete(&self, id: &str) -> AppResult<Vec<WorkbenchRecord>> {
        let _lock = DataLock::exclusive(&self.state.locks_dir, LockDomain::Workbench)?;
        self.store().delete(id).map_err(AppError::filesystem)
    }

    pub fn clear(&self) -> AppResult<()> {
        let _lock = DataLock::exclusive(&self.state.locks_dir, LockDomain::Workbench)?;
        self.store().clear().map_err(AppError::filesystem)
    }

    fn store(&self) -> WorkbenchStore {
        WorkbenchStore::new(self.state.workbench_records_path.clone())
    }
}
