use reqwest::multipart;
use serde_json::{Map, Value};

use super::download::materialize_image_reference_as_base64;
use super::media_parse::{
    extract_images_from_chat_completions_response, extract_images_from_image_api_response,
};
use super::sse::read_responses_stream_images;
use super::transport::{
    create_client, parse_http_error, post_authenticated_json, post_authenticated_multipart,
    read_response_text, response_preview, send_authenticated_json_bytes,
};
use super::utils::{dedupe_images, endpoint_url};

pub struct ImageApiRequest<'a> {
    pub api_base: &'a str,
    pub api_key: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub count: u32,
    pub size: &'a str,
    pub aspect_ratio: &'a str,
    pub resolution: &'a str,
    pub proxy_url: &'a str,
}

/// POST /responses — 使用 image_generation 工具的流式请求。
pub async fn call_responses_api(
    request: &ImageApiRequest<'_>,
    input_image_data_urls: &[&str],
) -> Result<Vec<String>, String> {
    require_image_generation_request(request.api_key, request.prompt)?;

    let url = endpoint_url(request.api_base, "responses");
    let body = build_responses_image_generation_body(
        request.model,
        request.prompt,
        input_image_data_urls,
        request.count,
        request.size,
        true,
    );
    let body_bytes =
        serde_json::to_vec(&body).map_err(|e| format!("序列化 Responses 请求失败: {e}"))?;
    let api_client = create_client(request.proxy_url)?;

    let resp = send_authenticated_json_bytes(
        &api_client,
        &url,
        request.api_key,
        &body_bytes,
        "/responses",
    )
    .await?;

    let status = resp.status();
    if !status.is_success() {
        let resp_body = read_response_text(resp, "/responses").await?;
        let msg = parse_http_error(status.as_u16(), &resp_body);
        eprintln!("[api] /responses {msg}");
        return Err(msg);
    }

    read_responses_stream_images(resp, request.count).await
}

