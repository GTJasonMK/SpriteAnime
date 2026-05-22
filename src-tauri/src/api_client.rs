use base64::Engine;
use reqwest::{multipart, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::time::{Duration, Instant};

/// API基础URL（可通过配置修改）
pub const DEFAULT_API_BASE_URL: &str = "http://127.0.0.1:8000/v1";
const GENERATION_TIMEOUT: Duration = Duration::from_secs(360);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const API_CHECK_TIMEOUT: Duration = Duration::from_secs(25);
const VIDEO_POLL_INTERVAL: Duration = Duration::from_secs(3);
const VIDEO_POLL_TIMEOUT: Duration = Duration::from_secs(900);
const USER_AGENT: &str = "SpriteAnimte/0.1";

struct ApiHttpClient {
    label: String,
    client: Client,
}

struct ApiResponseBody {
    content_type: String,
    body: String,
}

struct VideoJobInfo {
    id: String,
    status: String,
    error_message: Option<String>,
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

async fn send_authenticated_json_bytes(
    api_client: &ApiHttpClient,
    url: &str,
    api_key: &str,
    body: &[u8],
    log_label: &str,
    max_attempts: usize,
) -> Result<reqwest::Response, String> {
    for attempt in 1..=max_attempts.max(1) {
        match api_client
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .body(body.to_vec())
            .send()
            .await
        {
            Ok(resp) => return Ok(resp),
            Err(e) => {
                let msg = describe_send_error(&e);
                if attempt < max_attempts && is_retryable_send_error(&e) {
                    eprintln!("[api] {log_label} 请求失败，将重试一次: {msg}");
                    continue;
                }
                eprintln!("[api] {log_label} 请求失败: {msg}");
                return Err(msg);
            }
        }
    }

    Err(format!("{log_label} 请求未返回响应"))
}

async fn post_authenticated_json(
    api_client: &ApiHttpClient,
    url: &str,
    api_key: &str,
    body: &Value,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = api_client
        .client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] {log_label} 请求失败: {msg}");
            msg
        })?;

    read_api_response_body(resp, log_label).await
}

async fn post_authenticated_multipart(
    api_client: &ApiHttpClient,
    url: &str,
    api_key: &str,
    form: multipart::Form,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = api_client
        .client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] {log_label} 请求失败: {msg}");
            msg
        })?;

    read_api_response_body(resp, log_label).await
}

async fn get_authenticated(
    api_client: &ApiHttpClient,
    url: &str,
    api_key: &str,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = api_client
        .client
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] {log_label} 请求失败: {msg}");
            msg
        })?;

    read_api_response_body(resp, log_label).await
}

async fn read_api_response_body(
    resp: reqwest::Response,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let status = resp.status();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &body);
        eprintln!("[api] {log_label} {msg}");
        return Err(msg);
    }

    Ok(ApiResponseBody { content_type, body })
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
    let resp = send_authenticated_json_bytes(
        &api_client,
        &url,
        api_key,
        &body_bytes,
        "/responses",
        max_attempts,
    )
    .await?;

    let status = resp.status();
    if !status.is_success() {
        let resp_body = resp.text().await.unwrap_or_default();
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses {msg}");
        return Err(msg);
    }

    read_responses_stream_images(resp, count).await
}

