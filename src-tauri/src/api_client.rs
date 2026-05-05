use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::time::Duration;

/// API基础URL（可通过配置修改）
pub const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8000/v1";
const GENERATION_TIMEOUT: Duration = Duration::from_secs(360);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const API_CHECK_TIMEOUT: Duration = Duration::from_secs(25);
const USER_AGENT: &str = "SpriteAnimte/0.1";

struct ApiHttpClient {
    label: String,
    client: Client,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCheckResult {
    pub ok: bool,
    pub status: String,
    pub message: String,
    pub endpoint: String,
    pub model: String,
    pub model_found: Option<bool>,
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
        .http1_only()
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
        .http1_only()
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

fn is_retryable_send_error(e: &reqwest::Error) -> bool {
    if e.is_timeout() || e.is_connect() {
        return true;
    }
    let detail = describe_reqwest_error(e).to_ascii_lowercase();
    detail.contains("connection closed before message completed")
        || detail.contains("client error (sendrequest)")
        || detail.contains("connection reset")
        || detail.contains("broken pipe")
        || detail.contains("unexpected eof")
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

fn response_preview(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 300 {
        body.to_string()
    } else {
        let preview: String = body.chars().take(300).collect();
        format!("{preview}...（已截断）")
    }
}

/// ============================================================
/// POST /responses — 固定使用 image_generation 工具的流式请求
/// ============================================================
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

    let url = endpoint_url(api_base, "models");
    let api_client = create_client(if proxy_url.trim().is_empty() {
        None
    } else {
        Some(proxy_url.trim())
    })?;

    eprintln!(
        "[api-check] /models 检测 | endpoint={url} model={} mode={}",
        if model.is_empty() {
            "(未指定)"
        } else {
            model
        },
        api_client.label
    );
    let resp = api_client
        .client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .timeout(API_CHECK_TIMEOUT)
        .send()
        .await
        .map_err(|e| describe_send_error(&e))?;

    let status = resp.status();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(parse_http_error(status.as_u16(), &resp_body));
    }

    let value = serde_json::from_str::<Value>(&resp_body).ok();
    let model_ids = value.as_ref().map(extract_model_ids).unwrap_or_default();
    let model_found = if model.is_empty() || model_ids.is_empty() {
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
        None if model.is_empty() => (
            "ok",
            "连接成功，/models 可访问。未填写模型名，已跳过模型匹配。".into(),
        ),
        None => (
            "ok",
            "基础连接成功，/models 可访问；该服务未返回标准模型列表，已跳过模型名匹配。".into(),
        ),
    };

    Ok(ApiCheckResult {
        ok: true,
        status: status.into(),
        message,
        endpoint: url,
        model: model.into(),
        model_found,
    })
}

pub async fn call_responses_api(
    api_base: &str,
    api_key: &str,
    prompt: &str,
    input_image_data_url: Option<&str>,
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
    eprintln!(
        "[api] /responses 请求模型={model} size={size} count={count} input_image={}",
        input_image_data_url.is_some()
    );

    let body = build_responses_image_generation_body(
        model,
        prompt,
        input_image_data_url,
        count,
        size,
        true,
    );
    let body_bytes =
        serde_json::to_vec(&body).map_err(|e| format!("序列化 Responses 请求失败: {e}"))?;
    eprintln!(
        "[api] /responses 请求体大小约 {:.1} KiB",
        body_bytes.len() as f64 / 1024.0
    );

    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    eprintln!("[api] /responses 尝试 {} (stream/sized)", api_client.label);
    let max_attempts = if input_image_data_url.is_some() { 1 } else { 2 };
    let mut resp = None;
    for attempt in 1..=max_attempts {
        match api_client
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(body_bytes.clone())
            .send()
            .await
        {
            Ok(value) => {
                resp = Some(value);
                break;
            }
            Err(e) => {
                let msg = describe_send_error(&e);
                if attempt < max_attempts && is_retryable_send_error(&e) {
                    eprintln!("[api] /responses 请求失败，将重试一次: {msg}");
                    continue;
                }
                eprintln!("[api] /responses 请求失败: {msg}");
                return Err(msg);
            }
        }
    }
    let resp = resp.ok_or_else(|| "Responses API 请求未返回响应".to_string())?;

    let status = resp.status();
    if !status.is_success() {
        let resp_body = resp.text().await.unwrap_or_default();
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses {msg}");
        return Err(msg);
    }

    read_responses_stream_images(resp, count).await
}

fn build_responses_image_generation_body(
    model: &str,
    prompt: &str,
    input_image_data_url: Option<&str>,
    count: u32,
    size: &str,
    stream: bool,
) -> Value {
    let input = if let Some(image_url) = input_image_data_url.filter(|value| !value.is_empty()) {
        serde_json::json!([
            {
                "role": "user",
                "content": [
                    {"type": "input_text", "text": prompt},
                    {"type": "input_image", "image_url": image_url}
                ]
            }
        ])
    } else {
        Value::String(prompt.to_string())
    };

    serde_json::json!({
        "model": model,
        "input": input,
        "tools": [{"type": "image_generation", "size": size, "n": count}],
        "stream": stream,
    })
}

/// POST /responses — 文本请求，用于提示词优化。
pub async fn call_responses_text_api(
    api_base: &str,
    api_key: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
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
            input_image_data_url,
            model,
            proxy_url,
        )
        .await;
    }

    match call_responses_text_api_once(
        api_base,
        api_key,
        instructions,
        input,
        input_image_data_url,
        model,
        proxy_url,
    )
    .await
    {
        Ok(text) => Ok(text),
        Err(responses_error) if should_try_chat_completions_text_fallback(&responses_error) => {
            eprintln!(
                "[api] /responses 文本接口不可用，尝试 /chat/completions 兜底: {responses_error}"
            );
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
            .map_err(|chat_error| {
                    format!(
                        "/responses 文本接口失败：{responses_error}；/chat/completions 兜底失败：{chat_error}"
                    )
                })
        }
        Err(err) => Err(err),
    }
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
    eprintln!("[api] /responses 文本请求模型={model}");

    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    let first_uses_list_input = input_image_data_url.is_some();
    let string_body = build_responses_text_body(
        model,
        instructions,
        input,
        input_image_data_url,
        first_uses_list_input,
    );
    let first_input_shape = if first_uses_list_input {
        "list+image"
    } else {
        "string"
    };
    match send_responses_text_request(&api_client, &url, api_key, &string_body, first_input_shape)
        .await
    {
        Ok(text) => Ok(text),
        Err(string_error)
            if !first_uses_list_input && should_retry_responses_text_list_input(&string_error) =>
        {
            eprintln!(
                "[api] /responses 文本请求要求 input 列表，使用 list input 重试: {string_error}"
            );
            let list_body =
                build_responses_text_body(model, instructions, input, input_image_data_url, true);
            send_responses_text_request(&api_client, &url, api_key, &list_body, "list")
                .await
                .map_err(|list_error| {
                    format!(
                        "/responses string input 失败：{string_error}；list input 重试失败：{list_error}"
                    )
                })
        }
        Err(err) => Err(err),
    }
}

