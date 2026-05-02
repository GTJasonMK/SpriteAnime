use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

/// 生成日志条目
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerationLog {
    pub timestamp: String,
    // 输入参数
    pub model: String,
    pub style: String,
    pub ratio: String,
    pub resolution: String,
    pub count: u32,
    // 提示词
    pub prompt: String,
    pub negative_prompt: String,
    pub full_prompt: String,
    // 结果
    pub success: bool,
    pub image_paths: Vec<String>,
    pub duration_seconds: f64,
    pub save_dir: String,
}

/// JSON Lines 日志写入器
pub struct JsonLinesLogger {
    log_file: PathBuf,
}

impl JsonLinesLogger {
    /// 创建日志写入器
    pub fn new(log_dir: &PathBuf) -> Result<Self, String> {
        std::fs::create_dir_all(log_dir).map_err(|e| format!("创建日志目录失败: {}", e))?;
        let log_file = log_dir.join("generation.log");
        Ok(Self { log_file })
    }

    /// 追加一条日志记录
    pub fn append(&self, log: &GenerationLog) -> Result<(), String> {
        let line = serde_json::to_string(log).map_err(|e| format!("序列化日志失败: {}", e))?;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
            .map_err(|e| format!("打开日志文件失败: {}", e))?;

        writeln!(file, "{}", line).map_err(|e| format!("写入日志失败: {}", e))?;
        Ok(())
    }
}