pub async fn call_chat_completions_image_api(
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

    let url = endpoint_url(api_base, "chat/completions");
    eprintln!(
        "[api] /chat/completions 生图请求模型={model} size={size} count={count} input_image={}",
        input_image_data_url.is_some()
    );

    let stream_body = build_chat_completions_image_generation_body(
        model,
        prompt,
        input_image_data_url,
        count,
        size,
        true,
    );
    let api_client = create_client(if proxy_url.is_empty() {
        None
    } else {
        Some(proxy_url)
    })?;

    eprintln!(
        "[api] /chat/completions 尝试 {} (image/stream)",
        api_client.label
    );
    let response = match post_authenticated_json(
        &api_client,
        &url,
        api_key,
        &stream_body,
        "/chat/completions 生图请求",
    )
    .await
    {
        Ok(response) => response,
        Err(err) if should_retry_chat_image_without_generation_options(&err) => {
            eprintln!(
                "[api] /chat/completions 生图请求不接受 n/size 参数，使用最小流式请求体重试: {err}"
            );
            let minimal_body = build_minimal_chat_completions_image_generation_body(
                model,
                prompt,
                input_image_data_url,
                true,
            );
            match post_authenticated_json(
                &api_client,
                &url,
                api_key,
                &minimal_body,
                "/chat/completions 生图请求",
            )
            .await
            {
                Ok(response) => response,
                Err(minimal_err) if should_retry_chat_image_without_stream(&minimal_err) => {
                    eprintln!(
                        "[api] /chat/completions 生图请求不支持 stream，使用最小非流式请求体重试: {minimal_err}"
                    );
                    let minimal_non_stream_body =
                        build_minimal_chat_completions_image_generation_body(
                            model,
                            prompt,
                            input_image_data_url,
                            false,
                        );
                    post_authenticated_json(
                        &api_client,
                        &url,
                        api_key,
                        &minimal_non_stream_body,
                        "/chat/completions 生图请求",
                    )
                    .await
                    .map_err(|non_stream_err| {
                        format!(
                            "/chat/completions 生图请求失败：{err}；最小流式重试失败：{minimal_err}；最小非流式重试失败：{non_stream_err}"
                        )
                    })?
                }
                Err(minimal_err) => {
                    return Err(format!(
                        "/chat/completions 生图请求失败：{err}；最小流式请求体重试失败：{minimal_err}"
                    ));
                }
            }
        }
        Err(err) if should_retry_chat_image_without_stream(&err) => {
            eprintln!("[api] /chat/completions 生图请求不支持 stream，使用非流式请求体重试: {err}");
            let non_stream_body = build_chat_completions_image_generation_body(
                model,
                prompt,
                input_image_data_url,
                count,
                size,
                false,
            );
            match post_authenticated_json(
                &api_client,
                &url,
                api_key,
                &non_stream_body,
                "/chat/completions 生图请求",
            )
            .await
            {
                Ok(response) => response,
                Err(non_stream_err)
                    if should_retry_chat_image_without_generation_options(&non_stream_err) =>
                {
                    eprintln!(
                        "[api] /chat/completions 生图请求不接受 n/size 参数，使用最小非流式请求体重试: {non_stream_err}"
                    );
                    let minimal_non_stream_body =
                        build_minimal_chat_completions_image_generation_body(
                            model,
                            prompt,
                            input_image_data_url,
                            false,
                        );
                    post_authenticated_json(
                        &api_client,
                        &url,
                        api_key,
                        &minimal_non_stream_body,
                        "/chat/completions 生图请求",
                    )
                    .await
                    .map_err(|minimal_err| {
                        format!(
                            "/chat/completions 生图请求失败：{err}；非流式重试失败：{non_stream_err}；最小非流式重试失败：{minimal_err}"
                        )
                    })?
                }
                Err(non_stream_err) => {
                    return Err(format!(
                        "/chat/completions 流式生图请求失败：{err}；非流式重试失败：{non_stream_err}"
                    ));
                }
            }
        }
        Err(err) => return Err(err),
    };

    let image_refs = extract_images_from_chat_completions_response(&response.body);
    if image_refs.is_empty() {
        return Err(format!(
            "Chat Completions API 未返回图片内容；响应预览: {}",
            response_preview(&response.body)
        ));
    }

    let mut images = Vec::new();
    for image_ref in image_refs.into_iter().take(count as usize) {
        let image =
            materialize_image_reference_as_base64(&api_client, api_base, api_key, &image_ref)
                .await?;
        if !image.trim().is_empty() {
            images.push(image);
        }
    }

    let images = dedupe_images(images);
    if images.is_empty() {
        return Err("Chat Completions API 返回了图片引用，但内容为空".into());
    }
    Ok(images)
}

pub async fn call_chat_completions_video_api(
    api_base: &str,
    api_key: &str,
    prompt: &str,
    model: &str,
    size: &str,
    seconds: &str,
    proxy_url: &str,
) -> Result<Vec<u8>, String> {
    let api_base = api_base.trim();
    let api_key = api_key.trim();
    let prompt = prompt.trim();
    let model = model.trim();
    let size = size.trim();
    let seconds = seconds.trim();

    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if api_base.is_empty() {
        return Err("API 地址为空".into());
    }
    if prompt.is_empty() {
        return Err("视频提示词为空".into());
    }
    if model.is_empty() {
        return Err("视频模型为空".into());
    }

    let url = endpoint_url(api_base, "chat/completions");
    let api_client = create_client(if proxy_url.trim().is_empty() {
        None
    } else {
        Some(proxy_url.trim())
    })?;
    let stream_body =
        build_chat_completions_video_generation_body(model, prompt, size, seconds, true, true);

    eprintln!(
        "[api] /chat/completions 视频请求 | 模型={model} size={size} seconds={seconds} mode={} stream=true",
        api_client.label
    );
    let response = match post_authenticated_json(
        &api_client,
        &url,
        api_key,
        &stream_body,
        "/chat/completions 视频请求",
    )
    .await
    {
        Ok(response) => response,
        Err(err) if should_retry_chat_video_without_generation_options(&err) => {
            eprintln!(
                "[api] /chat/completions 视频请求不接受 size/seconds 参数，使用最小流式请求体重试: {err}"
            );
            let minimal_body = build_chat_completions_video_generation_body(
                model, prompt, size, seconds, true, false,
            );
            match post_authenticated_json(
                &api_client,
                &url,
                api_key,
                &minimal_body,
                "/chat/completions 视频请求",
            )
            .await
            {
                Ok(response) => response,
                Err(minimal_err) if should_retry_chat_image_without_stream(&minimal_err) => {
                    eprintln!(
                        "[api] /chat/completions 视频请求不支持 stream，使用最小非流式请求体重试: {minimal_err}"
                    );
                    let minimal_non_stream_body = build_chat_completions_video_generation_body(
                        model, prompt, size, seconds, false, false,
                    );
                    post_authenticated_json(
                        &api_client,
                        &url,
                        api_key,
                        &minimal_non_stream_body,
                        "/chat/completions 视频请求",
                    )
                    .await
                    .map_err(|non_stream_err| {
                        format!(
                            "/chat/completions 视频请求失败：{err}；最小流式重试失败：{minimal_err}；最小非流式重试失败：{non_stream_err}"
                        )
                    })?
                }
                Err(minimal_err) => {
                    return Err(format!(
                        "/chat/completions 视频请求失败：{err}；最小流式请求体重试失败：{minimal_err}"
                    ));
                }
            }
        }
        Err(err) if should_retry_chat_image_without_stream(&err) => {
            eprintln!("[api] /chat/completions 视频请求不支持 stream，使用非流式请求体重试: {err}");
            let non_stream_body = build_chat_completions_video_generation_body(
                model, prompt, size, seconds, false, true,
            );
            match post_authenticated_json(
                &api_client,
                &url,
                api_key,
                &non_stream_body,
                "/chat/completions 视频请求",
            )
            .await
            {
                Ok(response) => response,
                Err(non_stream_err)
                    if should_retry_chat_video_without_generation_options(&non_stream_err) =>
                {
                    eprintln!(
                        "[api] /chat/completions 视频请求不接受 size/seconds 参数，使用最小非流式请求体重试: {non_stream_err}"
                    );
                    let minimal_non_stream_body = build_chat_completions_video_generation_body(
                        model, prompt, size, seconds, false, false,
                    );
                    post_authenticated_json(
                        &api_client,
                        &url,
                        api_key,
                        &minimal_non_stream_body,
                        "/chat/completions 视频请求",
                    )
                    .await
                    .map_err(|minimal_err| {
                        format!(
                            "/chat/completions 视频请求失败：{err}；非流式重试失败：{non_stream_err}；最小非流式重试失败：{minimal_err}"
                        )
                    })?
                }
                Err(non_stream_err) => {
                    return Err(format!(
                        "/chat/completions 流式视频请求失败：{err}；非流式重试失败：{non_stream_err}"
                    ));
                }
            }
        }
        Err(err) => return Err(err),
    };

    let video_refs = extract_videos_from_chat_completions_response(&response.body);
    if video_refs.is_empty() {
        return Err(format!(
            "Chat Completions API 未返回视频内容；响应预览: {}",
            response_preview(&response.body)
        ));
    }

    materialize_video_reference_as_bytes(&api_client, api_base, api_key, &video_refs[0]).await
}

