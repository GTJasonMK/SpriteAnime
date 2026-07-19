use super::*;

fn temp_records_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "sprite-anime-{name}-{}.json",
        chrono::Local::now()
            .timestamp_nanos_opt()
            .expect("当前时间应可表示为纳秒时间戳")
    ))
}

#[test]
fn read_rejects_empty_record_id() {
    let path = temp_records_file("empty-id");
    let store = WorkbenchStore::new(path.clone());
    let first_image = temp_records_file("one.png").to_string_lossy().to_string();
    let records = vec![WorkbenchRecord {
        id: String::new(),
        path: first_image,
        label: "one".into(),
        prompt: String::new(),
        model: "手动添加".into(),
        duration_seconds: None,
        created_at: "2026-01-01 00:00:00".into(),
        updated_at: "2026-01-01 00:00:00".into(),
    }];
    store.save_all(&records).unwrap();

    let err = store.read_all().unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("工作台记录 ID 为空"));
}

#[test]
fn read_missing_records_file_returns_empty_list() {
    let path = temp_records_file("missing-read");
    let store = WorkbenchStore::new(path.clone());

    let records = store.read_all().unwrap();

    assert!(records.is_empty());
}

#[test]
fn read_invalid_records_file_returns_actionable_error() {
    let path = temp_records_file("invalid-json");
    std::fs::write(&path, "{not valid json").unwrap();
    let store = WorkbenchStore::new(path.clone());

    let err = store.read_all().unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("读取工作台记录失败"));
    assert!(err.contains("JSON 解析失败"));
    assert!(err.contains("备份并修复"));
    assert!(err.contains("手动删除"));
}

#[test]
fn read_record_without_label_returns_actionable_error_without_path_fallback() {
    let path = temp_records_file("invalid-label");
    let store = WorkbenchStore::new(path.clone());
    let image_path = temp_records_file("image.png").to_string_lossy().to_string();
    let records = vec![WorkbenchRecord {
        id: "invalid-label".into(),
        path: image_path.clone(),
        label: String::new(),
        prompt: String::new(),
        model: "手动添加".into(),
        duration_seconds: None,
        created_at: "2026-01-01 00:00:00".into(),
        updated_at: "2026-01-01 00:00:00".into(),
    }];
    store.save_all(&records).unwrap();

    let err = store.read_all().unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("读取工作台记录失败"));
    assert!(err.contains("工作台记录缺少标签"));
    assert!(err.contains(&image_path));
    assert!(err.contains("修复工作台记录 JSON"));
    assert!(err.contains("label"));
}

#[test]
fn read_record_without_model_returns_actionable_error() {
    let path = temp_records_file("invalid-model");
    let store = WorkbenchStore::new(path.clone());
    let image_path = temp_records_file("image.png").to_string_lossy().to_string();
    let records = vec![WorkbenchRecord {
        id: "invalid-model".into(),
        path: image_path.clone(),
        label: "image".into(),
        prompt: String::new(),
        model: String::new(),
        duration_seconds: None,
        created_at: "2026-01-01 00:00:00".into(),
        updated_at: "2026-01-01 00:00:00".into(),
    }];
    store.save_all(&records).unwrap();

    let err = store.read_all().unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("读取工作台记录失败"));
    assert!(err.contains("工作台记录缺少模型或来源信息"));
    assert!(err.contains(&image_path));
    assert!(err.contains("修复工作台记录 JSON"));
    assert!(err.contains("model"));
}

#[test]
fn upsert_rejects_record_without_path_instead_of_skipping_it() {
    let path = temp_records_file("missing-path-upsert");
    let store = WorkbenchStore::new(path.clone());

    let err = store
        .upsert_many(vec![WorkbenchRecord {
            id: "missing-path".into(),
            path: "   ".into(),
            label: "空路径".into(),
            prompt: String::new(),
            model: String::new(),
            duration_seconds: None,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }])
        .unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("待写入工作台记录第1条缺少图片路径"));
    assert!(err.contains("重新添加带完整路径的本地图片"));
}

#[test]
fn upsert_rejects_record_without_model() {
    let path = temp_records_file("missing-model-upsert");
    let store = WorkbenchStore::new(path.clone());
    let image_path = temp_records_file("new.png").to_string_lossy().to_string();

    let err = store
        .upsert_many(vec![WorkbenchRecord {
            id: "missing-model".into(),
            path: image_path,
            label: "new".into(),
            prompt: String::new(),
            model: "   ".into(),
            duration_seconds: None,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }])
        .unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("待写入工作台记录第1条无效"));
    assert!(err.contains("工作台记录缺少模型或来源信息"));
    assert!(err.contains("重新添加或重新生成"));
}

#[test]
fn upsert_rejects_record_without_label() {
    let path = temp_records_file("missing-label-upsert");
    let store = WorkbenchStore::new(path.clone());
    let image_path = temp_records_file("new.png").to_string_lossy().to_string();

    let err = store
        .upsert_many(vec![WorkbenchRecord {
            id: "missing-label".into(),
            path: image_path.clone(),
            label: "   ".into(),
            prompt: String::new(),
            model: "手动添加".into(),
            duration_seconds: None,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }])
        .unwrap_err();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("待写入工作台记录第1条无效"));
    assert!(err.contains("工作台记录缺少标签"));
    assert!(err.contains(&image_path));
    assert!(err.contains("label"));
}

#[test]
fn upsert_rejects_invalid_existing_records_file_without_overwriting_it() {
    let path = temp_records_file("invalid-upsert");
    let original = "{not valid json";
    std::fs::write(&path, original).unwrap();
    let store = WorkbenchStore::new(path.clone());
    let image_path = temp_records_file("new.png").to_string_lossy().to_string();

    let err = store
        .upsert_many(vec![WorkbenchRecord {
            id: "invalid-existing".into(),
            path: image_path,
            label: String::new(),
            prompt: String::new(),
            model: String::new(),
            duration_seconds: None,
            created_at: "2026-01-01 00:00:00".into(),
            updated_at: "2026-01-01 00:00:00".into(),
        }])
        .unwrap_err();
    let after = std::fs::read_to_string(&path).unwrap();

    let _ = std::fs::remove_file(path);
    assert!(err.contains("读取工作台记录失败"));
    assert_eq!(after, original);
}
