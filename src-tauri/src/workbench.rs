use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

const MAX_WORKBENCH_RECORDS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchRecord {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub duration_seconds: Option<f64>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
}

pub struct WorkbenchStore {
    records_file: PathBuf,
}

impl WorkbenchStore {
    pub fn new(records_file: PathBuf) -> Self {
        Self { records_file }
    }

    pub fn read_all(&self) -> Vec<WorkbenchRecord> {
        let Ok(content) = std::fs::read_to_string(&self.records_file) else {
            return Vec::new();
        };
        let mut records =
            serde_json::from_str::<Vec<WorkbenchRecord>>(&content).unwrap_or_default();
        normalize_records(&mut records);
        records
    }

    pub fn read_recent(&self, limit: usize) -> Vec<WorkbenchRecord> {
        let records = self.read_all();
        if limit == 0 || records.len() <= limit {
            return records;
        }
        records[records.len() - limit..].to_vec()
    }

    pub fn upsert_many(
        &self,
        records: Vec<WorkbenchRecord>,
    ) -> Result<Vec<WorkbenchRecord>, String> {
        let mut existing = self.read_all();
        let now = current_time_string();

        for mut record in records {
            record.path = record.path.trim().to_string();
            if record.path.is_empty() {
                continue;
            }
            normalize_record(&mut record, &now);

            if let Some(index) = existing
                .iter()
                .position(|item| item.id == record.id || item.path == record.path)
            {
                let created_at = if existing[index].created_at.is_empty() {
                    record.created_at.clone()
                } else {
                    existing[index].created_at.clone()
                };
                if record.duration_seconds.is_none() {
                    record.duration_seconds = existing[index].duration_seconds;
                }
                record.created_at = created_at;
                existing[index] = record;
            } else {
                existing.push(record);
            }
        }

        trim_old_records(&mut existing);
        self.save_all(&existing)?;
        Ok(existing)
    }

    pub fn delete(&self, id: &str) -> Result<Vec<WorkbenchRecord>, String> {
        let id = id.trim();
        if id.is_empty() {
            return Err("记录ID为空".into());
        }
        let mut records = self.read_all();
        let before = records.len();
        records.retain(|item| item.id != id);
        if records.len() == before {
            return Err("记录不存在".into());
        }
        self.save_all(&records)?;
        Ok(records)
    }

    pub fn clear(&self) -> Result<(), String> {
        self.save_all(&[])
    }

    fn save_all(&self, records: &[WorkbenchRecord]) -> Result<(), String> {
        if let Some(parent) = self.records_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建记录目录失败: {}", e))?;
        }
        let json = serde_json::to_string_pretty(records)
            .map_err(|e| format!("序列化工作台记录失败: {}", e))?;
        std::fs::write(&self.records_file, json).map_err(|e| format!("写入工作台记录失败: {}", e))
    }
}

fn normalize_records(records: &mut Vec<WorkbenchRecord>) {
    let now = current_time_string();
    records.retain_mut(|record| {
        record.path = record.path.trim().to_string();
        if record.path.is_empty() {
            return false;
        }
        normalize_record(record, &now);
        true
    });
}

fn trim_old_records(records: &mut Vec<WorkbenchRecord>) {
    if records.len() > MAX_WORKBENCH_RECORDS {
        let remove_count = records.len() - MAX_WORKBENCH_RECORDS;
        records.drain(0..remove_count);
    }
}

fn normalize_record(record: &mut WorkbenchRecord, now: &str) {
    if record.id.trim().is_empty() {
        record.id = stable_record_id(&record.path);
    }
    if record.label.trim().is_empty() {
        record.label = default_label_for_path(&record.path);
    } else {
        record.label = record.label.trim().to_string();
    }
    record.prompt = record.prompt.trim().to_string();
    record.model = record.model.trim().to_string();
    record.duration_seconds = record
        .duration_seconds
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| (value * 100.0).round() / 100.0);
    if record.created_at.trim().is_empty() {
        record.created_at = now.to_string();
    }
    if record.updated_at.trim().is_empty() {
        record.updated_at = now.to_string();
    }
}

fn stable_record_id(path: &str) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("img-{:016x}", hasher.finish())
}

fn default_label_for_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(|| "未命名图片".into())
}

fn current_time_string() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_records_file(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "sprite-anime-{name}-{}.json",
            chrono::Local::now()
                .timestamp_nanos_opt()
                .unwrap_or_default()
        ))
    }

    #[test]
    fn delete_rejects_empty_id_without_removing_records() {
        let path = temp_records_file("empty-delete");
        let store = WorkbenchStore::new(path.clone());
        let records = vec![
            WorkbenchRecord {
                id: String::new(),
                path: "/tmp/one.png".into(),
                label: String::new(),
                prompt: String::new(),
                model: String::new(),
                duration_seconds: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            WorkbenchRecord {
                id: String::new(),
                path: "/tmp/two.png".into(),
                label: String::new(),
                prompt: String::new(),
                model: String::new(),
                duration_seconds: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
        ];
        store.save_all(&records).unwrap();

        let result = store.delete("");
        let remaining = store.read_all();

        let _ = std::fs::remove_file(path);
        assert!(result.is_err());
        assert_eq!(remaining.len(), 2);
        assert!(remaining.iter().all(|record| !record.id.is_empty()));
    }

    #[test]
    fn delete_can_remove_normalized_legacy_empty_id_record() {
        let path = temp_records_file("legacy-delete");
        let store = WorkbenchStore::new(path.clone());
        let records = vec![WorkbenchRecord {
            id: String::new(),
            path: "/tmp/legacy.png".into(),
            label: String::new(),
            prompt: String::new(),
            model: String::new(),
            duration_seconds: None,
            created_at: String::new(),
            updated_at: String::new(),
        }];
        store.save_all(&records).unwrap();
        let id = store.read_all()[0].id.clone();

        let remaining = store.delete(&id).unwrap();

        let _ = std::fs::remove_file(path);
        assert!(remaining.is_empty());
    }
}