pub async fn call_videos_api(
    api_base: &str,
    api_key: &str,
    prompt: &str,
    model: &str,
    size: &str,
    seconds: &str,
    proxy_url: &str,
) -> Result<Vec<u8>, String> {
    let api_base = api_base.trim();
    let api_key = api_key.trim();
    let prompt = prompt.trim();
    let model = model.trim();
    let size = size.trim();
    let seconds = seconds.trim();

    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if api_base.is_empty() {
        return Err("API 地址为空".into());
    }
    if prompt.is_empty() {
        return Err("视频提示词为空".into());
    }
    if model.is_empty() {
        return Err("视频模型为空".into());
    }

    let url = endpoint_url(api_base, "videos");
    let api_client = create_client(if proxy_url.trim().is_empty() {
        None
    } else {
        Some(proxy_url.trim())
    })?;
    let form = build_videos_generation_form(model, prompt, size, seconds);
    let json_body = build_videos_generation_body(model, prompt, size, seconds);

    eprintln!(
        "[api] /videos 视频请求 | 模型={model} size={size} seconds={seconds} mode={}",
        api_client.label
    );
    let created =
        match post_authenticated_multipart(&api_client, &url, api_key, form, "/videos 视频请求")
            .await
        {
            Ok(response) => response,
            Err(multipart_err) if should_retry_videos_with_json(&multipart_err) => {
                eprintln!(
                    "[api] /videos multipart 请求失败，使用 JSON 请求体重试: {multipart_err}"
                );
                post_authenticated_json(&api_client, &url, api_key, &json_body, "/videos 视频请求")
                    .await
                    .map_err(|json_err| {
                        format!(
                        "/videos multipart 请求失败：{multipart_err}；JSON 重试失败：{json_err}"
                    )
                    })?
            }
            Err(err) => return Err(err),
        };

    let direct_refs = extract_videos_from_video_api_response(&created.body);
    if let Some(video_ref) = direct_refs.first() {
        return materialize_video_reference_as_bytes(&api_client, api_base, api_key, video_ref)
            .await;
    }

    let mut job = parse_video_job_info(&created.body).ok_or_else(|| {
        format!(
            "Videos API 未返回视频任务 ID；响应预览: {}",
            response_preview(&created.body)
        )
    })?;
    eprintln!(
        "[api] /videos 已创建任务 id={} status={}",
        job.id, job.status
    );

    let started = Instant::now();
    loop {
        if is_video_job_completed(&job.status) {
            return download_video_content_by_id(&api_client, api_base, api_key, &job.id).await;
        }
        if is_video_job_failed(&job.status) {
            return Err(format!(
                "Videos API 任务失败: id={} status={}{}",
                job.id,
                job.status,
                job.error_message
                    .as_deref()
                    .map(|message| format!(" message={message}"))
                    .unwrap_or_default()
            ));
        }
        if started.elapsed() > VIDEO_POLL_TIMEOUT {
            return Err(format!(
                "Videos API 任务超时: id={} status={}，已等待 {} 秒",
                job.id,
                job.status,
                VIDEO_POLL_TIMEOUT.as_secs()
            ));
        }

        tokio::time::sleep(VIDEO_POLL_INTERVAL).await;
        let status_url = endpoint_url(api_base, &format!("videos/{}", job.id));
        let status_response =
            get_authenticated(&api_client, &status_url, api_key, "/videos 任务状态").await?;
        let direct_refs = extract_videos_from_video_api_response(&status_response.body);
        if let Some(video_ref) = direct_refs.first() {
            return materialize_video_reference_as_bytes(&api_client, api_base, api_key, video_ref)
                .await;
        }
        job = parse_video_job_info(&status_response.body).ok_or_else(|| {
            format!(
                "Videos API 状态响应无法解析；响应预览: {}",
                response_preview(&status_response.body)
            )
        })?;
        eprintln!("[api] /videos 任务状态 id={} status={}", job.id, job.status);
    }
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

fn build_chat_completions_image_generation_body(
    model: &str,
    prompt: &str,
    input_image_data_url: Option<&str>,
    count: u32,
    size: &str,
    stream: bool,
) -> Value {
    let user_content =
        if let Some(image_url) = input_image_data_url.filter(|value| !value.trim().is_empty()) {
            serde_json::json!([
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": image_url}}
            ])
        } else {
            Value::String(prompt.to_string())
        };

    let mut body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": user_content}
        ],
        "n": count,
        "size": size,
    });
    if stream {
        if let Some(map) = body.as_object_mut() {
            map.insert("stream".into(), Value::Bool(true));
        }
    }
    body
}

