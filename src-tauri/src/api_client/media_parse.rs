use serde_json::Value;

use super::media_refs::{
    extract_data_image_urls, extract_data_video_urls, extract_http_urls, looks_like_http_url,
    looks_like_image_ref, looks_like_image_url_or_signed_url, looks_like_video_ref,
    looks_like_video_url_or_signed_url, trim_markdown_code_fence,
};
use super::sse::{normalize_image_base64, sse_data_payloads};
use super::text::looks_like_sse_body;
use super::transport::response_preview;
use super::types::VideoJobStatus;
use super::utils::dedupe_images;

pub(super) fn normalize_video_base64(value: &str) -> String {
    if value.starts_with("data:video/") {
        if let Some(comma_pos) = value.find("base64,") {
            return value[comma_pos + 7..].to_string();
        }
    }
    value.to_string()
}

pub(super) fn extract_images_from_chat_completions_response(body: &str) -> Vec<String> {
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

pub(super) fn extract_images_from_image_api_response(body: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(body.trim()) else {
        return Vec::new();
    };
    let items = value
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| value.as_array());
    let Some(items) = items else {
        return Vec::new();
    };

    let mut images = Vec::new();
    for item in items {
        if let Some(image) = item.get("b64_json").and_then(Value::as_str) {
            images.push(normalize_image_base64(image));
        }
        if let Some(url) = item.get("url").and_then(Value::as_str) {
            images.push(normalize_image_base64(url));
        }
    }
    dedupe_images(images)
}

pub(super) fn extract_videos_from_chat_completions_response(body: &str) -> Vec<String> {
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

pub(super) fn extract_videos_from_video_api_response(body: &str) -> Vec<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let mut videos = Vec::new();
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        collect_chat_completion_videos(&value, &mut videos);
        for pointer in ["/video/url", "/data/video/url", "/result/video/url"] {
            if let Some(url) = value.pointer(pointer).and_then(Value::as_str) {
                if looks_like_video_ref(url) || looks_like_http_url(url) {
                    videos.push(normalize_video_base64(url));
                }
            }
        }
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

pub(super) fn parse_video_job_id(body: &str) -> Option<String> {
    let value = serde_json::from_str::<Value>(body.trim()).ok()?;
    find_string_for_keys(
        &value,
        &["request_id", "requestId", "id", "video_id", "videoId"],
    )
}

pub(super) fn parse_video_job_status(body: &str) -> Option<VideoJobStatus> {
    let value = serde_json::from_str::<Value>(body.trim()).ok()?;
    let status = find_string_for_keys(&value, &["status", "state"])?;
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
            .and_then(Value::as_str)
            .map(str::to_string)
    })
    .or_else(|| {
        value
            .get("error")
            .and_then(|error| find_string_for_keys(error, &["message", "code"]))
    });

    Some(VideoJobStatus {
        status,
        error_message,
    })
}

pub(super) fn format_unparseable_video_creation_response(context: &str, body: &str) -> String {
    format!(
        "{context}无法解析：创建响应必须返回 request_id、id、video_id 或 videoId；也可以直接返回视频 URL/base64。响应预览: {}",
        response_preview(body)
    )
}

pub(super) fn format_unparseable_video_status_response(context: &str, body: &str) -> String {
    format!(
        "{context}无法解析：状态响应必须返回 status/state，完成时应在 video.url 或标准视频 URL/base64 字段中返回成品。响应预览: {}",
        response_preview(body)
    )
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

pub(super) fn is_video_job_completed(status: &str) -> bool {
    matches!(
        status.trim().to_ascii_lowercase().as_str(),
        "completed" | "complete" | "succeeded" | "success" | "done" | "finished"
    )
}

pub(super) fn is_video_job_failed(status: &str) -> bool {
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
            for key in ["b64_json", "image_base64", "base64"] {
                if let Some(image) = map.get(key).and_then(Value::as_str) {
                    images.push(normalize_image_base64(image));
                }
            }

            if map.get("type").and_then(Value::as_str) == Some("image_url") {
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
            let is_video_context = map
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|type_name| type_name.contains("video"))
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
