use std::error::Error;
use std::time::Duration;

use reqwest::{multipart, Client};
use serde_json::Value;

use super::types::{ApiResponseBody, CONNECT_TIMEOUT, GENERATION_TIMEOUT, USER_AGENT};

pub(super) fn create_client(proxy_url: &str) -> Result<Client, String> {
    let proxy_url = proxy_url.trim();
    if !proxy_url.is_empty() {
        return build_proxy_client(proxy_url);
    }

    build_environment_proxy_client()
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

fn build_environment_proxy_client() -> Result<Client, String> {
    Client::builder()
        .user_agent(USER_AGENT)
        .http1_only()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(GENERATION_TIMEOUT)
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建HTTP客户端失败: {}", e))
}

pub(super) async fn send_authenticated_json_bytes(
    client: &Client,
    url: &str,
    api_key: &str,
    body: &[u8],
    log_label: &str,
) -> Result<reqwest::Response, String> {
    client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .body(body.to_vec())
        .send()
        .await
        .map_err(|error| report_send_error(&error, log_label))
}

pub(super) async fn post_authenticated_json(
    client: &Client,
    url: &str,
    api_key: &str,
    body: &Value,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|error| report_send_error(&error, log_label))?;

    read_api_response_body(resp, log_label).await
}

pub(super) async fn post_authenticated_multipart(
    client: &Client,
    url: &str,
    api_key: &str,
    form: multipart::Form,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|error| report_send_error(&error, log_label))?;

    read_api_response_body(resp, log_label).await
}

pub(super) async fn get_authenticated(
    client: &Client,
    url: &str,
    api_key: &str,
    log_label: &str,
) -> Result<ApiResponseBody, String> {
    let resp = client
        .get(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|error| report_send_error(&error, log_label))?;

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
        .map(|value| {
            value
                .to_str()
                .map(str::to_string)
                .map_err(|e| format!("{log_label} 返回了非法 Content-Type 响应头: {e}"))
        })
        .transpose()?
        .unwrap_or_default();
    let body = read_response_text(resp, log_label).await?;
    if !status.is_success() {
        let msg = parse_http_error(status.as_u16(), &body);
        eprintln!("[api] {log_label} {msg}");
        return Err(msg);
    }

    Ok(ApiResponseBody { content_type, body })
}

pub(super) async fn read_response_text(
    resp: reqwest::Response,
    context: &str,
) -> Result<String, String> {
    resp.text()
        .await
        .map_err(|e| format_response_text_read_error(context, &describe_send_error(&e)))
}

pub(super) fn format_response_text_read_error(context: &str, detail: &str) -> String {
    format!(
        "{context} 响应体读取失败：{detail}。解决方法：请检查网络连接、代理配置和服务端是否提前断开连接后重试。"
    )
}

pub(super) fn describe_send_error(e: &reqwest::Error) -> String {
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

fn report_send_error(error: &reqwest::Error, log_label: &str) -> String {
    let message = describe_send_error(error);
    eprintln!("[api] {log_label} 请求失败: {message}");
    message
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

/// 从HTTP错误响应中提取可读的错误消息
pub(super) fn parse_http_error(status: u16, body: &str) -> String {
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
        if let Some(msg) = json["error"].as_str() {
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

pub(super) fn response_preview(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 300 {
        body.to_string()
    } else {
        let preview: String = body.chars().take(300).collect();
        format!("{preview}...（已截断）")
    }
}
