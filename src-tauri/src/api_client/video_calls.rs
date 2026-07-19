use std::time::Instant;

use serde_json::{Map, Value};

use super::download::{download_video_content_by_id, materialize_video_reference_as_bytes};
use super::media_parse::{
    extract_videos_from_chat_completions_response, extract_videos_from_video_api_response,
    format_unparseable_video_creation_response, format_unparseable_video_status_response,
    is_video_job_completed, is_video_job_failed, parse_video_job_id, parse_video_job_status,
};
use super::transport::{
    create_client, get_authenticated, post_authenticated_json, response_preview,
};
use super::types::{ApiResponseBody, VIDEO_STATUS_RETRY_ATTEMPTS, VIDEO_STATUS_RETRY_INTERVAL};
use super::types::{VIDEO_POLL_INTERVAL, VIDEO_POLL_TIMEOUT};
use super::utils::endpoint_url;

struct VideoGenerationParameters<'a> {
    api_base: &'a str,
    api_key: &'a str,
    prompt: &'a str,
    model: &'a str,
    size: &'a str,
    seconds: &'a str,
}

pub struct VideoApiRequest<'a> {
    pub api_base: &'a str,
    pub api_key: &'a str,
    pub prompt: &'a str,
    pub model: &'a str,
    pub size: &'a str,
    pub seconds: u32,
    pub video_id: Option<&'a str>,
    pub direction: Option<&'a str>,
    pub reference_images: &'a [String],
    pub proxy_url: &'a str,
}

fn require_video_generation_parameters<'a>(
    api_base: &'a str,
    api_key: &'a str,
    prompt: &'a str,
    model: &'a str,
    size: &'a str,
    seconds: &'a str,
) -> Result<VideoGenerationParameters<'a>, String> {
    let parameters = VideoGenerationParameters {
        api_base: api_base.trim(),
        api_key: api_key.trim(),
        prompt: prompt.trim(),
        model: model.trim(),
        size: size.trim(),
        seconds: seconds.trim(),
    };
    if parameters.api_key.is_empty() {
        return Err("API Key为空".into());
    }
    if parameters.api_base.is_empty() {
        return Err("API 地址为空".into());
    }
    if parameters.prompt.is_empty() {
        return Err("视频提示词为空".into());
    }
    if parameters.model.is_empty() {
        return Err("视频模型为空".into());
    }
    Ok(parameters)
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
    let parameters =
        require_video_generation_parameters(api_base, api_key, prompt, model, size, seconds)?;
    let url = endpoint_url(parameters.api_base, "chat/completions");
    let api_client = create_client(proxy_url)?;
    let stream_body = build_chat_completions_video_generation_body(
        parameters.model,
        parameters.prompt,
        parameters.size,
        parameters.seconds,
    );

    let response = post_authenticated_json(
        &api_client,
        &url,
        parameters.api_key,
        &stream_body,
        "/chat/completions 视频请求",
    )
    .await
    .map_err(build_chat_video_generation_request_error)?;

    let video_refs = extract_videos_from_chat_completions_response(&response.body);
    if video_refs.is_empty() {
        return Err(format!(
            "Chat Completions API 未返回视频内容；响应预览: {}",
            response_preview(&response.body)
        ));
    }

    materialize_video_reference_as_bytes(
        &api_client,
        parameters.api_base,
        parameters.api_key,
        &video_refs[0],
    )
    .await
}

pub async fn call_video_api(
    endpoint: &str,
    request: &VideoApiRequest<'_>,
) -> Result<Vec<u8>, String> {
    require_video_generation_parameters(
        request.api_base,
        request.api_key,
        request.prompt,
        request.model,
        request.size,
        &request.seconds.to_string(),
    )?;
    let url = endpoint_url(request.api_base, endpoint);
    let api_client = create_client(request.proxy_url)?;
    let body = build_video_api_body(request);
    let label = format!("/{endpoint} 视频请求");
    let created = post_authenticated_json(&api_client, &url, request.api_key, &body, &label)
        .await
        .map_err(|err| build_video_api_request_error(endpoint, err))?;

    finish_video_job(&api_client, endpoint, request, &created.body).await
}

