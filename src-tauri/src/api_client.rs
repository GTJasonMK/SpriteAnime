use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

/// API基础URL（可通过配置修改）
pub const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8000/v1";
const GENERATION_TIMEOUT: Duration = Duration::from_secs(360);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const USER_AGENT: &str = "SpriteAnimte/0.1";

struct ApiHttpClient {
    label: String,
    client: Client,
}

#[derive(Default)]
struct StreamResponseState {
    response_id: Option<String>,
    status: Option<String>,
    model: Option<String>,
    last_event: Option<String>,
}

/// 生成结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationResult {
    /// 图片base64数据列表
    pub images_base64: Vec<String>,
    /// 图片本地路径列表（保存后回填）
    pub image_urls: Vec<String>,
    /// 本次生成总耗时（秒）
    #[serde(default)]
    pub duration_seconds: Option<f64>,
}

/// 创建唯一客户端：显式配置代理时只走代理；没有代理时才使用环境代理或直连。
fn create_client(config_proxy: Option<&str>) -> Result<ApiHttpClient, String> {
    if let Some(url) = config_proxy.filter(|s| !s.trim().is_empty()) {
        eprintln!("[api] 使用配置代理: {url}");
        return Ok(ApiHttpClient {
            label: "配置代理/自动协议".into(),
            client: build_proxy_client(url)?,
        });
    }

    if let Some(url) = env_proxy_url() {
        eprintln!("[api] 使用环境代理: {url}");
        return Ok(ApiHttpClient {
            label: "环境代理/自动协议".into(),
            client: build_proxy_client(&url)?,
        });
    }

    eprintln!("[api] 直连模式");
    Ok(ApiHttpClient {
        label: "直连".into(),
        client: build_direct_client()?,
    })
}

fn build_proxy_client(proxy_url: &str) -> Result<Client, String> {
    Client::builder()
        .proxy(reqwest::Proxy::all(proxy_url).map_err(|e| format!("代理配置失败: {}", e))?)
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(GENERATION_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
}

fn build_direct_client() -> Result<Client, String> {
    Client::builder()
        .no_proxy()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(GENERATION_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
}

fn describe_send_error(e: &reqwest::Error) -> String {
    let detail = describe_reqwest_error(e);
    let lower = detail.to_ascii_lowercase();
    if e.is_timeout() {
        format!("请求超时: {detail}")
    } else if e.is_connect() {
        format!("连接/代理失败: {detail}")
    } else if lower.contains("connection closed before message completed")
        || lower.contains("client error (sendrequest)")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
    {
        format!("连接中断: {detail}")
    } else if e.is_request() {
        format!("请求发送失败: {detail}")
    } else if e.is_body() {
        format!("请求体发送失败: {detail}")
    } else if e.is_decode() {
        format!("响应解码失败: {detail}")
    } else {
        format!("请求异常: {detail}")
    }
}

fn describe_reqwest_error(e: &reqwest::Error) -> String {
    let mut parts = vec![e.to_string()];
    let mut source = e.source();
    while let Some(err) = source {
        let msg = err.to_string();
        if !parts.iter().any(|part| part == &msg) {
            parts.push(msg);
        }
        source = err.source();
    }
    if let Some(url) = e.url() {
        parts.push(format!("url={}", url));
    }
    parts.join(" | ")
}

fn env_proxy_url() -> Option<String> {
    std::env::var("HTTPS_PROXY")
        .or_else(|_| std::env::var("https_proxy"))
        .or_else(|_| std::env::var("HTTP_PROXY"))
        .or_else(|_| std::env::var("http_proxy"))
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// 从HTTP错误响应中提取可读的错误消息
fn parse_http_error(status: u16, body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return format!(
            "HTTP {status}: 服务器返回空响应（中继站可能过载或暂时不可用，请稍后重试）"
        );
    }
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = json["error"]["message"].as_str() {
            return format!("HTTP {status}: {msg}");
        }
        if let Some(msg) = json["message"].as_str() {
            return format!("HTTP {status}: {msg}");
        }
    }
    if body.chars().count() <= 300 {
        format!("HTTP {status}: {body}")
    } else {
        let preview: String = body.chars().take(300).collect();
        format!("HTTP {status}: {preview}...（已截断）")
    }
}

/// ============================================================
/// POST /responses — 固定使用 image_generation 工具的流式请求
/// ============================================================
pub async fn call_responses_api(
    api_base: &str,
    api_key: &str,
    prompt: &str,
    model: &str,
    count: u32,
    size: &str,
    proxy_url: &str,
) -> Result<Vec<String>, String> {
    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if prompt.is_empty() {
        return Err("提示词为空".into());
    }

    let url = endpoint_url(api_base, "responses");
    eprintln!("[api] /responses 请求模型={model} size={size} count={count}");

    let body = serde_json::json!({
        "model": model,
        "input": prompt,
        "tools": [{"type": "image_generation", "size": size, "n": count}],
        "stream": true,
    });

    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    eprintln!("[api] /responses 尝试 {} (stream/sized)", api_client.label);
    let resp = api_client
        .client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] /responses 请求失败: {msg}");
            msg
        })?;

    let status = resp.status();
    if !status.is_success() {
        let resp_body = resp.text().await.unwrap_or_default();
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses {msg}");
        return Err(msg);
    }

    read_responses_stream_images(resp, count).await
}

