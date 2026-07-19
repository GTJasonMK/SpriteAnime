use serde_json::Value;

use crate::config::PromptOptimizerApiMode;

use super::sse::sse_data_payloads;
use super::transport::{create_client, post_authenticated_json, response_preview};
use super::utils::endpoint_url;

pub struct PromptOptimizerTextRequest<'a> {
    pub api_mode: PromptOptimizerApiMode,
    pub api_base: &'a str,
    pub api_key: &'a str,
    pub instructions: &'a str,
    pub input: &'a str,
    pub input_image_data_url: Option<&'a str>,
    pub model: &'a str,
    pub proxy_url: &'a str,
}

pub async fn call_prompt_optimizer_text_api(
    request: PromptOptimizerTextRequest<'_>,
) -> Result<String, String> {
    let PromptOptimizerTextRequest {
        api_mode,
        api_base,
        api_key,
        instructions,
        input,
        input_image_data_url,
        model,
        proxy_url,
    } = request;
    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if input.is_empty() {
        return Err("优化输入为空".into());
    }
    match api_mode {
        PromptOptimizerApiMode::Responses => {
            call_responses_text_api(
                api_base,
                api_key,
                instructions,
                input,
                input_image_data_url,
                model,
                proxy_url,
            )
            .await
        }
        PromptOptimizerApiMode::ChatCompletions => {
            call_chat_completions_text_api(
                api_base,
                api_key,
                instructions,
                input,
                input_image_data_url,
                model,
                proxy_url,
            )
            .await
        }
    }
}

/// POST /responses — 文本请求，用于提示词优化。
async fn call_responses_text_api(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
    model: &str,
    proxy_url: &str,
) -> Result<String, String> {
    call_responses_text_api_once(
        api_base,
        api_key,
        instructions,
        input,
        input_image_data_url,
        model,
        proxy_url,
    )
    .await
    .map_err(|err| build_responses_text_error(err, input_image_data_url))
}

async fn call_responses_text_api_once(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
    model: &str,
    proxy_url: &str,
) -> Result<String, String> {
    let url = endpoint_url(api_base, "responses");
    let api_client = create_client(proxy_url)?;

    let body = build_responses_text_body(model, instructions, input, input_image_data_url);
    send_responses_text_request(&api_client, &url, api_key, &body).await
}

pub(super) fn build_responses_text_error(
    err: String,
    input_image_data_url: Option<&str>,
) -> String {
    if input_image_data_url.is_some() {
        format!(
            "/responses 文本请求失败：{err}。解决方法：请确认当前提示词优化 API 地址支持 Responses 多模态输入，且模型支持图像输入；如果模型不支持参考图视觉理解，请在界面关闭“参考图视觉理解”后重试。"
        )
    } else {
        format!(
            "/responses 文本请求失败：{err}。解决方法：请确认提示词优化 API 地址支持 /responses 文本接口；如果服务只支持 /chat/completions，请在提示词优化设置中把调用方式改为 Chat Completions。"
        )
    }
}

pub(super) fn build_responses_text_body(
    model: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
) -> Value {
    let input_value = if let Some(image_url) = input_image_data_url {
        let mut content = vec![serde_json::json!({"type": "input_text", "text": input})];
        content.push(serde_json::json!({"type": "input_image", "image_url": image_url}));
        serde_json::json!([{ "role": "user", "content": content }])
    } else {
        Value::String(input.to_string())
    };

    serde_json::json!({
        "model": model,
        "instructions": instructions,
        "input": input_value,
    })
}

async fn send_responses_text_request(
    api_client: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &Value,
) -> Result<String, String> {
    let response =
        post_authenticated_json(api_client, url, api_key, body, "/responses 文本请求").await?;

    parse_text_api_response("Responses API", &response.body, &response.content_type)
}

async fn call_chat_completions_text_api(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
    model: &str,
    proxy_url: &str,
) -> Result<String, String> {
    let url = endpoint_url(api_base, "chat/completions");
    let user_content = if let Some(image_url) = input_image_data_url {
        serde_json::json!([
            {"type": "text", "text": input},
            {"type": "image_url", "image_url": {"url": image_url}}
        ])
    } else {
        Value::String(input.to_string())
    };

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": user_content}
        ],
        "temperature": 0.2,
        "response_format": {"type": "json_object"},
    });

    let api_client = create_client(proxy_url)?;
    let response = post_authenticated_json(
        &api_client,
        &url,
        api_key,
        &body,
        "/chat/completions 文本请求",
    )
    .await?;

    parse_text_api_response(
        "Chat Completions API",
        &response.body,
        &response.content_type,
    )
}

