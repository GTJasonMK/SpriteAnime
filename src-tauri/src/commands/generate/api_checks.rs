use tauri::command;

use crate::api_client::{self, ApiCheckResult};

use super::config_commands::{
    require_api_setting, require_prompt_optimizer_api_settings, IMAGE_API_SETTINGS_RESOLUTION,
};
use super::prompt::{API_CHECK_TEXT_INPUT, API_CHECK_TEXT_INSTRUCTIONS};

/// 轻量检测生图 API：只访问 /models，不触发生图。
#[command]
pub async fn check_generation_api(
    api_key: String,
    api_base: String,
    model: String,
    proxy_url: String,
) -> Result<ApiCheckResult, String> {
    let api_key = require_api_setting(api_key, "生图 API Key", IMAGE_API_SETTINGS_RESOLUTION)?;
    let api_base = require_api_setting(api_base, "生图 API 地址", IMAGE_API_SETTINGS_RESOLUTION)?;
    let model = require_api_setting(model, "生图模型", IMAGE_API_SETTINGS_RESOLUTION)?;
    let proxy_url = proxy_url.trim().to_string();

    api_client::check_models_api_connection(&api_base, &api_key, &model, &proxy_url).await
}

/// 检测提示词优化 API：必须跑一次极小文本请求，/models 只作为模型名提示。
#[command]
pub async fn check_prompt_optimizer_api(
    api_key: String,
    api_base: String,
    model: String,
    api_mode: String,
    proxy_url: String,
) -> Result<ApiCheckResult, String> {
    let settings =
        require_prompt_optimizer_api_settings(api_key, api_base, api_mode, model, proxy_url)?;
    let api_key = settings.api_key;
    let api_base = settings.api_base;
    let api_mode = settings.api_mode;
    let model = settings.model;
    let proxy_url = settings.proxy_url;

    api_client::call_prompt_optimizer_text_api(api_client::PromptOptimizerTextRequest {
        api_mode,
        api_base: &api_base,
        api_key: &api_key,
        instructions: API_CHECK_TEXT_INSTRUCTIONS,
        input: API_CHECK_TEXT_INPUT,
        input_image_data_url: None,
        model: &model,
        proxy_url: &proxy_url,
    })
    .await?;

    Ok(ApiCheckResult {
        status: "ok".into(),
        message: "文本调用成功，提示词优化 API 可用。".into(),
        endpoint: format!(
            "{}/{}",
            api_base.trim_end_matches('/'),
            match api_mode {
                crate::config::PromptOptimizerApiMode::Responses => "responses",
                crate::config::PromptOptimizerApiMode::ChatCompletions => "chat/completions",
            }
        ),
        model,
    })
}