/// POST /responses — 文本请求，用于提示词优化。
pub async fn call_responses_text_api(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    model: &str,
    proxy_url: &str,
) -> Result<String, String> {
    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if input.is_empty() {
        return Err("优化输入为空".into());
    }
    if should_use_chat_completions(api_base, model) {
        return call_chat_completions_text_api(
            api_base,
            api_key,
            instructions,
            input,
            model,
            proxy_url,
        )
        .await;
    }

    let url = endpoint_url(api_base, "responses");
    eprintln!("[api] /responses 文本请求模型={model}");

    let body = serde_json::json!({
        "model": model,
        "instructions": instructions,
        "input": input,
    });

    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    eprintln!("[api] /responses 尝试 {} (normal/text)", api_client.label);
    let resp = api_client
        .client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] /responses 文本请求失败: {msg}");
            msg
        })?;

    let status = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses 文本请求 {msg}");
        return Err(msg);
    }

    let value: Value =
        serde_json::from_str(&resp_body).map_err(|e| format!("解析文本响应失败: {e}"))?;
    let text =
        extract_response_text(&value).ok_or_else(|| "Responses API 未返回文本内容".to_string())?;
    Ok(text.trim().to_string())
}

async fn call_chat_completions_text_api(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    model: &str,
    proxy_url: &str,
) -> Result<String, String> {
    let url = endpoint_url(api_base, "chat/completions");
    eprintln!("[api] /chat/completions 文本请求模型={model}");

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": instructions},
            {"role": "user", "content": input}
        ],
        "temperature": 0.2,
        "response_format": {"type": "json_object"},
    });

    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    eprintln!(
        "[api] /chat/completions 尝试 {} (normal/text)",
        api_client.label
    );
    let resp = api_client
        .client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] /chat/completions 文本请求失败: {msg}");
            msg
        })?;

    let status = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /chat/completions 文本请求 {msg}");
        return Err(msg);
    }

    let value: Value =
        serde_json::from_str(&resp_body).map_err(|e| format!("解析文本响应失败: {e}"))?;
    let text = extract_response_text(&value)
        .ok_or_else(|| "Chat Completions API 未返回文本内容".to_string())?;
    Ok(text.trim().to_string())
}

fn should_use_chat_completions(api_base: &str, model: &str) -> bool {
    let api_base = api_base.to_ascii_lowercase();
    let model = model.to_ascii_lowercase();
    api_base.contains("api.deepseek.com") || model.starts_with("deepseek")
}

