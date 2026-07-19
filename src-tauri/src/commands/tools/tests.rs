use super::*;

#[test]
fn configured_ffmpeg_tools_trims_explicit_paths() {
    let config = UserConfig {
        ffmpeg_path: " /opt/video/bin/ffmpeg ".into(),
        ffprobe_path: " /opt/video/bin/ffprobe ".into(),
        ..Default::default()
    };

    let tools = configured_ffmpeg_tools(&config).unwrap();

    assert_eq!(tools.ffmpeg, "/opt/video/bin/ffmpeg");
    assert_eq!(tools.ffprobe, "/opt/video/bin/ffprobe");
}

#[test]
fn configured_ffmpeg_tools_requires_both_paths_without_deriving_ffprobe() {
    let config = UserConfig {
        ffmpeg_path: "/opt/video/bin/ffmpeg".into(),
        ffprobe_path: String::new(),
        ..Default::default()
    };

    let err = configured_ffmpeg_tools(&config).unwrap_err();

    assert!(err.contains("FFprobe 路径未配置"));
    assert!(err.contains("下载并配置"));
}

#[test]
fn configured_ffmpeg_tools_reports_missing_pair() {
    let config = UserConfig::default();

    let err = configured_ffmpeg_tools(&config).unwrap_err();

    assert!(err.contains("FFmpeg/FFprobe 路径未配置"));
    assert!(err.contains("同时填写"));
}

#[test]
fn ffmpeg_download_error_body_read_failure_has_resolution_steps() {
    let err = format_ffmpeg_download_error_body_read_failure(
        reqwest::StatusCode::BAD_GATEWAY,
        "connection closed",
    );

    assert!(err.contains("下载 FFmpeg 失败，HTTP 502 Bad Gateway"));
    assert!(err.contains("读取错误响应体失败"));
    assert!(err.contains("检查网络连接、代理配置"));
    assert!(err.contains("下载源是否提前断开连接"));
}

#[test]
fn ffmpeg_download_manifest_supports_main_desktop_targets() {
    let linux = ffmpeg_download_manifest_for("linux", "x86_64").unwrap();
    assert_eq!(linux.platform_dir, "linux-x86_64");
    assert!(linux.archives[0].file_name.ends_with(".tar.xz"));

    let windows = ffmpeg_download_manifest_for("windows", "x86_64").unwrap();
    assert_eq!(windows.platform_dir, "windows-x86_64");
    assert!(windows.archives[0].file_name.ends_with(".zip"));

    let macos = ffmpeg_download_manifest_for("macos", "aarch64").unwrap();
    assert_eq!(macos.platform_dir, "macos-aarch64");
    assert_eq!(macos.archives.len(), 2);
}

#[test]
fn ffmpeg_download_manifest_rejects_unsupported_arch() {
    assert!(ffmpeg_download_manifest_for("linux", "riscv64").is_none());
}