fn build_responses_text_body(
    model: &str,
    instructions: &str,
    input: &str,
    input_image_data_url: Option<&str>,
    use_list_input: bool,
) -> Value {
    let input_value = if use_list_input || input_image_data_url.is_some() {
        let mut content = vec![serde_json::json!({"type": "input_text", "text": input})];
        if let Some(image_url) = input_image_data_url.filter(|value| !value.trim().is_empty()) {
            content.push(serde_json::json!({"type": "input_image", "image_url": image_url}));
        }
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
    api_client: &ApiHttpClient,
    url: &str,
    api_key: &str,
    body: &Value,
    input_shape: &str,
) -> Result<String, String> {
    eprintln!(
        "[api] /responses 尝试 {} (normal/text input={input_shape})",
        api_client.label
    );
    let resp = api_client
        .client
        .post(url)
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
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses 文本请求 {msg}");
        return Err(msg);
    }

    parse_text_api_response("Responses API", &resp_body, &content_type)
}

fn should_retry_responses_text_list_input(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("input must be a list")
        || lower.contains("input should be a list")
        || lower.contains("expected a list")
        || lower.contains("expected list")
        || lower.contains("invalid type for 'input'")
        || lower.contains("invalid type for input")
}

fn should_try_chat_completions_text_fallback(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("返回了 html 页面")
        || lower.contains("http 404")
        || lower.contains("http 405")
        || lower.contains("http 501")
        || lower.contains("not found")
        || lower.contains("unknown endpoint")
        || lower.contains("unsupported endpoint")
        || lower.contains("unsupported url")
        || lower.contains("responses api 未返回文本内容")
        || lower.contains("解析文本响应失败")
        || lower.contains("http 成功但响应体为空")
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
    eprintln!("[api] /chat/completions 文本请求模型={model}");

    let user_content =
        if let Some(image_url) = input_image_data_url.filter(|value| !value.trim().is_empty()) {
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
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let resp_body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /chat/completions 文本请求 {msg}");
        return Err(msg);
    }

    parse_text_api_response("Chat Completions API", &resp_body, &content_type)
}