async fn finish_video_job(
    api_client: &reqwest::Client,
    endpoint: &str,
    request: &VideoApiRequest<'_>,
    created_body: &str,
) -> Result<Vec<u8>, String> {
    let direct_refs = extract_videos_from_video_api_response(created_body);
    if let Some(video_ref) = direct_refs.first() {
        return materialize_video_reference_as_bytes(
            api_client,
            request.api_base,
            request.api_key,
            video_ref,
        )
        .await;
    }

    let context = format!("/{endpoint} 创建任务响应");
    let job_id = parse_video_job_id(created_body)
        .ok_or_else(|| format_unparseable_video_creation_response(&context, created_body))?;
    let started = Instant::now();
    let mut last_status = "submitted".to_string();
    loop {
        if started.elapsed() > VIDEO_POLL_TIMEOUT {
            return Err(format!(
                "Videos API 任务超时: id={} status={}，已等待 {} 秒",
                job_id,
                last_status,
                VIDEO_POLL_TIMEOUT.as_secs()
            ));
        }

        let status_url = endpoint_url(request.api_base, &format!("videos/{job_id}"));
        let status_response =
            get_video_status(api_client, &status_url, request.api_key, &job_id).await?;
        let direct_refs = extract_videos_from_video_api_response(&status_response.body);
        if let Some(video_ref) = direct_refs.first() {
            return materialize_video_reference_as_bytes(
                api_client,
                request.api_base,
                request.api_key,
                video_ref,
            )
            .await;
        }
        let job = parse_video_job_status(&status_response.body).ok_or_else(|| {
            format_unparseable_video_status_response("/videos 任务状态响应", &status_response.body)
        })?;
        last_status = job.status.clone();
        if is_video_job_completed(&job.status) {
            return download_video_content_by_id(
                api_client,
                request.api_base,
                request.api_key,
                &job_id,
            )
            .await;
        }
        if is_video_job_failed(&job.status) {
            let error_detail = job
                .error_message
                .as_deref()
                .map(|message| format!(" message={message}"))
                .unwrap_or_default();
            return Err(format!(
                "Videos API 任务失败: id={job_id} status={}{}",
                job.status, error_detail
            ));
        }
        tokio::time::sleep(VIDEO_POLL_INTERVAL).await;
    }
}

async fn get_video_status(
    api_client: &reqwest::Client,
    status_url: &str,
    api_key: &str,
    job_id: &str,
) -> Result<ApiResponseBody, String> {
    let mut last_error = String::new();
    for attempt in 1..=VIDEO_STATUS_RETRY_ATTEMPTS {
        match get_authenticated(api_client, status_url, api_key, "/videos 任务状态").await {
            Ok(response) => return Ok(response),
            Err(error) => last_error = error,
        }
        if attempt < VIDEO_STATUS_RETRY_ATTEMPTS {
            tokio::time::sleep(VIDEO_STATUS_RETRY_INTERVAL).await;
        }
    }
    Err(format!(
        "Videos API 状态查询连续失败 {VIDEO_STATUS_RETRY_ATTEMPTS} 次: id={job_id}，最后错误：{last_error}"
    ))
}

pub(super) fn build_video_api_body(request: &VideoApiRequest<'_>) -> Value {
    let mut body = Map::from_iter([
        ("model".into(), Value::String(request.model.to_string())),
        ("prompt".into(), Value::String(request.prompt.to_string())),
        ("seconds".into(), Value::from(request.seconds)),
        ("size".into(), Value::String(request.size.to_string())),
    ]);
    if let Some(video_id) = request.video_id {
        body.insert("video_id".into(), Value::String(video_id.to_string()));
    }
    if let Some(direction) = request.direction {
        body.insert("direction".into(), Value::String(direction.to_string()));
    }
    match request.reference_images {
        [image] => {
            body.insert(
                "input_reference".into(),
                serde_json::json!({"image_url": image}),
            );
        }
        images if !images.is_empty() => {
            body.insert("reference_images".into(), serde_json::json!(images));
        }
        _ => {}
    }
    Value::Object(body)
}

pub(super) fn build_chat_completions_video_generation_body(
    model: &str,
    prompt: &str,
    size: &str,
    seconds: &str,
) -> Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "user", "content": prompt}
        ],
        "size": size,
        "seconds": seconds,
        "stream": true,
    })
}

pub(super) fn build_chat_video_generation_request_error(err: String) -> String {
    format!(
        "/chat/completions 视频请求失败：{err}。解决方法：请确认当前服务支持通过 /chat/completions 生成视频，且支持 stream、size/seconds 参数；如果服务使用不同的视频接口或参数，请切换视频 API 模式、模型或提供方配置。"
    )
}

pub(super) fn build_video_api_request_error(endpoint: &str, err: String) -> String {
    if err
        .to_ascii_lowercase()
        .contains("text-to-video is not supported")
    {
        return format!(
            "/{endpoint} 视频请求失败：{err}。当前模型不支持纯文本生成视频；请添加参考图后重试，或改用支持文生视频的模型（例如 grok-imagine-video）。"
        );
    }
    format!(
        "/{endpoint} 视频请求失败：{err}。解决方法：请确认当前服务支持该视频端点、请求体为 JSON，且 model、prompt、size、seconds 与当前模型兼容。"
    )
}