async fn read_responses_stream_images(
    mut resp: reqwest::Response,
    count: u32,
) -> Result<Vec<String>, String> {
    let mut buffer = String::new();
    let mut images = Vec::new();
    let mut stream_state = StreamResponseState::default();
    let mut saw_generation_started = false;
    let mut stream_error: Option<String> = None;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("读取流式响应失败: {}", describe_send_error(&e)))?
    {
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);
        process_complete_sse_events(
            &mut buffer,
            &mut images,
            &mut stream_state,
            &mut saw_generation_started,
            &mut stream_error,
        );
    }

    if !buffer.trim().is_empty() {
        process_sse_event(
            &buffer,
            &mut images,
            &mut stream_state,
            &mut saw_generation_started,
            &mut stream_error,
        );
        buffer.clear();
    }

    if images.is_empty() {
        if let Some(msg) = stream_error {
            return Err(msg);
        }
        if saw_generation_started {
            let id_hint = stream_state
                .response_id
                .as_deref()
                .map(|id| format!(" response_id={id};"))
                .unwrap_or_default();
            let last_event_hint = stream_state
                .last_event
                .as_deref()
                .map(|event| format!(" last_event={event};"))
                .unwrap_or_default();
            return Err(format!(
                "中继已开始生图但提前结束流，未返回最终图片结果;{id_hint}{last_event_hint} anyrouter 当前路由可能没有完整转发生图结果"
            ));
        }
        let state_hint = format_stream_state_hint(&stream_state);
        return Err(format!("Responses API 流式响应未返回图片{state_hint}"));
    }
    let mut images = dedupe_images(images);
    images.truncate(count as usize);
    Ok(images)
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
            let type_name = map.get("type").and_then(Value::as_str).unwrap_or_default();
            if (type_name == "output_text" || type_name == "text")
                && map.get("text").and_then(Value::as_str).is_some()
            {
                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    chunks.push(text.to_string());
                }
            } else if type_name == "message" {
                if let Some(content) = map.get("content") {
                    collect_response_text_chunks(content, chunks);
                }
            } else if let Some(text) = map.get("content").and_then(Value::as_str) {
                chunks.push(text.to_string());
            }

            if let Some(output) = map.get("output") {
                collect_response_text_chunks(output, chunks);
            }
            if let Some(choices) = map.get("choices") {
                collect_response_text_chunks(choices, chunks);
            }
            if let Some(message) = map.get("message") {
                collect_response_text_chunks(message, chunks);
            }
        }
        _ => {}
    }
}

fn process_complete_sse_events(
    buffer: &mut String,
    images: &mut Vec<String>,
    state: &mut StreamResponseState,
    saw_generation_started: &mut bool,
    stream_error: &mut Option<String>,
) {
    while let Some((end, delimiter_len)) = find_sse_event_end(buffer) {
        let event = buffer[..end].to_string();
        buffer.drain(..end + delimiter_len);
        process_sse_event(
            event.as_str(),
            images,
            state,
            saw_generation_started,
            stream_error,
        );
    }
}

fn find_sse_event_end(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|index| (index, 2));
    let crlf = buffer.find("\r\n\r\n").map(|index| (index, 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

fn process_sse_event(
    event: &str,
    images: &mut Vec<String>,
    state: &mut StreamResponseState,
    saw_generation_started: &mut bool,
    stream_error: &mut Option<String>,
) {
    for data in sse_event_payloads(event) {
        let Ok(value) = serde_json::from_str::<Value>(&data) else {
            continue;
        };
        update_stream_state_from_value(state, &value);
        if stream_error.is_none() {
            if let Some(msg) = value["error"]["message"].as_str() {
                *stream_error = Some(format!("Responses API 流式错误: {msg}"));
            } else if let Some(msg) = value["response"]["error"]["message"].as_str() {
                *stream_error = Some(format!("Responses API 流式错误: {msg}"));
            }
        }
        if value["type"].as_str() == Some("response.image_generation_call.generating") {
            *saw_generation_started = true;
        }
        collect_response_images(&value, images);
    }
}

fn sse_event_payloads(event: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for line in event.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim_start();
            if data != "[DONE]" {
                lines.push(data);
            }
        }
    }
    if lines.is_empty() {
        Vec::new()
    } else {
        vec![lines.join("\n")]
    }
}

