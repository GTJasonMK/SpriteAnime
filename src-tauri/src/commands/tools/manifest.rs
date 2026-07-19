#[derive(Debug, Clone, Copy)]
pub(super) struct FfmpegArchive {
    pub(super) url: &'static str,
    pub(super) file_name: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FfmpegDownloadManifest {
    pub(super) platform_dir: &'static str,
    pub(super) source: &'static str,
    pub(super) archives: &'static [FfmpegArchive],
}

const WIN64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-lgpl.zip",
    file_name: "ffmpeg-master-latest-win64-lgpl.zip",
}];

const WINARM64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-winarm64-lgpl.zip",
    file_name: "ffmpeg-master-latest-winarm64-lgpl.zip",
}];

const LINUX64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linux64-lgpl.tar.xz",
    file_name: "ffmpeg-master-latest-linux64-lgpl.tar.xz",
}];

const LINUXARM64_ARCHIVES: &[FfmpegArchive] = &[FfmpegArchive {
    url: "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-linuxarm64-lgpl.tar.xz",
    file_name: "ffmpeg-master-latest-linuxarm64-lgpl.tar.xz",
}];

const MACOS_AMD64_ARCHIVES: &[FfmpegArchive] = &[
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffmpeg.zip",
        file_name: "ffmpeg-macos-amd64.zip",
    },
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/amd64/release/ffprobe.zip",
        file_name: "ffprobe-macos-amd64.zip",
    },
];

const MACOS_ARM64_ARCHIVES: &[FfmpegArchive] = &[
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffmpeg.zip",
        file_name: "ffmpeg-macos-arm64.zip",
    },
    FfmpegArchive {
        url: "https://ffmpeg.martin-riedl.de/redirect/latest/macos/arm64/release/ffprobe.zip",
        file_name: "ffprobe-macos-arm64.zip",
    },
];

pub(super) fn current_ffmpeg_download_manifest() -> Option<FfmpegDownloadManifest> {
    ffmpeg_download_manifest_for(std::env::consts::OS, std::env::consts::ARCH)
}

pub(super) fn ffmpeg_download_manifest_for(os: &str, arch: &str) -> Option<FfmpegDownloadManifest> {
    match (os, arch) {
        ("windows", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "windows-x86_64",
            source: "BtbN FFmpeg LGPL static build",
            archives: WIN64_ARCHIVES,
        }),
        ("windows", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "windows-aarch64",
            source: "BtbN FFmpeg LGPL static build",
            archives: WINARM64_ARCHIVES,
        }),
        ("linux", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "linux-x86_64",
            source: "BtbN FFmpeg LGPL static build",
            archives: LINUX64_ARCHIVES,
        }),
        ("linux", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "linux-aarch64",
            source: "BtbN FFmpeg LGPL static build",
            archives: LINUXARM64_ARCHIVES,
        }),
        ("macos", "x86_64") => Some(FfmpegDownloadManifest {
            platform_dir: "macos-x86_64",
            source: "Martin Riedl FFmpeg static build",
            archives: MACOS_AMD64_ARCHIVES,
        }),
        ("macos", "aarch64") => Some(FfmpegDownloadManifest {
            platform_dir: "macos-aarch64",
            source: "Martin Riedl FFmpeg static build",
            archives: MACOS_ARM64_ARCHIVES,
        }),
        _ => None,
    }
}

pub(super) fn executable_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}
