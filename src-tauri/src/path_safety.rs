use std::path::Path;

/// 将用户提供的本地文件名片段转换为可安全落盘的名称。
///
/// 保留 Unicode 和普通标点，只替换控制字符及各平台常见的非法路径字符；
/// 首尾只有分隔符或空白时视为空名称，由调用方返回错误。
pub fn sanitize_file_name_component(value: &str) -> String {
    let sanitized: String = value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
            {
                '_'
            } else {
                ch
            }
        })
        .collect();

    sanitized
        .trim_matches(|ch: char| ch == '.' || ch == '_' || ch == '-' || ch.is_whitespace())
        .to_string()
}

pub fn required_file_name(path: &Path, context: &str, resolution: &str) -> Result<String, String> {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| !name.trim().is_empty())
        .ok_or_else(|| format!("{context}缺少文件名。解决方法：{resolution}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_unicode_and_replaces_path_characters() {
        assert_eq!(
            sanitize_file_name_component(" 角色:奔跑/循环 "),
            "角色_奔跑_循环"
        );
    }

    #[test]
    fn separator_only_name_becomes_empty() {
        assert!(sanitize_file_name_component(" ._-_ ").is_empty());
    }

    #[test]
    fn required_file_name_enforces_the_shared_path_contract() {
        assert_eq!(
            required_file_name(Path::new("/tmp/image.png"), "图片", "请重新选择。").unwrap(),
            "image.png"
        );
        let err = required_file_name(Path::new("/"), "图片", "请重新选择。").unwrap_err();
        assert_eq!(err, "图片缺少文件名。解决方法：请重新选择。");
    }
}