pub async fn call_chat_completions_image_api(
    request: &ImageApiRequest<'_>,
    input_image_data_urls: &[&str],
) -> Result<Vec<String>, String> {
    require_image_generation_request(request.api_key, request.prompt)?;

    let url = endpoint_url(request.api_base, "chat/completions");
    let stream_body = build_chat_completions_image_generation_body(
        request.model,
        request.prompt,
        input_image_data_urls,
        request.count,
        request.size,
    );
    let api_client = create_client(request.proxy_url)?;

    let response = post_authenticated_json(
        &api_client,
        &url,
        request.api_key,
        &stream_body,
        "/chat/completions 生图请求",
    )
    .await
    .map_err(|err| build_chat_image_generation_request_error(err, input_image_data_urls.len()))?;

    let image_refs = extract_images_from_chat_completions_response(&response.body);
    if image_refs.is_empty() {
        return Err(format!(
            "Chat Completions API 未返回图片内容；响应预览: {}",
            response_preview(&response.body)
        ));
    }

    let mut images = Vec::new();
    for image_ref in image_refs.into_iter().take(request.count as usize) {
        let image = materialize_image_reference_as_base64(
            &api_client,
            request.api_base,
            request.api_key,
            &image_ref,
        )
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

pub async fn call_images_generations_api(
    request: &ImageApiRequest<'_>,
) -> Result<Vec<String>, String> {
    call_images_json_api("images/generations", request, &[]).await
}

pub async fn call_images_edits_json_api(
    request: &ImageApiRequest<'_>,
    image_data_urls: &[&str],
) -> Result<Vec<String>, String> {
    if image_data_urls.is_empty() {
        return Err("/images/edits JSON 至少需要一张参考图".into());
    }
    call_images_json_api("images/edits", request, image_data_urls).await
}

pub async fn call_images_edits_multipart_api(
    request: &ImageApiRequest<'_>,
    images: &[(&[u8], &str)],
) -> Result<Vec<String>, String> {
    require_image_generation_request(request.api_key, request.prompt)?;
    if images.is_empty() {
        return Err("/images/edits multipart 至少需要一张参考图".into());
    }
    let mut form = multipart::Form::new()
        .text("model", request.model.to_string())
        .text("prompt", request.prompt.to_string())
        .text("n", request.count.to_string())
        .text("response_format", "b64_json");
    for (index, (bytes, mime)) in images.iter().enumerate() {
        let file_name = multipart_reference_file_name(index, mime)?;
        let part = multipart::Part::bytes(bytes.to_vec())
            .file_name(file_name)
            .mime_str(mime)
            .map_err(|e| format!("构建 /images/edits 第{}张参考图表单失败: {e}", index + 1))?;
        form = form.part("image", part);
    }
    if !request.size.trim().is_empty() {
        form = form.text("size", request.size.to_string());
    }
    if !request.aspect_ratio.trim().is_empty() {
        form = form.text("aspect_ratio", request.aspect_ratio.to_string());
    }
    if !request.resolution.trim().is_empty() && request.resolution != "原始" {
        form = form.text("resolution", request.resolution.to_string());
    }

    let client = create_client(request.proxy_url)?;
    let url = endpoint_url(request.api_base, "images/edits");
    let response = post_authenticated_multipart(
        &client,
        &url,
        request.api_key,
        form,
        "/images/edits multipart 图片请求",
    )
    .await
    .map_err(|err| format_images_request_error("/images/edits multipart", err))?;
    materialize_image_api_response(&client, request, &response.body).await
}

pub(super) fn multipart_reference_file_name(index: usize, mime: &str) -> Result<String, String> {
    let extension = match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        other => {
            return Err(format!(
                "第{}张参考图 MIME 类型不受支持：{other}",
                index + 1
            ))
        }
    };
    Ok(format!("reference_{:02}.{extension}", index + 1))
}

async fn call_images_json_api(
    endpoint: &str,
    request: &ImageApiRequest<'_>,
    image_data_urls: &[&str],
) -> Result<Vec<String>, String> {
    require_image_generation_request(request.api_key, request.prompt)?;
    let body = build_images_json_body(request, image_data_urls);
    let client = create_client(request.proxy_url)?;
    let url = endpoint_url(request.api_base, endpoint);
    let response = post_authenticated_json(
        &client,
        &url,
        request.api_key,
        &body,
        &format!("/{endpoint} 图片请求"),
    )
    .await
    .map_err(|err| format_images_request_error(&format!("/{endpoint}"), err))?;
    materialize_image_api_response(&client, request, &response.body).await
}

pub(super) fn build_images_json_body(
    request: &ImageApiRequest<'_>,
    image_data_urls: &[&str],
) -> Value {
    let mut body = Map::from_iter([
        ("model".into(), Value::String(request.model.to_string())),
        ("prompt".into(), Value::String(request.prompt.to_string())),
        ("n".into(), Value::from(request.count)),
        ("response_format".into(), Value::String("b64_json".into())),
    ]);
    if !request.size.trim().is_empty() {
        body.insert("size".into(), Value::String(request.size.to_string()));
    }
    if !request.aspect_ratio.trim().is_empty() {
        body.insert(
            "aspect_ratio".into(),
            Value::String(request.aspect_ratio.to_string()),
        );
    }
    if !request.resolution.trim().is_empty() && request.resolution != "原始" {
        body.insert(
            "resolution".into(),
            Value::String(request.resolution.to_string()),
        );
    }
    if !image_data_urls.is_empty() {
        body.insert(
            "images".into(),
            Value::Array(
                image_data_urls
                    .iter()
                    .map(|image| serde_json::json!({"image_url": image}))
                    .collect(),
            ),
        );
    }
    Value::Object(body)
}

async fn materialize_image_api_response(
    client: &reqwest::Client,
    request: &ImageApiRequest<'_>,
    body: &str,
) -> Result<Vec<String>, String> {
    let image_refs = extract_images_from_image_api_response(body);
    if image_refs.is_empty() {
        return Err(format!(
            "标准图片 API 未返回 data[].url 或 data[].b64_json；响应预览: {}",
            response_preview(body)
        ));
    }
    let mut images = Vec::new();
    for image_ref in image_refs.into_iter().take(request.count as usize) {
        let image = materialize_image_reference_as_base64(
            client,
            request.api_base,
            request.api_key,
            &image_ref,
        )
        .await?;
        if !image.trim().is_empty() {
            images.push(image);
        }
    }
    let images = dedupe_images(images);
    if images.is_empty() {
        return Err("标准图片 API 返回了图片引用，但内容为空".into());
    }
    Ok(images)
}

pub(super) fn format_images_request_error(endpoint: &str, err: String) -> String {
    if err.contains("Failed to rewrite upstream model") {
        return format!(
            "{endpoint} 图片请求失败：{err}。该响应表示请求已到达服务，但上游网关无法把当前模型映射到这个图片端点。解决方法：请在设置 > API 配置 > 图片生成中改用支持参考图编辑的模型或调用方式；已有分组重绘运行绑定了创建时的 API 快照，修改配置后请删除旧运行并重新创建。"
        );
    }
    format!(
        "{endpoint} 图片请求失败：{err}。解决方法：请确认 API 地址包含正确的 /v1 根路径、模型支持该端点，并按当前调用方式选择纯文本生成或参考图编辑。"
    )
}

fn require_image_generation_request(api_key: &str, prompt: &str) -> Result<(), String> {
    if api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if prompt.is_empty() {
        return Err("提示词为空".into());
    }
    Ok(())
}

pub(super) fn build_responses_image_generation_body(
    model: &str,
    prompt: &str,
    input_image_data_urls: &[&str],
    count: u32,
    size: &str,
    stream: bool,
) -> Value {
    let input =
        if input_image_data_urls.is_empty() {
            Value::String(prompt.to_string())
        } else {
            let mut content = vec![serde_json::json!({"type": "input_text", "text": prompt})];
            content.extend(input_image_data_urls.iter().map(
                |image_url| serde_json::json!({"type": "input_image", "image_url": image_url}),
            ));
            serde_json::json!([
                {
                    "role": "user",
                    "content": content
                }
            ])
        };

    serde_json::json!({
        "model": model,
        "input": input,
        "tools": [{"type": "image_generation", "size": size, "n": count}],
        "stream": stream,
    })
}

pub(super) fn build_chat_completions_image_generation_body(
    model: &str,
    prompt: &str,
    input_image_data_urls: &[&str],
    count: u32,
    size: &str,
) -> Value {
    let user_content = if input_image_data_urls.is_empty() {
        Value::String(prompt.to_string())
    } else {
        let mut content = vec![serde_json::json!({"type": "text", "text": prompt})];
        content.extend(input_image_data_urls.iter().map(
            |image_url| serde_json::json!({"type": "image_url", "image_url": {"url": image_url}}),
        ));
        Value::Array(content)
    };

    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": user_content}
        ],
        "n": count,
        "size": size,
        "stream": true,
    })
}

pub(super) fn build_chat_image_generation_request_error(
    err: String,
    reference_count: usize,
) -> String {
    let reference_note = if reference_count > 0 {
        "；当前请求包含参考图，请确认模型支持多模态 image_url 输入，并在多图请求中支持多个 image_url"
    } else {
        ""
    };
    format!(
        "/chat/completions 生图请求失败：{err}。解决方法：请确认当前服务支持通过 /chat/completions 生图，且支持 stream、n/size 参数{reference_note}；如果服务不支持这些参数，请切换到 Responses 生图模式，或调整模型/API 服务配置。"
    )
}