fn should_use_chat_completions(api_base: &str, model: &str) -> bool {
    let api_base = api_base.to_ascii_lowercase();
    let model = model.to_ascii_lowercase();
    api_base.contains("api.deepseek.com") || model.starts_with("deepseek")
}

fn parse_text_api_response(
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

fn looks_like_sse_body(value: &str) -> bool {
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
            if looks_like_prompt_optimizer_payload(value) {
                if let Ok(text) = serde_json::to_string(value) {
                    chunks.push(text);
                }
            }
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
            if type_name != "message" {
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

fn extract_model_ids(value: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        collect_model_ids_from_value(&Value::Array(data.clone()), &mut ids);
    } else if let Some(models) = value.get("models").and_then(Value::as_array) {
        collect_model_ids_from_value(&Value::Array(models.clone()), &mut ids);
    } else if let Some(items) = value.as_array() {
        collect_model_ids_from_value(&Value::Array(items.clone()), &mut ids);
    } else {
        collect_model_ids_from_value(value, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_model_ids_from_value(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(id) = item.as_str() {
                    ids.push(id.to_string());
                } else {
                    collect_model_ids_from_value(item, ids);
                }
            }
        }
        Value::Object(map) => {
            for key in ["id", "name", "model", "model_id", "modelId", "model_name"] {
                if let Some(id) = map.get(key).and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
            for child in map.values() {
                collect_model_ids_from_value(child, ids);
            }
        }
        _ => {}
    }
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

    #[test]
    fn text_api_parser_reports_empty_success_body() {
        let err = parse_text_api_response("Responses API", "", "application/json").unwrap_err();
        assert!(err.contains("HTTP 成功但响应体为空"));
    }

    #[test]
    fn text_api_parser_accepts_sse_text_deltas() {
        let raw = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"ok\\\":\"}}]}\n\n",
            "data: {\"choices\":[{\"delta\":{\"content\":\"true}\"}}]}\n\n",
            "data: [DONE]\n\n"
        );

        let text =
            parse_text_api_response("Chat Completions API", raw, "text/event-stream").unwrap();
        assert_eq!(text, "{\"ok\":true}");
    }

    #[test]
    fn text_api_parser_accepts_direct_optimizer_json() {
        let raw = r#"{"prompt":"角色跑步","negative_prompt":"","grid_rows":2,"grid_cols":3}"#;
        let text = parse_text_api_response("Responses API", raw, "application/json").unwrap();
        assert_eq!(text, raw);
    }

    #[test]
    fn text_api_parser_extracts_nested_optimizer_json() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": {
                            "prompt": "角色跑步",
                            "negative_prompt": "",
                            "grid_rows": 2,
                            "grid_cols": 3
                        }
                    }
                }
            ]
        })
        .to_string();

        let text =
            parse_text_api_response("Chat Completions API", &raw, "application/json").unwrap();
        assert!(text.contains("\"prompt\":\"角色跑步\""));
    }

    #[test]
    fn responses_text_body_can_use_list_input_shape() {
        let body = build_responses_text_body("model-a", "system", "hello", None, true);

        assert_eq!(body["model"], "model-a");
        assert_eq!(body["instructions"], "system");
        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][0]["content"][0]["text"], "hello");
    }

    #[test]
    fn responses_text_body_can_include_reference_image() {
        let body = build_responses_text_body(
            "model-a",
            "system",
            "hello",
            Some("data:image/jpeg;base64,abc"),
            false,
        );

        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(
            body["input"][0]["content"][1]["image_url"],
            "data:image/jpeg;base64,abc"
        );
    }

    #[test]
    fn responses_text_retry_detects_input_list_error() {
        assert!(should_retry_responses_text_list_input(
            "HTTP 400: Input must be a list"
        ));
        assert!(should_retry_responses_text_list_input(
            "invalid type for 'input'"
        ));
        assert!(!should_retry_responses_text_list_input(
            "HTTP 401: invalid api key"
        ));
    }

    #[test]
    fn text_api_parser_rejects_html_body_with_preview() {
        let err = parse_text_api_response(
            "Responses API",
            "<html><body>not api</body></html>",
            "text/html",
        )
        .unwrap_err();
        assert!(err.contains("返回了 HTML 页面"));
        assert!(err.contains("缺少 /v1"));
        assert!(err.contains("not api"));
    }

    #[test]
    fn responses_text_errors_indicate_when_chat_fallback_is_useful() {
        assert!(should_try_chat_completions_text_fallback(
            "Responses API 返回了 HTML 页面"
        ));
        assert!(should_try_chat_completions_text_fallback(
            "HTTP 404: not found"
        ));
        assert!(!should_try_chat_completions_text_fallback(
            "HTTP 401: invalid api key"
        ));
    }

    #[test]
    fn image_generation_body_uses_plain_text_input_without_reference_image() {
        let body = build_responses_image_generation_body(
            "model-a",
            "draw a sprite",
            None,
            2,
            "1024x1024",
            true,
        );

        assert_eq!(body["model"], "model-a");
        assert_eq!(body["input"], "draw a sprite");
        assert_eq!(body["tools"][0]["type"], "image_generation");
        assert_eq!(body["tools"][0]["n"], 2);
        assert_eq!(body["tools"][0]["size"], "1024x1024");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn image_generation_body_uses_multimodal_input_with_reference_image() {
        let image = "data:image/png;base64,abc";
        let body = build_responses_image_generation_body(
            "model-a",
            "redraw this icon",
            Some(image),
            1,
            "1024x1024",
            true,
        );

        assert_eq!(body["input"][0]["role"], "user");
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][0]["content"][0]["text"], "redraw this icon");
        assert_eq!(body["input"][0]["content"][1]["type"], "input_image");
        assert_eq!(body["input"][0]["content"][1]["image_url"], image);
        assert_eq!(body["tools"][0]["type"], "image_generation");
    }

    #[test]
    fn model_id_extractor_accepts_openai_models_shape() {
        let value = serde_json::json!({
            "object": "list",
            "data": [
                {"id": "model-a", "object": "model"},
                {"id": "model-b", "object": "model"}
            ]
        });

        assert_eq!(extract_model_ids(&value), vec!["model-a", "model-b"]);
    }

    #[test]
    fn model_id_extractor_accepts_string_model_arrays() {
        let value = serde_json::json!({
            "models": ["model-b", "model-a", "model-a"]
        });

        assert_eq!(extract_model_ids(&value), vec!["model-a", "model-b"]);
    }

    #[test]
    fn model_id_extractor_accepts_nested_nonstandard_shapes() {
        let value = serde_json::json!({
            "result": {
                "items": [
                    {"model": "model-c"},
                    {"model_name": "model-a"},
                    {"config": {"model_id": "model-b"}}
                ]
            }
        });

        assert_eq!(
            extract_model_ids(&value),
            vec!["model-a", "model-b", "model-c"]
        );
    }
}
