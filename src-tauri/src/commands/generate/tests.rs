use super::*;
use super::{
    image_generation::normalize_reference_image_paths, reference::*, types::ReferenceImagePayload,
};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_image_path(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "sprite_anime_{name}_{}_{}.png",
        std::process::id(),
        stamp
    ))
}

#[test]
fn video_generation_options_reject_values_outside_current_ui_contract() {
    assert_eq!(require_video_size("1280x720").unwrap(), "1280x720");
    assert_eq!(require_video_seconds(8).unwrap(), 8);
    assert_eq!(require_video_size("").unwrap_err(), "视频尺寸无效：");
    assert!(require_video_seconds(20)
        .unwrap_err()
        .contains("文档允许 1 到 15 秒"));
}

#[test]
fn video_edit_and_extension_require_source_video_id() {
    assert!(
        validate_video_mode_inputs(crate::config::VideoApiMode::VideosEdits, "", "", false)
            .unwrap_err()
            .contains("原视频 ID")
    );
    assert!(validate_video_mode_inputs(
        crate::config::VideoApiMode::VideosExtensions,
        "vid_1",
        "forward",
        true
    )
    .is_ok());
    assert!(validate_video_mode_inputs(crate::config::VideoApiMode::Videos, "", "", true).is_ok());
}

#[test]
fn matting_options_reject_unknown_color_mode() {
    assert!(matches!(
        parse_color_key_mode("auto").unwrap(),
        crate::image_processor::ColorKeyMode::Auto
    ));
    assert_eq!(
        parse_color_key_mode("legacy").unwrap_err(),
        "抠图颜色模式无效：legacy"
    );
}

fn decode_data_url_image(data_url: &str) -> image::DynamicImage {
    let (_, b64) = data_url.split_once("base64,").unwrap();
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).unwrap();
    image::load_from_memory(&bytes).unwrap()
}

#[test]
fn require_api_setting_trims_non_empty_values() {
    let value =
        require_api_setting("  https://api.example.com/v1  ".into(), "生图 API 地址", "").unwrap();

    assert_eq!(value, "https://api.example.com/v1");
}

#[test]
fn require_api_setting_reports_missing_image_api_base_with_resolution() {
    let err = require_api_setting("   ".into(), "生图 API 地址", IMAGE_API_SETTINGS_RESOLUTION)
        .unwrap_err();

    assert!(err.contains("生图 API 地址为空"));
    assert!(err.contains("设置 > API 配置 > 图片生成"));
}

#[test]
fn require_api_setting_reports_missing_prompt_optimizer_api_base_with_resolution() {
    let err = require_api_setting(
        "".into(),
        "提示词优化 API 地址",
        PROMPT_OPTIMIZER_API_SETTINGS_RESOLUTION,
    )
    .unwrap_err();

    assert!(err.contains("提示词优化 API 地址为空"));
    assert!(err.contains("设置 > API 配置 > 提示词优化"));
}

#[test]
fn prompt_optimizer_settings_reject_unknown_api_mode() {
    let err = require_prompt_optimizer_api_settings(
        "secret".into(),
        "https://api.example/v1".into(),
        "auto".into(),
        "model".into(),
        "".into(),
    )
    .err()
    .expect("未知提示词优化调用方式应被拒绝");

    assert!(err.contains("提示词优化调用方式无效：auto"));
}

#[test]
fn require_api_setting_reports_missing_video_api_base_with_resolution() {
    let err = require_api_setting(
        "\n\t".into(),
        "视频生成 API 地址",
        VIDEO_API_SETTINGS_RESOLUTION,
    )
    .unwrap_err();

    assert!(err.contains("视频生成 API 地址为空"));
    assert!(err.contains("设置 > API 配置 > 视频生成"));
}

#[test]
fn transparent_command_result_rejects_missing_file_name() {
    let err = transparent_command_result("/".into(), 12).unwrap_err();

    assert!(err.contains("抠图保存结果缺少文件名"));
    assert!(err.contains("重新保存抠图后再使用"));
}

