use super::*;

fn temp_config_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "sprite-anime-config-{name}-{}.json",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ))
}

#[test]
fn ratio_parser_accepts_and_normalizes_positive_dimensions() {
    assert_eq!(get_ratio_tuple("2:3").unwrap(), (2, 3));
    assert_eq!(get_ratio_tuple("2:2").unwrap(), (1, 1));
    assert_eq!(get_ratio_tuple("4:2").unwrap(), (2, 1));
    assert_eq!(get_ratio_tuple("4:1").unwrap(), (4, 1));
    assert!(get_ratio_tuple("0:1").unwrap_err().contains("必须大于 0"));
    assert!(get_ratio_tuple("square").unwrap_err().contains("格式无效"));
}

#[test]
fn serialized_ratio_presets_expose_only_the_frontend_key() {
    let value = serde_json::to_value(get_presets()).unwrap();
    let ratio = value["ratios"][0].as_object().unwrap();

    assert_eq!(ratio.len(), 1);
    assert!(ratio.contains_key("key"));
}

#[test]
fn style_suffix_rejects_unknown_style() {
    assert!(get_style_suffix("anime").is_ok());
    assert_eq!(
        get_style_suffix("missing-style").unwrap_err(),
        "生成风格不存在：missing-style"
    );
}

#[test]
fn current_config_schema_rejects_legacy_top_level_api_fields() {
    let err = serde_json::from_str::<UserConfig>(r#"{"api_key":"secret"}"#).unwrap_err();

    assert!(err.to_string().contains("unknown field `api_key`"));
}

#[test]
fn current_config_schema_rejects_unknown_fields() {
    let mut top_level = serde_json::to_value(UserConfig::default()).unwrap();
    top_level["obsolete_setting"] = serde_json::json!(true);
    let top_level_err = serde_json::from_value::<UserConfig>(top_level).unwrap_err();
    assert!(top_level_err
        .to_string()
        .contains("unknown field `obsolete_setting`"));

    let mut profile = serde_json::to_value(UserConfig::default()).unwrap();
    profile["api_profiles"][0]["obsolete_mode"] = serde_json::json!("auto");
    let profile_err = serde_json::from_value::<UserConfig>(profile).unwrap_err();
    assert!(profile_err
        .to_string()
        .contains("unknown field `obsolete_mode`"));
}

#[test]
fn current_config_schema_requires_prompt_optimizer_mode() {
    let mut value = serde_json::to_value(UserConfig::default()).unwrap();
    value["api_profiles"][0]
        .as_object_mut()
        .unwrap()
        .remove("prompt_optimizer_api_mode");

    let err = serde_json::from_value::<UserConfig>(value).unwrap_err();
    assert!(err
        .to_string()
        .contains("missing field `prompt_optimizer_api_mode`"));
}

#[test]
fn normalize_api_profiles_trims_current_schema_values() {
    let mut config = UserConfig::default();
    let profile = &mut config.api_profiles[0];
    profile.id = " default ".into();
    profile.name = " Primary ".into();
    profile.api_key = " secret ".into();
    profile.api_base = " https://api.example/v1 ".into();
    profile.last_model = " image-model ".into();
    profile.prompt_optimizer_api_mode = " chat_completions ".into();
    config.active_api_profile_id = " default ".into();

    config.normalize_api_profiles().unwrap();

    let profile = &config.api_profiles[0];
    assert_eq!(profile.id, "default");
    assert_eq!(profile.name, "Primary");
    assert_eq!(profile.api_key, "secret");
    assert_eq!(profile.api_base, "https://api.example/v1");
    assert_eq!(profile.last_model, "image-model");
    assert_eq!(profile.prompt_optimizer_api_mode, "chat_completions");
    assert_eq!(config.active_api_profile_id, "default");
}

#[test]
fn normalize_api_profiles_rejects_empty_profile_list() {
    let mut config = UserConfig::default();
    config.api_profiles.clear();

    let err = config.normalize_api_profiles().unwrap_err();

    assert!(err.contains("API 配置组为空"));
}

#[test]
fn normalize_api_profiles_rejects_duplicate_ids() {
    let mut config = UserConfig::default();
    let mut duplicate = config.api_profiles[0].clone();
    duplicate.name = "Duplicate".into();
    config.api_profiles.push(duplicate);

    let err = config.normalize_api_profiles().unwrap_err();

    assert!(err.contains("API 配置 id 重复：default"));
}

#[test]
fn normalize_api_profiles_rejects_missing_active_profile() {
    let mut config = UserConfig {
        active_api_profile_id: "missing".into(),
        ..UserConfig::default()
    };

    let err = config.normalize_api_profiles().unwrap_err();

    assert!(err.contains("活动 API 配置不存在：missing"));
}

#[test]
fn normalize_api_profiles_rejects_empty_identity_fields() {
    let mut missing_id = UserConfig::default();
    missing_id.api_profiles[0].id.clear();
    assert!(missing_id
        .normalize_api_profiles()
        .unwrap_err()
        .contains("缺少 id"));

    let mut missing_name = UserConfig::default();
    missing_name.api_profiles[0].name = " ".into();
    assert!(missing_name
        .normalize_api_profiles()
        .unwrap_err()
        .contains("缺少名称"));
}

#[test]
fn normalize_api_profiles_rejects_invalid_modes() {
    let mut image_config = UserConfig::default();
    image_config.api_profiles[0].name = "Work".into();
    image_config.api_profiles[0].generation_api_mode = "chat/completions".into();
    let image_err = image_config.normalize_api_profiles().unwrap_err();
    assert!(image_err.contains("API 配置「Work」图片生成调用方式无效：chat/completions"));

    let mut video_config = UserConfig::default();
    video_config.api_profiles[0].name = "Video".into();
    video_config.api_profiles[0].video_api_mode = "v1/videos".into();
    let video_err = video_config.normalize_api_profiles().unwrap_err();
    assert!(video_err.contains("API 配置「Video」视频生成调用方式无效：v1/videos"));

    let mut optimizer_config = UserConfig::default();
    optimizer_config.api_profiles[0].name = "Optimizer".into();
    optimizer_config.api_profiles[0].prompt_optimizer_api_mode = "auto".into();
    let optimizer_err = optimizer_config.normalize_api_profiles().unwrap_err();
    assert!(optimizer_err.contains("API 配置「Optimizer」提示词优化调用方式无效：auto"));
}

#[test]
fn normalize_api_profiles_accepts_all_documented_media_modes() {
    for image_mode in [
        "responses",
        "chat_completions",
        "images_generations",
        "images_edits_json",
        "images_edits_multipart",
    ] {
        assert_eq!(
            parse_generation_api_mode(image_mode, "图片模式")
                .unwrap()
                .as_str(),
            image_mode
        );
    }
    for video_mode in [
        "chat_completions",
        "videos",
        "videos_generations",
        "videos_edits",
        "videos_extensions",
    ] {
        assert_eq!(
            parse_video_api_mode(video_mode, "视频模式")
                .unwrap()
                .as_str(),
            video_mode
        );
    }

    for optimizer_mode in ["responses", "chat_completions"] {
        assert_eq!(
            parse_prompt_optimizer_api_mode(optimizer_mode, "优化模式")
                .unwrap()
                .as_str(),
            optimizer_mode
        );
    }
}

#[test]
fn serialized_profile_contains_optimizer_mode() {
    let value = serde_json::to_value(UserConfig::default()).unwrap();

    assert_eq!(
        value["api_profiles"][0]["prompt_optimizer_api_mode"],
        "chat_completions"
    );
}

#[test]
fn load_missing_config_returns_default_config() {
    let path = temp_config_file("missing");

    let config = UserConfig::load(&path).unwrap();

    assert_eq!(config.active_api_profile_id, "default");
    assert_eq!(config.api_profiles.len(), 1);
    assert_eq!(config.api_profiles[0].last_model, "gpt-5.3-codex");
    assert_eq!(config.api_profiles[0].video_model, "sora-2");
    assert!(!path.exists());
}

#[test]
fn load_valid_current_config_normalizes_profiles() {
    let path = temp_config_file("valid");
    let mut config = UserConfig::default();
    config.api_profiles[0].api_key = " secret ".into();
    config.api_profiles[0].api_base = " https://api.example/v1 ".into();
    std::fs::write(&path, serde_json::to_string_pretty(&config).unwrap()).unwrap();

    let loaded = UserConfig::load(&path).unwrap();

    let _ = std::fs::remove_file(path);
    assert_eq!(loaded.api_profiles[0].api_key, "secret");
    assert_eq!(loaded.api_profiles[0].api_base, "https://api.example/v1");
}

#[test]
fn serializing_user_config_contains_only_current_schema() {
    let json = serde_json::to_value(UserConfig::default()).unwrap();
    let object = json.as_object().unwrap();

    assert_eq!(
        object.keys().cloned().collect::<Vec<_>>(),
        vec![
            "active_api_profile_id",
            "api_profiles",
            "ffmpeg_path",
            "ffprobe_path",
            "last_count",
            "last_ratio",
            "last_resolution",
            "last_style",
            "prompt_history",
        ]
    );
}

#[test]
fn load_invalid_config_returns_actionable_error_without_overwriting_file() {
    let path = temp_config_file("invalid-json");
    let original = "{not valid json";
    std::fs::write(&path, original).unwrap();

    let err = UserConfig::load(&path).unwrap_err();
    let after = std::fs::read_to_string(&path).unwrap();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("读取配置文件失败"));
    assert!(err.contains("JSON 解析失败"));
    assert!(err.contains("备份并修复"));
    assert!(err.contains("手动删除"));
    assert_eq!(after, original);
}
