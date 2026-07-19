use tauri::{command, State};

use crate::config::AppState;
use crate::services::records::{PromptHistoryService, WorkbenchService};
use crate::workbench::WorkbenchRecord;

/// 添加提示词到历史
#[command]
pub fn add_prompt_history(
    state: State<'_, AppState>,
    prompt: String,
) -> Result<Vec<String>, String> {
    PromptHistoryService::new(&state)
        .add(&prompt)
        .map_err(|error| error.to_string())
}

/// 读取工作台图片记录
#[command]
pub fn read_workbench_records(
    state: State<'_, AppState>,
    limit: usize,
) -> Result<Vec<WorkbenchRecord>, String> {
    WorkbenchService::new(&state)
        .list(limit)
        .map_err(|error| error.to_string())
}

/// 新增或更新工作台图片记录
#[command]
pub fn upsert_workbench_records(
    state: State<'_, AppState>,
    records: Vec<WorkbenchRecord>,
) -> Result<Vec<WorkbenchRecord>, String> {
    WorkbenchService::new(&state)
        .upsert(records)
        .map_err(|error| error.to_string())
}

/// 从工作台移除一条记录，不删除实际图片文件
#[command]
pub fn delete_workbench_record(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<WorkbenchRecord>, String> {
    WorkbenchService::new(&state)
        .delete(&id)
        .map_err(|error| error.to_string())
}

/// 清空工作台记录，不删除实际图片文件
#[command]
pub fn clear_workbench_records(state: State<'_, AppState>) -> Result<(), String> {
    WorkbenchService::new(&state)
        .clear()
        .map_err(|error| error.to_string())
}
