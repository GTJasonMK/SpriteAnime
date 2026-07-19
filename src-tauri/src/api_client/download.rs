use base64::Engine;

use super::media_parse::normalize_video_base64;
use super::media_refs::looks_like_http_url;
use super::sse::normalize_image_base64;
use super::transport::{describe_send_error, parse_http_error, read_response_text};
use super::utils::endpoint_url;

pub(super) async fn materialize_image_reference_as_base64(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    image_ref: &str,
) -> Result<String, String> {
    let image_ref = image_ref.trim();
    if image_ref.is_empty() {
        return Ok(String::new());
    }
    if looks_like_http_url(image_ref) {
        return download_image_url_as_base64(client, api_base, api_key, image_ref).await;
    }
    Ok(normalize_image_base64(image_ref))
}

pub(super) async fn materialize_video_reference_as_bytes(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    video_ref: &str,
) -> Result<Vec<u8>, String> {
    let video_ref = video_ref.trim();
    if video_ref.is_empty() {
        return Err("Chat Completions API 返回了空视频引用".into());
    }
    if looks_like_http_url(video_ref) {
        return download_video_url_as_bytes(client, api_base, api_key, video_ref).await;
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
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    url: &str,
) -> Result<String, String> {
    let mut request = client.get(url).header("Accept", "image/*,*/*");
    if should_authenticate_image_download(api_base, url) {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }
    let resp = request.send().await.map_err(|e| describe_send_error(&e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = read_response_text(resp, "图片 URL 错误响应").await?;
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
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    url: &str,
) -> Result<Vec<u8>, String> {
    let mut request = client.get(url).header("Accept", "video/*,*/*");
    if should_authenticate_image_download(api_base, url) {
        request = request.header("Authorization", format!("Bearer {}", api_key));
    }
    let resp = request.send().await.map_err(|e| {
        format!(
            "视频 URL 下载失败：{}。请在设置 > API 配置 > 视频生成中填写可访问该地址的代理，或确认启动应用的环境包含有效的 HTTP_PROXY/HTTPS_PROXY/ALL_PROXY。",
            describe_send_error(&e)
        )
    })?;
    let status = resp.status();
    if !status.is_success() {
        let body = read_response_text(resp, "视频 URL 错误响应").await?;
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

pub(super) async fn download_video_content_by_id(
    client: &reqwest::Client,
    api_base: &str,
    api_key: &str,
    video_id: &str,
) -> Result<Vec<u8>, String> {
    let url = video_content_url(api_base, video_id);
    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Accept", "video/*,*/*")
        .send()
        .await
        .map_err(|e| describe_send_error(&e))?;
    let status = resp.status();
    if !status.is_success() {
        let body = read_response_text(resp, "/videos 内容错误响应").await?;
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

pub(super) fn video_content_url(api_base: &str, video_id: &str) -> String {
    endpoint_url(
        api_base,
        &format!("videos/{video_id}/content?variant=video"),
    )
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
