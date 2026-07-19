use super::*;
use super::{download::*, media_parse::*, utils::*};

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
fn chat_image_error_includes_endpoint_resolution_steps() {
    let err = build_chat_image_generation_request_error(
        "HTTP 400: unsupported parameter: stream".into(),
        0,
    );

    assert!(err.contains("/chat/completions 生图请求失败"));
    assert!(err.contains("stream"));
    assert!(err.contains("n/size"));
    assert!(err.contains("Responses 生图模式"));
}

#[test]
fn chat_image_error_includes_reference_image_resolution_steps() {
    let err =
        build_chat_image_generation_request_error("HTTP 400: image input unsupported".into(), 2);

    assert!(err.contains("/chat/completions 生图请求失败"));
    assert!(err.contains("多模态 image_url 输入"));
    assert!(err.contains("模型/API 服务配置"));
}

#[test]
fn models_response_parser_rejects_non_json_success_body() {
    let err = parse_models_response_body("<html>login required</html>").unwrap_err();

    assert!(err.contains("/models 响应解析失败"));
    assert!(err.contains("不是合法 JSON"));
    assert!(err.contains("兼容 OpenAI 的接口根路径"));
    assert!(err.contains("HTML、登录页或网关页面"));
    assert!(err.contains("<html>login required</html>"));
}

#[test]
fn models_check_result_warns_when_named_model_cannot_be_verified() {
    let result =
        build_models_api_check_result("https://api.example.test/models".into(), "model-a", &[]);

    assert_eq!(result.status, "warning");
    assert!(result.message.contains("未发现可识别的模型 ID"));
    assert!(result.message.contains("无法确认模型名"));
    assert!(result.message.contains("请确认模型名称和 /models 响应结构"));
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
    );

    assert_eq!(body["model"], "video-model");
    assert_eq!(body["messages"][0]["role"], "user");
    assert_eq!(body["messages"][0]["content"], "make a short loop");
    assert_eq!(body["size"], "1280x720");
    assert_eq!(body["seconds"], "4");
    assert_eq!(body["stream"], true);
}

#[test]
fn chat_video_error_includes_endpoint_resolution_steps() {
    let err = build_chat_video_generation_request_error("HTTP 400: unsupported parameter".into());

    assert!(err.contains("/chat/completions 视频请求失败"));
    assert!(err.contains("stream"));
    assert!(err.contains("size/seconds"));
    assert!(err.contains("切换视频 API 模式"));
}

#[test]
fn videos_error_requires_json_contract() {
    let err = build_video_api_request_error("videos", "HTTP 415: content-type".into());

    assert!(err.contains("/videos 视频请求失败"));
    assert!(err.contains("请求体为 JSON"));
    assert!(err.contains("model、prompt、size、seconds"));
}

#[test]
fn video_content_endpoint_includes_documented_variant() {
    assert_eq!(
        video_content_url("https://api.imggb.top/v1", "video_123"),
        "https://api.imggb.top/v1/videos/video_123/content?variant=video"
    );
}

#[test]
fn text_to_video_capability_error_suggests_reference_or_model_change() {
    let err = build_video_api_request_error(
        "videos",
        "HTTP 400: Text-to-video is not supported for this model.".into(),
    );

    assert!(err.contains("当前模型不支持纯文本生成视频"));
    assert!(err.contains("添加参考图"));
    assert!(err.contains("grok-imagine-video"));
}

#[test]
fn standard_video_body_uses_integer_seconds_and_single_reference() {
    let references = vec!["data:image/png;base64,abc".into()];
    let body = build_video_api_body(&VideoApiRequest {
        api_base: "https://api.example/v1",
        api_key: "key",
        prompt: "animate",
        model: "video-model",
        size: "1280x720",
        seconds: 6,
        video_id: Some("video_123"),
        direction: Some("forward"),
        reference_images: &references,
        proxy_url: "",
    });

    assert_eq!(body["seconds"], 6);
    assert_eq!(body["video_id"], "video_123");
    assert_eq!(body["direction"], "forward");
    assert_eq!(
        body["input_reference"]["image_url"],
        "data:image/png;base64,abc"
    );
    assert!(body.get("reference_images").is_none());
}

#[test]
fn standard_video_body_uses_reference_images_for_multiple_inputs() {
    let references = vec![
        "https://example.test/1.png".into(),
        "https://example.test/2.png".into(),
    ];
    let body = build_video_api_body(&VideoApiRequest {
        api_base: "https://api.example/v1",
        api_key: "key",
        prompt: "animate",
        model: "video-model",
        size: "1280x720",
        seconds: 8,
        video_id: None,
        direction: None,
        reference_images: &references,
        proxy_url: "",
    });

    assert_eq!(body["reference_images"].as_array().unwrap().len(), 2);
    assert!(body.get("input_reference").is_none());
}

#[test]
fn videos_creation_parser_accepts_gggb_request_id_without_status() {
    let raw = r#"{"request_id":"7f22b1cc-7b21-911c-aeef-9bfc14835353"}"#;

    assert_eq!(
        parse_video_job_id(raw).as_deref(),
        Some("7f22b1cc-7b21-911c-aeef-9bfc14835353")
    );
}

#[test]
fn videos_status_parser_accepts_status_without_repeated_id() {
    let raw = r#"{"status":"pending","progress":20}"#;

    let status = parse_video_job_status(raw).expect("video status");
    assert_eq!(status.status, "pending");
}

#[test]
fn videos_status_parser_accepts_top_level_failure_message() {
    let raw = r#"{"status":"failed","error":"upstream rejected the request"}"#;

    let status = parse_video_job_status(raw).expect("video status");
    assert_eq!(status.status, "failed");
    assert_eq!(
        status.error_message.as_deref(),
        Some("upstream rejected the request")
    );
}

#[test]
fn videos_creation_parse_error_names_request_id_contract() {
    let raw = r#"{"status":"pending"}"#;

    let err = format_unparseable_video_creation_response("/videos 创建任务响应", raw);

    assert!(err.contains("/videos 创建任务响应无法解析"));
    assert!(err.contains("request_id"));
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
fn videos_parser_extracts_gggb_nested_video_url() {
    let raw = serde_json::json!({
        "status": "done",
        "video": {
            "url": "https://vidgen.x.ai/generated.mp4",
            "duration": 4
        },
        "progress": 100
    })
    .to_string();

    assert_eq!(
        extract_videos_from_video_api_response(&raw),
        vec!["https://vidgen.x.ai/generated.mp4"]
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