fn build_minimal_chat_completions_image_generation_body(
    model: &str,
    prompt: &str,
    input_image_data_url: Option<&str>,
    stream: bool,
) -> Value {
    let user_content =
        if let Some(image_url) = input_image_data_url.filter(|value| !value.trim().is_empty()) {
            serde_json::json!([
                {"type": "text", "text": prompt},
                {"type": "image_url", "image_url": {"url": image_url}}
            ])
        } else {
            Value::String(prompt.to_string())
        };

    let mut body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": user_content}
        ],
    });
    if stream {
        if let Some(map) = body.as_object_mut() {
            map.insert("stream".into(), Value::Bool(true));
        }
    }
    body
}

fn build_chat_completions_video_generation_body(
    model: &str,
    prompt: &str,
    size: &str,
    seconds: &str,
    stream: bool,
    include_generation_options: bool,
) -> Value {
    let mut body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
    });
    if let Some(map) = body.as_object_mut() {
        if stream {
            map.insert("stream".into(), Value::Bool(true));
        }
        if include_generation_options {
            map.insert("size".into(), Value::String(size.to_string()));
            map.insert("seconds".into(), Value::String(seconds.to_string()));
        }
    }
    body
}

fn build_videos_generation_form(
    model: &str,
    prompt: &str,
    size: &str,
    seconds: &str,
) -> multipart::Form {
    multipart::Form::new()
        .text("model", model.to_string())
        .text("prompt", prompt.to_string())
        .text("size", size.to_string())
        .text("seconds", seconds.to_string())
}

fn build_videos_generation_body(model: &str, prompt: &str, size: &str, seconds: &str) -> Value {
    serde_json::json!({
        "model": model,
        "prompt": prompt,
        "size": size,
        "seconds": seconds,
    })
}

fn should_retry_chat_image_without_generation_options(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    (lower.contains("unknown parameter")
        || lower.contains("unrecognized request argument")
        || lower.contains("unsupported parameter")
        || lower.contains("extra fields")
        || lower.contains("not permitted")
        || lower.contains("invalid field"))
        && (lower.contains("size") || lower.contains("\"n\"") || lower.contains("'n'"))
}

fn should_retry_chat_image_without_stream(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    (lower.contains("unknown parameter")
        || lower.contains("unrecognized request argument")
        || lower.contains("unsupported parameter")
        || lower.contains("extra fields")
        || lower.contains("not permitted")
        || lower.contains("invalid field")
        || lower.contains("streaming is not supported")
        || lower.contains("stream is not supported"))
        && lower.contains("stream")
}

fn should_retry_chat_video_without_generation_options(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    (lower.contains("unknown parameter")
        || lower.contains("unrecognized request argument")
        || lower.contains("unsupported parameter")
        || lower.contains("extra fields")
        || lower.contains("not permitted")
        || lower.contains("invalid field"))
        && (lower.contains("size")
            || lower.contains("seconds")
            || lower.contains("duration")
            || lower.contains("video"))
}

