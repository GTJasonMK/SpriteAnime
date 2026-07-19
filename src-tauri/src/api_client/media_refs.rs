pub(super) fn trim_markdown_code_fence(value: &str) -> &str {
    let value = value.trim();
    if !value.starts_with("```") {
        return value;
    }
    let Some(first_newline) = value.find('\n') else {
        return value;
    };
    let body = &value[first_newline + 1..];
    body.trim_end_matches("```").trim()
}

pub(super) fn extract_data_image_urls(value: &str) -> Vec<String> {
    extract_data_urls(value, "data:image/")
}

pub(super) fn extract_data_video_urls(value: &str) -> Vec<String> {
    extract_data_urls(value, "data:video/")
}

fn extract_data_urls(value: &str, marker: &str) -> Vec<String> {
    let mut urls = Vec::new();
    let mut rest = value;
    while let Some(start) = rest.find(marker) {
        let candidate = &rest[start..];
        let end = candidate
            .find(|ch: char| {
                ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ')' || ch == ']'
            })
            .unwrap_or(candidate.len());
        urls.push(
            candidate[..end]
                .trim_end_matches(['.', ',', ';'])
                .to_string(),
        );
        rest = &candidate[end..];
    }
    urls
}

pub(super) fn extract_http_urls(value: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for marker in ["http://", "https://"] {
        let mut rest = value;
        while let Some(start) = rest.find(marker) {
            let candidate = &rest[start..];
            let end = candidate
                .find(|ch: char| {
                    ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ')' || ch == ']'
                })
                .unwrap_or(candidate.len());
            urls.push(
                candidate[..end]
                    .trim_end_matches(['.', ',', ';', ':'])
                    .to_string(),
            );
            rest = &candidate[end..];
        }
    }
    urls
}

pub(super) fn looks_like_image_ref(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("data:image/")
        || looks_like_http_url(trimmed)
        || looks_like_base64_image_payload(trimmed)
}

pub(super) fn looks_like_video_ref(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.starts_with("data:video/")
        || looks_like_video_url(trimmed)
        || looks_like_base64_video_payload(trimmed)
}

pub(super) fn looks_like_http_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn looks_like_video_url(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    looks_like_http_url(&lower)
        && (lower.contains(".mp4")
            || lower.contains(".webm")
            || lower.contains(".mov")
            || lower.contains(".m4v")
            || lower.contains(".mpeg")
            || lower.contains(".mpg"))
}

pub(super) fn looks_like_image_url_or_signed_url(url: &str, surrounding_text: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".png")
        || lower.contains(".jpg")
        || lower.contains(".jpeg")
        || lower.contains(".webp")
        || lower.contains(".gif")
    {
        return true;
    }
    let trimmed = surrounding_text.trim();
    trimmed == url || trimmed.contains("image_url") || trimmed.contains("b64_json")
}

pub(super) fn looks_like_video_url_or_signed_url(url: &str, surrounding_text: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    if lower.contains(".mp4")
        || lower.contains(".webm")
        || lower.contains(".mov")
        || lower.contains(".m4v")
        || lower.contains(".mpeg")
        || lower.contains(".mpg")
    {
        return true;
    }
    let trimmed = surrounding_text.trim();
    trimmed == url
        || trimmed.contains("video_url")
        || trimmed.contains("videoUrl")
        || trimmed.contains("download_url")
        || trimmed.contains("downloadUrl")
}

fn looks_like_base64_image_payload(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 128
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn looks_like_base64_video_payload(value: &str) -> bool {
    let trimmed = value.trim();
    trimmed.len() >= 512
        && trimmed
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}