pub(super) fn parse_text_api_response(
    service_name: &str,
    resp_body: &str,
    content_type: &str,
) -> Result<String, String> {
    let trimmed = resp_body.trim();
    if trimmed.is_empty() {
        return Err(format!("{service_name} 返回空响应：HTTP 成功但响应体为空"));
    }

    let content_type = content_type.to_ascii_lowercase();
    if content_type.contains("text/html")
        || trimmed.starts_with("<!doctype")
        || trimmed.starts_with("<html")
    {
        return Err(format!(
            "{service_name} 返回了 HTML 页面，不是可解析的文本 API 响应；通常表示 API 地址填成了网页首页/控制台地址，或缺少 /v1 这样的 API 路径；响应预览: {}",
            response_preview(trimmed)
        ));
    }

    if looks_like_sse_body(trimmed) {
        if let Some(text) = extract_response_text_from_sse_body(trimmed) {
            return Ok(text.trim().to_string());
        }
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        if is_direct_text_payload(&value) {
            return Ok(trimmed.to_string());
        }
        let text = extract_response_text(&value).ok_or_else(|| {
            format!(
                "{service_name} 未返回文本内容；响应预览: {}",
                response_preview(trimmed)
            )
        })?;
        return Ok(text.trim().to_string());
    }

    if content_type.contains("text/plain") || may_be_plain_model_text(trimmed) {
        return Ok(trimmed.to_string());
    }

    Err(format!(
        "解析文本响应失败：响应不是 JSON、SSE 或纯文本；响应预览: {}",
        response_preview(trimmed)
    ))
}

pub(super) fn looks_like_sse_body(value: &str) -> bool {
    value.starts_with("data:") || value.contains("\ndata:") || value.contains("\r\ndata:")
}

fn extract_response_text_from_sse_body(raw: &str) -> Option<String> {
    let mut chunks = Vec::new();
    for payload in sse_data_payloads(raw) {
        if let Ok(value) = serde_json::from_str::<Value>(&payload) {
            collect_response_text_chunks(&value, &mut chunks);
        } else if !payload.trim().is_empty() {
            chunks.push(payload);
        }
    }
    let text = chunks
        .into_iter()
        .map(|chunk| chunk.trim().to_string())
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>()
        .join("");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn is_direct_text_payload(value: &Value) -> bool {
    value.get("prompt").is_some()
        || value.get("negative_prompt").is_some()
        || value.get("grid_rows").is_some()
        || value.get("ok").is_some()
}

fn may_be_plain_model_text(value: &str) -> bool {
    !value.starts_with('{') && !value.starts_with('[') && !value.starts_with('<')
}

fn extract_response_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("output_text").and_then(Value::as_str) {
        if !text.trim().is_empty() {
            return Some(text.to_string());
        }
    }

    let mut chunks = Vec::new();
    collect_response_text_chunks(value, &mut chunks);
    let text = chunks
        .into_iter()
        .map(|chunk| chunk.trim().to_string())
        .filter(|chunk| !chunk.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn collect_response_text_chunks(value: &Value, chunks: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_response_text_chunks(item, chunks);
            }
        }
        Value::Object(map) => {
            if looks_like_prompt_optimizer_payload(value) {
                if let Ok(text) = serde_json::to_string(value) {
                    chunks.push(text);
                }
            }
            let type_name = map.get("type").and_then(Value::as_str);
            if matches!(type_name, Some("output_text" | "text"))
                && map.get("text").and_then(Value::as_str).is_some()
            {
                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    chunks.push(text.to_string());
                }
            } else if type_name == Some("message") {
                if let Some(content) = map.get("content") {
                    collect_response_text_chunks(content, chunks);
                }
            } else if let Some(text) = map.get("content").and_then(Value::as_str) {
                chunks.push(text.to_string());
            }
            if type_name != Some("message") {
                if let Some(content) = map.get("content").filter(|value| !value.is_string()) {
                    collect_response_text_chunks(content, chunks);
                }
            }

            if let Some(output) = map.get("output") {
                collect_response_text_chunks(output, chunks);
            }
            if let Some(data) = map.get("data") {
                collect_response_text_chunks(data, chunks);
            }
            if let Some(choices) = map.get("choices") {
                collect_response_text_chunks(choices, chunks);
            }
            if let Some(message) = map.get("message") {
                collect_response_text_chunks(message, chunks);
            }
            if let Some(delta) = map.get("delta") {
                if let Some(text) = delta.as_str() {
                    chunks.push(text.to_string());
                } else {
                    collect_response_text_chunks(delta, chunks);
                }
            }
        }
        _ => {}
    }
}

fn looks_like_prompt_optimizer_payload(value: &Value) -> bool {
    let Some(map) = value.as_object() else {
        return false;
    };
    map.get("prompt").and_then(Value::as_str).is_some()
        && map.get("negative_prompt").is_some()
        && map.get("grid_rows").is_some()
        && map.get("grid_cols").is_some()
}