fn should_retry_videos_with_json(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("multipart")
        || lower.contains("content-type")
        || lower.contains("content type")
        || lower.contains("form")
        || lower.contains("json")
        || lower.contains("invalid request body")
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
    let response =
        post_authenticated_json(api_client, url, api_key, body, "/responses 文本请求").await?;

    parse_text_api_response("Responses API", &response.body, &response.content_type)
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

fn normalize_video_base64(value: &str) -> String {
    if value.starts_with("data:video/") {
        if let Some(comma_pos) = value.find("base64,") {
            return value[comma_pos + 7..].to_string();
        }
    }
    value.to_string()
}

fn extract_images_from_chat_completions_response(body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut images = Vec::new();
    if looks_like_sse_body(trimmed) {
        for payload in sse_data_payloads(trimmed) {
            if let Ok(value) = serde_json::from_str::<Value>(&payload) {
                collect_chat_completion_images(&value, &mut images);
            } else {
                collect_image_refs_from_text(&payload, &mut images);
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_images(&value, &mut images);
    } else {
        collect_image_refs_from_text(trimmed, &mut images);
    }

    dedupe_images(images)
}

fn extract_videos_from_chat_completions_response(body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut videos = Vec::new();
    if looks_like_sse_body(trimmed) {
        for payload in sse_data_payloads(trimmed) {
            if let Ok(value) = serde_json::from_str::<Value>(&payload) {
                collect_chat_completion_videos(&value, &mut videos);
            } else {
                collect_video_refs_from_text(&payload, &mut videos);
            }
        }
    } else if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_videos(&value, &mut videos);
    } else {
        collect_video_refs_from_text(trimmed, &mut videos);
    }

    dedupe_images(videos)
}

fn extract_videos_from_video_api_response(body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut videos = Vec::new();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_videos(&value, &mut videos);
        for key in ["url", "download_url", "output_url", "content_url"] {
            if let Some(url) = value.get(key).and_then(Value::as_str) {
                if looks_like_video_ref(url) || looks_like_http_url(url) {
                    videos.push(normalize_video_base64(url));
                }
            }
        }
    } else {
        collect_video_refs_from_text(trimmed, &mut videos);
    }

    dedupe_images(videos)
}

fn parse_video_job_info(body: &str) -> Option<VideoJobInfo> {
    let value = serde_json::from_str::<Value>(body.trim()).ok()?;
    let id = find_string_for_keys(&value, &["id", "video_id", "videoId"])?;
    let status = find_string_for_keys(&value, &["status", "state"])
        .unwrap_or_else(|| "completed".to_string());
    let error_message = find_string_for_keys(
        &value,
        &[
            "message",
            "error_message",
            "failure_reason",
            "failed_reason",
        ],
    )
    .or_else(|| {
        value
            .get("error")
            .and_then(|error| find_string_for_keys(error, &["message", "code"]))
    });

    Some(VideoJobInfo {
        id,
        status,
        error_message,
    })
}

fn find_string_for_keys(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::Object(map) => {
            for key in keys {
                if let Some(found) = map.get(*key).and_then(Value::as_str) {
                    let trimmed = found.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
            for key in ["data", "result", "video", "output"] {
                if let Some(child) = map.get(key) {
                    if let Some(found) = find_string_for_keys(child, keys) {
                        return Some(found);
                    }
                }
            }
            None
        }
        Value::Array(items) => items
            .iter()
            .find_map(|item| find_string_for_keys(item, keys)),
        _ => None,
    }
}

fn is_video_job_completed(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "completed" | "complete" | "succeeded" | "success" | "done" | "finished"
    )
}

fn is_video_job_failed(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "failed" | "failure" | "cancelled" | "canceled" | "expired" | "error"
    )
}

fn collect_chat_completion_images(value: &Value, images: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_chat_completion_images(item, images);
            }
        }
        Value::Object(map) => {
            let type_name = map.get("type").and_then(Value::as_str).unwrap_or_default();

            for key in ["b64_json", "image_base64", "base64"] {
                if let Some(image) = map.get(key).and_then(Value::as_str) {
                    images.push(normalize_image_base64(image));
                }
            }

            if type_name == "image_url" {
                if let Some(image_url) = map.get("image_url") {
                    collect_image_url_value(image_url, images);
                }
                if let Some(url) = map.get("url").and_then(Value::as_str) {
                    images.push(normalize_image_base64(url));
                }
            }

            if let Some(image_url) = map.get("image_url") {
                collect_image_url_value(image_url, images);
            }
            if let Some(image) = map.get("image") {
                collect_image_url_value(image, images);
            }

            if let Some(content) = map.get("content") {
                match content {
                    Value::String(text) => collect_image_refs_from_text(text, images),
                    _ => collect_chat_completion_images(content, images),
                }
            }

            for key in ["data", "output", "result", "choices", "message", "delta"] {
                if let Some(child) = map.get(key) {
                    collect_chat_completion_images(child, images);
                }
            }
        }
        Value::String(text) => collect_image_refs_from_text(text, images),
        _ => {}
    }
}

fn collect_chat_completion_videos(value: &Value, videos: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_chat_completion_videos(item, videos);
            }
        }
        Value::Object(map) => {
            let type_name = map.get("type").and_then(Value::as_str).unwrap_or_default();
            let is_video_context = type_name.contains("video")
                || map.contains_key("video")
                || map.contains_key("videos")
                || map.contains_key("video_url")
                || map.contains_key("videoUrl")
                || map.contains_key("video_base64");

            for key in ["b64_json", "video_base64", "base64"] {
                if let Some(video) = map.get(key).and_then(Value::as_str) {
                    videos.push(normalize_video_base64(video));
                }
            }

            for key in [
                "video_url",
                "videoUrl",
                "download_url",
                "downloadUrl",
                "file_url",
                "fileUrl",
                "output_url",
                "outputUrl",
                "content_url",
                "contentUrl",
            ] {
                if let Some(child) = map.get(key) {
                    collect_video_url_value(child, videos);
                }
            }

            if is_video_context {
                if let Some(url) = map.get("url").and_then(Value::as_str) {
                    if looks_like_video_ref(url) || looks_like_http_url(url) {
                        videos.push(normalize_video_base64(url));
                    }
                }
            }

            if let Some(content) = map.get("content") {
                match content {
                    Value::String(text) => collect_video_refs_from_text(text, videos),
                    _ => collect_chat_completion_videos(content, videos),
                }
            }

            for key in [
                "data", "output", "result", "choices", "message", "delta", "video", "videos",
                "file", "files",
            ] {
                if let Some(child) = map.get(key) {
                    collect_chat_completion_videos(child, videos);
                }
            }
        }
        Value::String(text) => collect_video_refs_from_text(text, videos),
        _ => {}
    }
}

