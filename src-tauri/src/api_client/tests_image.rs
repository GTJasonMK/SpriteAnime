use super::*;
use super::{media_parse::*, sse::*, transport::*};

#[test]
fn response_text_read_error_includes_context_and_resolution_steps() {
    let err = format_response_text_read_error("/models", "连接中断: early eof");

    assert!(err.contains("/models 响应体读取失败"));
    assert!(err.contains("连接中断"));
    assert!(err.contains("检查网络连接、代理配置"));
    assert!(err.contains("服务端是否提前断开连接"));
}

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
fn http_error_parser_accepts_top_level_error_string() {
    let body =
        r#"{"code":"invalid-argument","error":"Text-to-video is not supported for this model."}"#;

    assert_eq!(
        parse_http_error(400, body),
        "HTTP 400: Text-to-video is not supported for this model."
    );
}

#[test]
fn image_edit_model_rewrite_error_names_upstream_contract() {
    let err = format_images_request_error(
        "/images/edits multipart",
        "HTTP 400: Failed to rewrite upstream model".into(),
    );

    assert!(err.contains("请求已到达服务"));
    assert!(err.contains("上游网关无法把当前模型映射"));
    assert!(err.contains("删除旧运行并重新创建"));
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

    let text = parse_text_api_response("Chat Completions API", raw, "text/event-stream").unwrap();
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

    let text = parse_text_api_response("Chat Completions API", &raw, "application/json").unwrap();
    assert!(text.contains("\"prompt\":\"角色跑步\""));
}

#[test]
fn responses_text_body_can_include_reference_image() {
    let body = build_responses_text_body(
        "model-a",
        "system",
        "hello",
        Some("data:image/jpeg;base64,abc"),
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
fn responses_text_error_includes_endpoint_resolution_steps() {
    let err = build_responses_text_error("HTTP 404: not found".into(), None);

    assert!(err.contains("/responses 文本请求失败"));
    assert!(err.contains("支持 /responses 文本接口"));
    assert!(err.contains("把调用方式改为 Chat Completions"));
}

#[test]
fn responses_text_error_includes_multimodal_resolution_steps() {
    let err = build_responses_text_error("HTTP 400: image input unsupported".into(), Some("image"));

    assert!(err.contains("/responses 文本请求失败"));
    assert!(err.contains("Responses 多模态输入"));
    assert!(err.contains("关闭“参考图视觉理解”后重试"));
}

#[test]
fn image_generation_body_uses_plain_text_input_without_reference_image() {
    let body = build_responses_image_generation_body(
        "model-a",
        "draw a sprite",
        &[],
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
        &[image],
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
fn image_generation_body_preserves_two_reference_image_order() {
    let references = [
        "data:image/png;base64,target",
        "data:image/png;base64,anchor",
    ];
    let body = build_responses_image_generation_body(
        "model-a",
        "continue the sequence",
        &references,
        1,
        "1024x1024",
        true,
    );

    assert_eq!(body["input"][0]["content"][1]["image_url"], references[0]);
    assert_eq!(body["input"][0]["content"][2]["image_url"], references[1]);
}

#[test]
fn standard_image_body_uses_documented_fields_and_json_reference() {
    let request = ImageApiRequest {
        api_base: "https://api.example/v1",
        api_key: "key",
        prompt: "redraw",
        model: "image-model",
        count: 2,
        size: "1024x1024",
        aspect_ratio: "1:1",
        resolution: "1K",
        proxy_url: "",
    };
    let references = ["data:image/png;base64,abc"];
    let body = build_images_json_body(&request, &references);

    assert_eq!(body["n"], 2);
    assert_eq!(body["response_format"], "b64_json");
    assert_eq!(body["size"], "1024x1024");
    assert_eq!(body["aspect_ratio"], "1:1");
    assert_eq!(body["resolution"], "1K");
    assert_eq!(body["images"][0]["image_url"], "data:image/png;base64,abc");
}

#[test]
fn standard_image_body_preserves_two_reference_image_order() {
    let request = ImageApiRequest {
        api_base: "https://api.example/v1",
        api_key: "key",
        prompt: "redraw",
        model: "image-model",
        count: 1,
        size: "1024x1024",
        aspect_ratio: "1:1",
        resolution: "1K",
        proxy_url: "",
    };
    let references = ["target", "anchor"];
    let body = build_images_json_body(&request, &references);

    assert_eq!(body["images"][0]["image_url"], "target");
    assert_eq!(body["images"][1]["image_url"], "anchor");
}

#[tokio::test]
async fn standard_json_edit_rejects_missing_reference_before_request() {
    let request = ImageApiRequest {
        api_base: "http://127.0.0.1:1/v1",
        api_key: "key",
        prompt: "redraw",
        model: "image-model",
        count: 1,
        size: "1024x1024",
        aspect_ratio: "1:1",
        resolution: "1K",
        proxy_url: "",
    };

    let error = call_images_edits_json_api(&request, &[]).await.unwrap_err();

    assert_eq!(error, "/images/edits JSON 至少需要一张参考图");
}

#[test]
fn standard_image_parser_accepts_url_and_base64_data_items() {
    let raw = serde_json::json!({
        "data": [
            {"url": "https://example.test/generated.png"},
            {"b64_json": "data:image/png;base64,abc"}
        ]
    })
    .to_string();

    assert_eq!(
        extract_images_from_image_api_response(&raw),
        vec!["https://example.test/generated.png", "abc"]
    );
}

#[test]
fn chat_image_body_uses_chat_completions_shape() {
    let body = build_chat_completions_image_generation_body(
        "model-a",
        "draw a sprite",
        &[],
        2,
        "1024x1024",
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
        &[image],
        1,
        "1024x1024",
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
fn chat_image_body_preserves_two_reference_image_order() {
    let references = ["target", "anchor"];
    let body = build_chat_completions_image_generation_body(
        "model-a",
        "continue",
        &references,
        1,
        "1024x1024",
    );

    assert_eq!(
        body["messages"][0]["content"][1]["image_url"]["url"],
        "target"
    );
    assert_eq!(
        body["messages"][0]["content"][2]["image_url"]["url"],
        "anchor"
    );
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