#[test]
fn prompt_optimizer_input_uses_reference_replication_mode() {
    let input = build_prompt_optimizer_input(
        "换成红衣女剑士",
        "低清晰度",
        "像素风",
        "1:1",
        "1024",
        2,
        3,
        true,
        false,
    );

    assert!(input.contains("当前已选择参考图"));
    assert!(input.contains("参考图是动作、构图、网格、姿态和帧间节奏模板"));
    assert!(input.contains("以随请求上传的参考图为唯一动作和构图模板"));
    assert!(input.contains("用用户指定角色替换参考图中的角色"));
    assert!(input.contains("不要重新设计动作"));
    assert!(input.contains("不要写长篇动作阶段"));
    assert!(input.contains("当前界面网格=2行3列，共6帧"));
    assert!(!input.contains("先识别用户真正想要的角色、动作、循环方式"));
}

#[test]
fn prompt_optimizer_input_uses_reference_vision_mode() {
    let input =
        build_prompt_optimizer_input("换成蓝衣骑士", "", "动画", "1:1", "1024", 2, 4, true, true);

    assert!(input.contains("本次提示词优化请求会直接上传这张参考图"));
    assert!(input.contains("视觉参考图复刻模式"));
    assert!(input.contains("先理解参考图的行列布局"));
    assert!(input.contains("不要写长篇图像描述"));
    assert!(input.contains("当前界面网格=2行4列，共8帧"));
}

#[test]
fn reference_vision_error_detects_image_input_errors() {
    assert!(is_reference_vision_input_error(
        "HTTP 400: This model does not support image input"
    ));
    assert!(is_reference_vision_input_error(
        "HTTP 400: invalid type for input content"
    ));
    assert!(!is_reference_vision_input_error(
        "HTTP 401: invalid API key"
    ));
    assert!(!is_reference_vision_input_error(
        "HTTP 429: rate limit exceeded"
    ));
}

#[test]
fn reference_vision_error_includes_user_resolution_steps() {
    let error = build_reference_vision_error(
        "HTTP 400: This model does not support image input",
        "text-only-model",
    );

    assert!(error.contains("参考图视觉理解失败"));
    assert!(error.contains("text-only-model"));
    assert!(error.contains("支持图像输入的多模态模型"));
    assert!(error.contains("关闭“参考图视觉理解”后重试"));
}

#[test]
fn prompt_optimizer_input_keeps_normal_design_mode_without_reference() {
    let input = build_prompt_optimizer_input(
        "小机器人跑步循环",
        "",
        "手绘",
        "1:1",
        "1024",
        4,
        4,
        false,
        false,
    );

    assert!(input.contains("先识别用户真正想要的角色、动作、循环方式"));
    assert!(input.contains("动作设计要用少量“第X-Y帧”阶段覆盖全部帧"));
    assert!(input.contains("当前界面网格=4行4列，共16帧"));
    assert!(!input.contains("当前已选择参考图"));
    assert!(!input.contains("参考图复刻模式"));
}

#[test]
fn prompt_optimizer_result_parser_accepts_strict_json() {
    let result = parse_prompt_optimization_result(
        r#"{"prompt":"  小机器人跑步 sprite sheet  ","negative_prompt":"  模糊  ","grid_rows":3,"grid_cols":4}"#,
    )
    .unwrap();

    assert_eq!(result.prompt, "小机器人跑步 sprite sheet");
    assert_eq!(result.negative_prompt, "模糊");
    assert_eq!(result.grid_rows, 3);
    assert_eq!(result.grid_cols, 4);
}

#[test]
fn prompt_optimizer_result_parser_rejects_markdown_wrapped_json() {
    let err = parse_prompt_optimization_result(
        "```json\n{\"prompt\":\"x\",\"negative_prompt\":\"\",\"grid_rows\":2,\"grid_cols\":4}\n```",
    )
    .unwrap_err();

    assert!(err.contains("提示词优化结果解析失败"));
    assert!(err.contains("必须只返回合法 JSON 对象"));
    assert!(err.contains("不能包含 Markdown"));
}

#[test]
fn prompt_optimizer_result_parser_rejects_missing_required_fields() {
    let err = parse_prompt_optimization_result(
        r#"{"prompt":"小机器人跑步","negative_prompt":"","grid_rows":3}"#,
    )
    .unwrap_err();

    assert!(err.contains("缺少 `grid_cols` 字段"));
    assert!(err.contains("prompt、negative_prompt、grid_rows、grid_cols"));
}