fn collect_image_url_value(value: &Value, images: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if looks_like_image_ref(text) {
                images.push(normalize_image_base64(text));
            } else {
                collect_image_refs_from_text(text, images);
            }
        }
        Value::Object(map) => {
            if let Some(url) = map.get("url").and_then(Value::as_str) {
                images.push(normalize_image_base64(url));
            }
            if let Some(b64) = map.get("b64_json").and_then(Value::as_str) {
                images.push(normalize_image_base64(b64));
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_image_url_value(item, images);
            }
        }
        _ => {}
    }
}

fn collect_video_url_value(value: &Value, videos: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            if looks_like_video_ref(text) {
                videos.push(normalize_video_base64(text));
            } else {
                collect_video_refs_from_text(text, videos);
            }
        }
        Value::Object(map) => {
            for key in ["url", "download_url", "downloadUrl"] {
                if let Some(url) = map.get(key).and_then(Value::as_str) {
                    videos.push(normalize_video_base64(url));
                }
            }
            for key in ["b64_json", "video_base64", "base64"] {
                if let Some(b64) = map.get(key).and_then(Value::as_str) {
                    videos.push(normalize_video_base64(b64));
                }
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_video_url_value(item, videos);
            }
        }
        _ => {}
    }
}

fn collect_image_refs_from_text(text: &str, images: &mut Vec<String>) {
    let trimmed = trim_markdown_code_fence(text.trim());
    if trimmed.is_empty() {
        return;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_images(&value, images);
        return;
    }

    if looks_like_image_ref(trimmed) {
        images.push(normalize_image_base64(trimmed));
    }

    for data_url in extract_data_image_urls(trimmed) {
        images.push(normalize_image_base64(&data_url));
    }
    for url in extract_http_urls(trimmed) {
        if looks_like_image_url_or_signed_url(&url, trimmed) {
            images.push(url);
        }
    }
}

fn collect_video_refs_from_text(text: &str, videos: &mut Vec<String>) {
    let trimmed = trim_markdown_code_fence(text.trim());
    if trimmed.is_empty() {
        return;
    }

    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_videos(&value, videos);
        return;
    }

    if looks_like_video_ref(trimmed) {
        videos.push(normalize_video_base64(trimmed));
    }

    for data_url in extract_data_video_urls(trimmed) {
        videos.push(normalize_video_base64(&data_url));
    }
    for url in extract_http_urls(trimmed) {
        if looks_like_video_url_or_signed_url(&url, trimmed) {
            videos.push(url);
        }
    }
}

fn trim_markdown_code_fence(value: &str) -> &str {
    let value = value.trim();
    if !value.starts_with("```") {
        return value;
    }
    let Some(first_newline) = value.find('\n') else {
        return value;
    };
    let body = &value[first_newline + 1..];
    body.trim_end_matches("```").trim()
}

fn extract_data_image_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("data:image/") {
        let candidate = &rest[start..];
        let end = candidate
            .find(|ch: char| {
                ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ')' || ch == ']'
            })
            .unwrap_or(candidate.len());
        urls.push(
            candidate[..end]
                .trim_end_matches(|ch| matches!(ch, '.' | ',' | ';'))
                .to_string(),
        );
        rest = &candidate[end..];
    }
    urls
}

fn extract_data_video_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find("data:video/") {
        let candidate = &rest[start..];
        let end = candidate
            .find(|ch: char| {
                ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ')' || ch == ']'
            })
            .unwrap_or(candidate.len());
        urls.push(
            candidate[..end]
                .trim_end_matches(|ch| matches!(ch, '.' | ',' | ';'))
                .to_string(),
        );
        rest = &candidate[end..];
    }
    urls
}

fn extract_http_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for marker in ["http://", "https://"] {
        let mut rest = value;
        while let Some(start) = rest.find(marker) {
            let candidate = &rest[start..];
            let end = candidate
                .find(|ch: char| {
                    ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ')' || ch == ']'
                })
                .unwrap_or(candidate.len());
            urls.push(
                candidate[..end]
                    .trim_end_matches(|ch| matches!(ch, '.' | ',' | ';' | ':'))
                    .to_string(),
            );
            rest = &candidate[end..];
        }
    }
    urls
}

fn looks_like_image_ref(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("data:image/")
        || looks_like_http_url(trimmed)
        || looks_like_base64_image_payload(trimmed)
}

fn looks_like_video_ref(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("data:video/")
        || looks_like_video_url(trimmed)
        || looks_like_base64_video_payload(trimmed)
}

fn looks_like_http_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn looks_like_video_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    looks_like_http_url(&lower)
        && (lower.contains(".mp4")
            || lower.contains(".webm")
            || lower.contains(".mov")
            || lower.contains(".m4v")
            || lower.contains(".mpeg")
            || lower.contains(".mpg"))
}

fn looks_like_image_url_or_signed_url(url: &str, surrounding_text: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png")
        || lower.contains(".jpg")
        || lower.contains(".jpeg")
        || lower.contains(".webp")
        || lower.contains(".gif")
    {
        return true;
    }
    let trimmed = surrounding_text.trim();
    trimmed == url || trimmed.contains("image_url") || trimmed.contains("b64_json")
}

