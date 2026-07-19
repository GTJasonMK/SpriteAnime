use tauri::{command, AppHandle, State};

use crate::config::{self, AppState, PresetsPayload, UserConfig};
use crate::services::config::ConfigService;

use super::types::{ConfigFileResult, ImportedConfigResult};

/// 获取所有预设选项。
#[command]
pub fn get_presets() -> PresetsPayload {
    config::get_presets()
}

/// 加载用户配置
#[command]
pub fn load_config(state: State<'_, AppState>) -> Result<UserConfig, String> {
    ConfigService::new(&state)
        .load()
        .map_err(|error| error.to_string())
}

/// 保存用户配置
#[command]
pub fn save_config(state: State<'_, AppState>, config: UserConfig) -> Result<(), String> {
    ConfigService::new(&state)
        .replace(config)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// 导出清除密钥后的可重新导入配置到 JSON 文件。
#[command]
pub fn export_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: UserConfig,
) -> Result<ConfigFileResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("导出 SpriteAnimte 配置")
        .set_file_name("sprite-animte-config.json")
        .add_filter("JSON 配置", &["json"])
        .blocking_save_file()
    else {
        return Err("用户取消选择".into());
    };

    let path = file_path
        .into_path()
        .map_err(|e| format!("解析导出路径失败: {e}"))?;
    let path = ConfigService::new(&state)
        .export_value(config, &path, false)
        .map_err(|error| error.to_string())?;

    Ok(ConfigFileResult {
        file_path: path.to_string_lossy().to_string(),
    })
}

/// 从 JSON 文件导入用户配置，并立即替换当前配置。
#[command]
pub fn import_config(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ImportedConfigResult, String> {
    use tauri_plugin_dialog::DialogExt;

    let Some(file_path) = app
        .dialog()
        .file()
        .set_title("导入 SpriteAnimte 配置")
        .add_filter("JSON 配置", &["json"])
        .blocking_pick_file()
    else {
        return Err("用户取消选择".into());
    };

    let path = file_path
        .into_path()
        .map_err(|e| format!("解析导入路径失败: {e}"))?;
    let imported = ConfigService::new(&state)
        .import(&path)
        .map_err(|error| error.to_string())?;

    Ok(ImportedConfigResult {
        file_path: path.to_string_lossy().to_string(),
        config: imported,
    })
}

pub(super) const IMAGE_API_SETTINGS_RESOLUTION: &str =
    "请打开设置 > API 配置 > 图片生成，填写 API Key、API 地址和模型后重试。";
pub(super) const PROMPT_OPTIMIZER_API_SETTINGS_RESOLUTION: &str =
    "请打开设置 > API 配置 > 提示词优化，填写优化 API Key、优化 API 地址、调用方式和优化模型后重试。";
pub(super) const VIDEO_API_SETTINGS_RESOLUTION: &str =
    "请打开设置 > API 配置 > 视频生成，填写视频 API Key、视频 API 地址和视频模型后重试。";

pub(super) struct PromptOptimizerApiSettings {
    pub(super) api_key: String,
    pub(super) api_base: String,
    pub(super) api_mode: crate::config::PromptOptimizerApiMode,
    pub(super) model: String,
    pub(super) proxy_url: String,
}

pub(super) fn require_prompt_optimizer_api_settings(
    api_key: String,
    api_base: String,
    api_mode: String,
    model: String,
    proxy_url: String,
) -> Result<PromptOptimizerApiSettings, String> {
    Ok(PromptOptimizerApiSettings {
        api_key: require_api_setting(
            api_key,
            "提示词优化 API Key",
            PROMPT_OPTIMIZER_API_SETTINGS_RESOLUTION,
        )?,
        api_base: require_api_setting(
            api_base,
            "提示词优化 API 地址",
            PROMPT_OPTIMIZER_API_SETTINGS_RESOLUTION,
        )?,
        api_mode: crate::config::parse_prompt_optimizer_api_mode(&api_mode, "提示词优化调用方式")?,
        model: require_api_setting(
            model,
            "提示词优化模型",
            PROMPT_OPTIMIZER_API_SETTINGS_RESOLUTION,
        )?,
        proxy_url: proxy_url.trim().to_string(),
    })
}

pub(super) fn require_api_setting(
    value: String,
    label: &str,
    resolution: &str,
) -> Result<String, String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        Err(format!("{label}为空。解决方法：{resolution}"))
    } else {
        Ok(value)
    }
}