#[test]
fn prompt_optimizer_result_parser_rejects_invalid_grid_size() {
    let err = parse_prompt_optimization_result(
        r#"{"prompt":"小机器人跑步","negative_prompt":"","grid_rows":0,"grid_cols":4}"#,
    )
    .unwrap_err();

    assert!(err.contains("`grid_rows` 必须是 1 到 20 之间的整数"));
    assert!(err.contains("实际为 0"));
}

#[test]
fn prompt_optimizer_result_parser_rejects_extra_fields() {
    let err = parse_prompt_optimization_result(
        r#"{"prompt":"小机器人跑步","negative_prompt":"","grid_rows":3,"grid_cols":4,"note":"done"}"#,
    )
    .unwrap_err();

    assert!(err.contains("提示词优化结果解析失败"));
    assert!(err.contains("额外说明"));
}

#[test]
fn reference_image_data_url_compacts_large_opaque_image_to_jpeg() {
    let path = temp_image_path("opaque");
    let img = image::RgbImage::from_pixel(2048, 1024, image::Rgb([42, 80, 220]));
    image::DynamicImage::ImageRgb8(img).save(&path).unwrap();

    let payload = load_reference_image_payload(path.to_str().unwrap()).unwrap();
    let data_url = &payload.data_url;
    let decoded = decode_data_url_image(data_url);
    let _ = std::fs::remove_file(&path);

    assert!(data_url.starts_with("data:image/jpeg;base64,"));
    assert!(decoded.width().max(decoded.height()) <= REFERENCE_IMAGE_MAX_EDGE);
    assert!(payload.label.contains("jpeg-q86"));
}

#[test]
fn reference_image_data_url_keeps_small_transparent_image_as_png() {
    let path = temp_image_path("transparent");
    let mut img = image::RgbaImage::from_pixel(32, 32, image::Rgba([220, 42, 42, 0]));
    img.put_pixel(16, 16, image::Rgba([220, 42, 42, 255]));
    image::DynamicImage::ImageRgba8(img).save(&path).unwrap();

    let payload = load_reference_image_payload(path.to_str().unwrap()).unwrap();
    let data_url = &payload.data_url;
    let decoded = decode_data_url_image(data_url);
    let _ = std::fs::remove_file(&path);

    assert!(data_url.starts_with("data:image/png;base64,"));
    assert_eq!(decoded.width(), 32);
    assert_eq!(decoded.height(), 32);
    assert!(payload.label.starts_with("png "));
}

#[test]
fn reference_generation_error_includes_payload_resolution_steps() {
    let payload = ReferenceImagePayload {
        data_url: "data:image/jpeg;base64,abc".into(),
        bytes: vec![1, 2, 3],
        mime: "image/jpeg",
        label: "jpeg-q86 2048x1024 -> 1024x512 image/jpeg 12.0 KiB".into(),
    };

    let error = build_reference_generation_error(
        "HTTP 413: payload too large".into(),
        std::slice::from_ref(&payload),
    );

    assert!(error.contains("参考图生图请求失败"));
    assert!(error.contains(&payload.label));
    assert!(error.contains("手动缩小参考图"));
    assert!(error.contains("移除参考图后重试"));
}

#[test]
fn image_size_rejects_unknown_resolution_before_generation() {
    let err = compute_image_size("auto", (1, 1)).unwrap_err();
    assert_eq!(err, "图片分辨率无效：auto");
}

#[test]
fn image_reference_paths_accept_two_ordered_distinct_paths() {
    let paths =
        normalize_reference_image_paths(vec![" target.png ".into(), "anchor.png".into()]).unwrap();
    assert_eq!(paths, vec!["target.png", "anchor.png"]);
}

#[test]
fn image_reference_paths_reject_empty_duplicate_and_excess_items() {
    assert!(normalize_reference_image_paths(vec!["".into()])
        .unwrap_err()
        .contains("路径为空"));
    assert!(
        normalize_reference_image_paths(vec!["same.png".into(), " same.png ".into()])
            .unwrap_err()
            .contains("路径重复")
    );
    assert!(
        normalize_reference_image_paths(vec!["a".into(), "b".into(), "c".into()])
            .unwrap_err()
            .contains("最多支持 2 张")
    );
}
