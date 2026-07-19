use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const MAX_WORKBENCH_RECORDS: usize = 500;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchRecord {
    pub id: String,
    pub path: String,
    pub label: String,
    pub prompt: String,
    pub model: String,
    pub duration_seconds: Option<f64>,
    pub created_at: String,
    pub updated_at: String,
}

pub struct WorkbenchStore {
    records_file: PathBuf,
}

impl WorkbenchStore {
    pub fn new(records_file: PathBuf) -> Self {
        Self { records_file }
    }

    pub fn read_all(&self) -> Result<Vec<WorkbenchRecord>, String> {
        let content = match std::fs::read_to_string(&self.records_file) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => {
                return Err(build_read_error(
                    &self.records_file,
                    &format!("读取失败：{err}"),
                ))
            }
        };
        let mut records =
            serde_json::from_str::<Vec<WorkbenchRecord>>(&content).map_err(|err| {
                build_read_error(&self.records_file, &format!("JSON 解析失败：{err}"))
            })?;
        normalize_records(&mut records)
            .map_err(|err| build_read_error(&self.records_file, &err))?;
        Ok(records)
    }

    pub fn read_recent(&self, limit: usize) -> Result<Vec<WorkbenchRecord>, String> {
        if limit == 0 {
            return Err("工作台记录读取数量必须大于 0".into());
        }
        let records = self.read_all()?;
        if records.len() <= limit {
            return Ok(records);
        }
        Ok(records[records.len() - limit..].to_vec())
    }

    pub fn upsert_many(
        &self,
        records: Vec<WorkbenchRecord>,
    ) -> Result<Vec<WorkbenchRecord>, String> {
        let mut existing = self.read_all()?;
        for (index, mut record) in records.into_iter().enumerate() {
            record.path = record.path.trim().to_string();
            if record.path.is_empty() {
                return Err(format!(
                    "待写入工作台记录第{}条缺少图片路径。解决方法：请重新添加带完整路径的本地图片。",
                    index + 1
                ));
            }
            normalize_record(&mut record)
                .map_err(|err| format!("待写入工作台记录第{}条无效：{err}", index + 1))?;

            if let Some(index) = existing
                .iter()
                .position(|item| item.id == record.id || item.path == record.path)
            {
                record.created_at = existing[index].created_at.clone();
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
        let mut records = self.read_all()?;
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

fn build_read_error(path: &Path, reason: &str) -> String {
    format!(
        "读取工作台记录失败：{}。原因：{}。解决方法：请关闭应用，备份并修复该 JSON 文件；如果不需要保留旧工作台记录，请手动删除该文件后重启应用。",
        path.display(),
        reason
    )
}

fn normalize_records(records: &mut [WorkbenchRecord]) -> Result<(), String> {
    for (index, record) in records.iter_mut().enumerate() {
        record.path = record.path.trim().to_string();
        if record.path.is_empty() {
            return Err(format!("第{}条记录缺少图片路径", index + 1));
        }
        normalize_record(record).map_err(|err| format!("第{}条记录无效：{err}", index + 1))?;
    }
    Ok(())
}

fn trim_old_records(records: &mut Vec<WorkbenchRecord>) {
    if records.len() > MAX_WORKBENCH_RECORDS {
        let remove_count = records.len() - MAX_WORKBENCH_RECORDS;
        records.drain(0..remove_count);
    }
}

fn normalize_record(record: &mut WorkbenchRecord) -> Result<(), String> {
    record.id = record.id.trim().to_string();
    if record.id.is_empty() {
        return Err("工作台记录 ID 为空".into());
    }
    record.label = required_label_for_record(&record.label, &record.path)?;
    record.prompt = record.prompt.trim().to_string();
    record.model = required_model_for_record(&record.model, &record.path)?;
    if let Some(value) = record.duration_seconds {
        if !value.is_finite() || value < 0.0 {
            return Err("工作台记录耗时无效".into());
        }
        record.duration_seconds = Some((value * 100.0).round() / 100.0);
    }
    record.created_at = record.created_at.trim().to_string();
    record.updated_at = record.updated_at.trim().to_string();
    if record.created_at.is_empty() || record.updated_at.is_empty() {
        return Err("工作台记录创建或更新时间为空".into());
    }
    Ok(())
}

fn required_label_for_record(label: &str, path: &str) -> Result<String, String> {
    let label = label.trim();
    if label.is_empty() {
        return Err(format!(
            "工作台记录缺少标签：{path}。解决方法：请重新添加该图片，或修复工作台记录 JSON 中的 label。"
        ));
    }
    Ok(label.to_string())
}

fn required_model_for_record(model: &str, path: &str) -> Result<String, String> {
    let model = model.trim();
    if model.is_empty() {
        return Err(format!(
            "工作台记录缺少模型或来源信息：{path}。解决方法：请重新添加或重新生成该图片，或修复工作台记录 JSON 中的 model。"
        ));
    }
    Ok(model.to_string())
}

#[cfg(test)]
mod tests;