#[cfg(test)]
fn extract_images_from_responses_stream(raw: &str) -> Vec<String> {
    let mut images = Vec::new();
    for data in sse_data_payloads(raw) {
        if let Ok(value) = serde_json::from_str::<Value>(&data) {
            collect_response_images(&value, &mut images);
        }
    }
    dedupe_images(images)
}

fn collect_response_images(value: &Value, images: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_response_images(item, images);
            }
        }
        Value::Object(map) => {
            let is_image_call = map
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "image_generation_call" || t.contains("image_generation"));

            if is_image_call {
                for key in ["result", "b64_json", "image_base64"] {
                    if let Some(b64) = map.get(key).and_then(Value::as_str) {
                        images.push(normalize_image_base64(b64));
                    }
                }
            }

            for child in map.values() {
                collect_response_images(child, images);
            }
        }
        _ => {}
    }
}

fn normalize_image_base64(value: &str) -> String {
    if value.starts_with("data:image/") {
        if let Some(comma_pos) = value.find("base64,") {
            return value[comma_pos + 7..].to_string();
        }
    }
    value.to_string()
}

#[cfg(test)]
fn sse_data_payloads(raw: &str) -> Vec<String> {
    let normalized = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut payloads = Vec::new();
    for event in normalized.split("\n\n") {
        let mut lines = Vec::new();
        for line in event.lines() {
            if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim_start();
                if data != "[DONE]" {
                    lines.push(data);
                }
            }
        }
        if !lines.is_empty() {
            payloads.push(lines.join("\n"));
        }
    }
    payloads
}

fn update_stream_state_from_value(state: &mut StreamResponseState, value: &Value) {
    if let Some(event_type) = value["type"].as_str() {
        state.last_event = Some(event_type.to_string());
    }

    if let Some(response) = value.get("response") {
        if let Some(id) = response["id"].as_str() {
            state.response_id = Some(id.to_string());
        }
        if let Some(status) = response["status"].as_str() {
            state.status = Some(status.to_string());
        }
        if let Some(model) = response["model"].as_str() {
            state.model = Some(model.to_string());
        }
    }

    if let Some(id) = value["id"].as_str() {
        if id.starts_with("resp_") {
            state.response_id = Some(id.to_string());
        }
    }
    if let Some(status) = value["status"].as_str() {
        state.status = Some(status.to_string());
    }
    if let Some(model) = value["model"].as_str() {
        state.model = Some(model.to_string());
    }
}

fn format_stream_state_hint(state: &StreamResponseState) -> String {
    let mut parts = Vec::new();
    if let Some(id) = state.response_id.as_deref() {
        parts.push(format!("response_id={id}"));
    }
    if let Some(status) = state.status.as_deref() {
        parts.push(format!("status={status}"));
    }
    if let Some(model) = state.model.as_deref() {
        parts.push(format!("relay_model={model}"));
    }
    if let Some(event) = state.last_event.as_deref() {
        parts.push(format!("last_event={event}"));
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!(" ({})", parts.join(", "))
    }
}

fn dedupe_images(images: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for image in images {
        if !deduped.iter().any(|item| item == &image) {
            deduped.push(image);
        }
    }
    deduped
}

fn endpoint_url(api_base: &str, endpoint: &str) -> String {
    format!("{}/{}", api_base.trim_end_matches('/'), endpoint)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sse_parser_accepts_crlf_events() {
        let raw = concat!(
            "event: response.created\r\n",
            "data: {\"type\":\"response.created\"}\r\n",
            "\r\n",
            "event: response.output_item.done\r\n",
            "data: {\"type\":\"image_generation_call\",\"result\":\"abc\"}\r\n",
            "\r\n",
            "data: [DONE]\r\n",
            "\r\n"
        );

        assert_eq!(extract_images_from_responses_stream(raw), vec!["abc"]);
    }

    #[test]
    fn long_multibyte_http_error_preview_does_not_panic() {
        let body = format!("x{}", "错".repeat(180));
        let result = std::panic::catch_unwind(|| parse_http_error(500, &body));
        assert!(result.is_ok());
    }
}
