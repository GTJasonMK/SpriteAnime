use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

pub(crate) fn append_text_log(
    log_dir: &Path,
    file_name: &str,
    message: &str,
) -> Result<(), String> {
    std::fs::create_dir_all(log_dir).map_err(|e| format!("创建日志目录失败: {e}"))?;
    let line = format!(
        "{} {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
        message
    );
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_dir.join(file_name))
        .map_err(|e| format!("打开日志文件失败: {e}"))?;
    writeln!(file, "{line}").map_err(|e| format!("写入日志失败: {e}"))
}

pub(crate) fn summarize_log_text(value: &str) -> String {
    const MAX_CHARS: usize = 600;
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.chars().count() <= MAX_CHARS {
        normalized
    } else {
        format!(
            "{}...",
            normalized.chars().take(MAX_CHARS).collect::<String>()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_log_text_collapses_whitespace() {
        assert_eq!(summarize_log_text("  a\n\tb   c  "), "a b c");
    }

    #[test]
    fn summarize_log_text_truncates_long_messages() {
        let text = "x".repeat(650);
        let summarized = summarize_log_text(&text);

        assert_eq!(summarized.chars().count(), 603);
        assert!(summarized.ends_with("..."));
    }
}
