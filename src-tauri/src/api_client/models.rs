use serde_json::Value;

use super::transport::{
    create_client, describe_send_error, parse_http_error, read_response_text, response_preview,
};
use super::types::{ApiCheckResult, API_CHECK_TIMEOUT};
use super::utils::{endpoint_url, extract_model_ids};

pub async fn check_models_api_connection(
    api_base: &str,
    api_key: &str,
    model: &str,
    proxy_url: &str,
) -> Result<ApiCheckResult, String> {
    let api_base = api_base.trim();
    let api_key = api_key.trim();
    let model = model.trim();
    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if api_base.is_empty() {
        return Err("API 地址为空".into());
    }
    if model.is_empty() {
        return Err("模型为空".into());
    }

    let url = endpoint_url(api_base, "models");
    let api_client = create_client(proxy_url)?;

    let resp = api_client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .timeout(API_CHECK_TIMEOUT)
        .send()
        .await
        .map_err(|e| describe_send_error(&e))?;

    let status = resp.status();
    let resp_body = read_response_text(resp, "/models").await?;
    if !status.is_success() {
        return Err(parse_http_error(status.as_u16(), &resp_body));
    }

    let value = parse_models_response_body(&resp_body)?;
    let model_ids = extract_model_ids(&value);

    Ok(build_models_api_check_result(url, model, &model_ids))
}

pub(super) fn parse_models_response_body(body: &str) -> Result<Value, String> {
    serde_json::from_str::<Value>(body.trim()).map_err(|err| {
        format!(
            "/models 响应解析失败：HTTP 成功但响应体不是合法 JSON。解析错误：{err}。解决方法：请确认 API 地址指向兼容 OpenAI 的接口根路径，/models 返回 JSON；如果返回 HTML、登录页或网关页面，请修正 API 地址、代理或服务配置。响应预览: {}",
            response_preview(body)
        )
    })
}

pub(super) fn build_models_api_check_result(
    endpoint: String,
    model: &str,
    model_ids: &[String],
) -> ApiCheckResult {
    let model = model.trim();
    let model_found = if model_ids.is_empty() {
        None
    } else {
        Some(model_ids.iter().any(|id| id == model))
    };

    let (status, message) = match model_found {
        Some(true) => (
            "ok",
            format!("连接成功，/models 可访问，模型 `{model}` 存在。"),
        ),
        Some(false) => (
            "warning",
            format!(
                "连接成功，/models 可访问，但模型列表中未找到 `{model}`。请确认模型名称是否由该服务提供。"
            ),
        ),
        None => (
            "warning",
            "连接成功，/models 返回 JSON，但未发现可识别的模型 ID，无法确认模型名。请确认模型名称和 /models 响应结构。".into(),
        ),
    };

    ApiCheckResult {
        status: status.into(),
        message,
        endpoint,
        model: model.into(),
    }
}