fn looks_like_video_url_or_signed_url(url: &str, surrounding_text: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".mp4")
        || lower.contains(".webm")
        || lower.contains(".mov")
        || lower.contains(".m4v")
        || lower.contains(".mpeg")
        || lower.contains(".mpg")
    {
        return true;
    }
    let trimmed = surrounding_text.trim();
    trimmed == url
        || trimmed.contains("video_url")
        || trimmed.contains("videoUrl")
        || trimmed.contains("download_url")
        || trimmed.contains("downloadUrl")
}

fn looks_like_base64_image_payload(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 128
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn looks_like_base64_video_payload(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 512
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

async fn materialize_image_reference_as_base64(
    api_client: &ApiHttpClient,
    api_base: &str,
    api_key: &str,
    image_ref: &str,
) -> Result<String, String> {
    let image_ref = image_ref.trim();
    if image_ref.is_empty() {
        return Ok(String::new());
    }
    if looks_like_http_url(image_ref) {
        return download_image_url_as_base64(api_client, api_base, api_key, image_ref).await;
    }
    Ok(normalize_image_base64(image_ref))
}

async fn materialize_video_reference_as_bytes(
    api_client: &ApiHttpClient,
    api_base: &str,
    api_key: &str,
    video_ref: &str,
) -> Result<Vec<u8>, String> {
    let video_ref = video_ref.trim();
    if video_ref.is_empty() {
        return Err("Chat Completions API 返回了空视频引用".into());
    }
    if looks_like_http_url(video_ref) {
        return download_video_url_as_bytes(api_client, api_base, api_key, video_ref).await;
    }

    let base64_payload = normalize_video_base64(video_ref);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(base64_payload.trim())
        .map_err(|e| format!("解析视频 base64 失败: {e}"))?;
    if bytes.is_empty() {
        return Err("Chat Completions API 返回的视频内容为空".into());
    }
    Ok(bytes)
}

async fn download_image_url_as_base64(
    api_client: &ApiHttpClient,
    api_base: &str,
    api_key: &str,
    url: &str,
) -> Result<String, String> {
    eprintln!("[api] 下载 /chat/completions 返回的图片 URL");
    let mut request = api_client.client.get(url).header("Accept", "image/*,*/*");
    if should_authenticate_image_download(api_base, url) {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }
    let resp = request.send().await.map_err(|e| {
        let msg = describe_send_error(&e);
        eprintln!("[api] 下载图片 URL 失败: {msg}");
        msg
    })?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(parse_http_error(status.as_u16(), &body));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取图片 URL 内容失败: {}", describe_send_error(&e)))?;
    if bytes.is_empty() {
        return Err("图片 URL 下载成功但内容为空".into());
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

async fn download_video_url_as_bytes(
    api_client: &ApiHttpClient,
    api_base: &str,
    api_key: &str,
    url: &str,
) -> Result<Vec<u8>, String> {
    eprintln!("[api] 下载 /chat/completions 返回的视频 URL");
    let mut request = api_client.client.get(url).header("Accept", "video/*,*/*");
    if should_authenticate_image_download(api_base, url) {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }
    let resp = request.send().await.map_err(|e| {
        let msg = describe_send_error(&e);
        eprintln!("[api] 下载视频 URL 失败: {msg}");
        msg
    })?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(parse_http_error(status.as_u16(), &body));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取视频 URL 内容失败: {}", describe_send_error(&e)))?;
    if bytes.is_empty() {
        return Err("视频 URL 下载成功但内容为空".into());
    }
    Ok(bytes.to_vec())
}

async fn download_video_content_by_id(
    api_client: &ApiHttpClient,
    api_base: &str,
    api_key: &str,
    video_id: &str,
) -> Result<Vec<u8>, String> {
    let url = endpoint_url(api_base, &format!("videos/{video_id}/content"));
    eprintln!("[api] 下载 /videos 生成内容 id={video_id}");
    let resp = api_client
        .client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "video/*,*/*")
        .send()
        .await
        .map_err(|e| {
            let msg = describe_send_error(&e);
            eprintln!("[api] 下载 /videos 内容失败: {msg}");
            msg
        })?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(parse_http_error(status.as_u16(), &body));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("读取 /videos 内容失败: {}", describe_send_error(&e)))?;
    if bytes.is_empty() {
        return Err("Videos API 内容下载成功但为空".into());
    }
    Ok(bytes.to_vec())
}

fn should_authenticate_image_download(api_base: &str, url: &str) -> bool {
    let Ok(base) = reqwest::Url::parse(api_base) else {
        return false;
    };
    let Ok(target) = reqwest::Url::parse(url) else {
        return false;
    };
    base.scheme() == target.scheme()
        && base.host_str() == target.host_str()
        && base.port_or_known_default() == target.port_or_known_default()
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
    fn chat_image_body_uses_chat_completions_shape() {
        let body = build_chat_completions_image_generation_body(
            "model-a",
            "draw a sprite",
            None,
            2,
            "1024x1024",
            true,
        );

        assert_eq!(body["model"], "model-a");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "draw a sprite");
        assert_eq!(body["n"], 2);
        assert_eq!(body["size"], "1024x1024");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn chat_image_body_can_include_reference_image() {
        let image = "data:image/png;base64,abc";
        let body = build_chat_completions_image_generation_body(
            "model-a",
            "redraw this icon",
            Some(image),
            1,
            "1024x1024",
            true,
        );

        assert_eq!(body["messages"][0]["content"][0]["type"], "text");
        assert_eq!(
            body["messages"][0]["content"][0]["text"],
            "redraw this icon"
        );
        assert_eq!(body["messages"][0]["content"][1]["type"], "image_url");
        assert_eq!(body["messages"][0]["content"][1]["image_url"]["url"], image);
    }

    #[test]
    fn chat_image_parser_extracts_direct_openai_image_shape() {
        let raw = serde_json::json!({
            "data": [
                {"b64_json": "abc"},
                {"b64_json": "data:image/png;base64,def"}
            ]
        })
        .to_string();

        assert_eq!(
            extract_images_from_chat_completions_response(&raw),
            vec!["abc", "def"]
        );
    }

    #[test]
    fn chat_image_parser_extracts_message_json_payload() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"b64_json\":\"abc\"}"
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_images_from_chat_completions_response(&raw),
            vec!["abc"]
        );
    }

    #[test]
    fn chat_image_parser_extracts_content_array_image_url() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            {"type":"text","text":"done"},
                            {"type":"image_url","image_url":{"url":"data:image/png;base64,abc"}}
                        ]
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_images_from_chat_completions_response(&raw),
            vec!["abc"]
        );
    }

    #[test]
    fn chat_image_parser_extracts_markdown_image_url_text() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "![image](https://example.test/generated.png?sig=1)"
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_images_from_chat_completions_response(&raw),
            vec!["https://example.test/generated.png?sig=1"]
        );
    }

    #[test]
    fn chat_image_retry_detects_unsupported_size_or_n() {
        assert!(should_retry_chat_image_without_generation_options(
            "HTTP 400: unknown parameter: size"
        ));
        assert!(should_retry_chat_image_without_generation_options(
            "HTTP 400: unsupported parameter 'n'"
        ));
        assert!(!should_retry_chat_image_without_generation_options(
            "HTTP 401: invalid api key"
        ));
    }

    #[test]
    fn chat_image_retry_detects_unsupported_stream() {
        assert!(should_retry_chat_image_without_stream(
            "HTTP 400: unsupported parameter: stream"
        ));
        assert!(should_retry_chat_image_without_stream(
            "HTTP 400: streaming is not supported"
        ));
        assert!(!should_retry_chat_image_without_stream(
            "HTTP 400: unsupported parameter: size"
        ));
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

    #[test]
    fn chat_video_body_uses_chat_completions_shape() {
        let body = build_chat_completions_video_generation_body(
            "video-model",
            "make a short loop",
            "1280x720",
            "4",
            true,
            true,
        );

        assert_eq!(body["model"], "video-model");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["messages"][0]["content"], "make a short loop");
        assert_eq!(body["size"], "1280x720");
        assert_eq!(body["seconds"], "4");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn videos_body_uses_videos_shape() {
        let body =
            build_videos_generation_body("video-model", "make a short loop", "1280x720", "4");

        assert_eq!(body["model"], "video-model");
        assert_eq!(body["prompt"], "make a short loop");
        assert_eq!(body["size"], "1280x720");
        assert_eq!(body["seconds"], "4");
    }

    #[test]
    fn videos_job_parser_extracts_nested_status() {
        let raw = serde_json::json!({
            "data": {
                "id": "video_123",
                "status": "in_progress"
            }
        })
        .to_string();

        let job = parse_video_job_info(&raw).expect("video job");
        assert_eq!(job.id, "video_123");
        assert_eq!(job.status, "in_progress");
    }

    #[test]
    fn videos_parser_extracts_direct_video_url() {
        let raw = serde_json::json!({
            "id": "video_123",
            "status": "completed",
            "video_url": "https://example.test/generated.mp4"
        })
        .to_string();

        assert_eq!(
            extract_videos_from_video_api_response(&raw),
            vec!["https://example.test/generated.mp4"]
        );
    }

    #[test]
    fn chat_video_parser_extracts_message_json_video_url() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "{\"video_url\":\"https://example.test/out.mp4?sig=1\"}"
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_videos_from_chat_completions_response(&raw),
            vec!["https://example.test/out.mp4?sig=1"]
        );
    }

    #[test]
    fn chat_video_parser_extracts_content_array_video_url() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": [
                            {"type":"text","text":"done"},
                            {"type":"video_url","video_url":{"url":"https://example.test/out.webm"}}
                        ]
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_videos_from_chat_completions_response(&raw),
            vec!["https://example.test/out.webm"]
        );
    }

    #[test]
    fn chat_video_parser_extracts_data_video_base64() {
        let raw = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "data:video/mp4;base64,abc"
                    }
                }
            ]
        })
        .to_string();

        assert_eq!(
            extract_videos_from_chat_completions_response(&raw),
            vec!["abc"]
        );
    }

    #[test]
    fn chat_video_parser_extracts_sse_delta_video_url() {
        let raw = concat!(
            "data: {\"choices\":[{\"delta\":{\"content\":\"{\\\"video_url\\\":\\\"https://example.test/out.mov\\\"}\"}}]}\n\n",
            "data: [DONE]\n\n"
        );

        assert_eq!(
            extract_videos_from_chat_completions_response(raw),
            vec!["https://example.test/out.mov"]
        );
    }
}
