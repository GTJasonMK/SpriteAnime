use serde_json::Value;

pub(super) fn dedupe_images(images: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for image in images {
        if !deduped.iter().any(|item| item == &image) {
            deduped.push(image);
        }
    }
    deduped
}

pub(super) fn endpoint_url(api_base: &str, endpoint: &str) -> String {
    format!("{}/{}", api_base.trim_end_matches('/'), endpoint)
}

pub(super) fn extract_model_ids(value: &Value) -> Vec<String> {
    let mut ids = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        collect_model_ids_from_value(&Value::Array(data.clone()), &mut ids);
    } else if let Some(models) = value.get("models").and_then(Value::as_array) {
        collect_model_ids_from_value(&Value::Array(models.clone()), &mut ids);
    } else if let Some(items) = value.as_array() {
        collect_model_ids_from_value(&Value::Array(items.clone()), &mut ids);
    } else {
        collect_model_ids_from_value(value, &mut ids);
    }
    ids.sort();
    ids.dedup();
    ids
}

fn collect_model_ids_from_value(value: &Value, ids: &mut Vec<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(id) = item.as_str() {
                    ids.push(id.to_string());
                } else {
                    collect_model_ids_from_value(item, ids);
                }
            }
        }
        Value::Object(map) => {
            for key in ["id", "name", "model", "model_id", "modelId", "model_name"] {
                if let Some(id) = map.get(key).and_then(Value::as_str) {
                    ids.push(id.to_string());
                }
            }
            for child in map.values() {
                collect_model_ids_from_value(child, ids);
            }
        }
        _ => {}
    }
}
