use serde_json::Value;

use super::transport::describe_send_error;
use super::types::StreamResponseState;
use super::utils::dedupe_images;

pub(super) async fn read_responses_stream_images(
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
pub(super) fn extract_images_from_responses_stream(raw: &str) -> Vec<String> {
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

pub(super) fn normalize_image_base64(value: &str) -> String {
    if value.starts_with("data:image/") {
        if let Some(comma_pos) = value.find("base64,") {
            return value[comma_pos + 7..].to_string();
        }
    }
    value.to_string()
}

pub(super) fn sse_data_payloads(raw: &str) -> Vec<String> {
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
