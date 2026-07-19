use std::path::Path;
use tauri::ipc::Channel;
use tauri::{command, State};

use crate::api_client;
use crate::asset_library::{self, AssetCategory};
use crate::config::{self, AppState};
use crate::events::VideoGenerationEvent;
use crate::path_safety::required_file_name;

use super::config_commands::{require_api_setting, VIDEO_API_SETTINGS_RESOLUTION};
use super::reference::load_reference_image_payload;
use super::types::{GenerateVideoRequest, GeneratedVideoResult};

/// 使用当前 API 配置调用视频生成模型，保存 MP4 并返回本地路径。
#[command]
pub async fn generate_video(
    state: State<'_, AppState>,
    channel: Channel<VideoGenerationEvent>,
    request: GenerateVideoRequest,
) -> Result<GeneratedVideoResult, String> {
    let progress = VideoChannelProgress { channel: &channel };
    generate_video_inner(&state, &progress, request, None).await
}

pub(crate) async fn generate_video_inner(
    state: &AppState,
    progress: &dyn crate::runtime::ProgressReporter,
    request: GenerateVideoRequest,
    output_dir: Option<std::path::PathBuf>,
) -> Result<GeneratedVideoResult, String> {
    let start_time = std::time::Instant::now();
    let prompt = request.prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("请输入视频生成提示词".into());
    }
    if prompt.chars().count() > 4096 {
        return Err("视频提示词不能超过 4096 个字符".into());
    }

    let api_key = require_api_setting(
        request.api_key,
        "视频生成 API Key",
        VIDEO_API_SETTINGS_RESOLUTION,
    )?;
    let api_base = require_api_setting(
        request.api_base,
        "视频生成 API 地址",
        VIDEO_API_SETTINGS_RESOLUTION,
    )?;
    let proxy_url = request.proxy_url.trim().to_string();
    let model = require_api_setting(request.model, "视频生成模型", VIDEO_API_SETTINGS_RESOLUTION)?;
    let api_mode = config::parse_video_api_mode(&request.api_mode, "视频生成调用方式")?;
    let save_dir = match output_dir {
        Some(path) => {
            std::fs::create_dir_all(&path)
                .map_err(|error| format!("创建视频输出目录失败: {error}"))?;
            path
        }
        None => {
            asset_library::category_dir(&state.default_save_dir, AssetCategory::GeneratedVideos)?
        }
    }
    .to_string_lossy()
    .to_string();

    let size = require_video_size(&request.size)?;
    let seconds = require_video_seconds(request.seconds)?;
    let source_video_id = request.source_video_id.trim();
    let extension_direction = request.extension_direction.trim();
    let reference_image = if request.reference_image_path.trim().is_empty() {
        None
    } else {
        Some(load_reference_image_payload(
            request.reference_image_path.trim(),
        )?)
    };
    validate_video_mode_inputs(
        api_mode,
        source_video_id,
        extension_direction,
        reference_image.is_some(),
    )?;

    progress
        .emit(crate::runtime::ProgressEvent::stage(
            "submitting",
            "正在提交视频生成请求",
        ))
        .map_err(|error| error.to_string())?;

    let reference_images = reference_image
        .iter()
        .map(|image| image.data_url.clone())
        .collect::<Vec<_>>();
    let standard_request = api_client::VideoApiRequest {
        api_base: &api_base,
        api_key: &api_key,
        prompt: &prompt,
        model: &model,
        size: &size,
        seconds,
        video_id: (!source_video_id.is_empty()).then_some(source_video_id),
        direction: (!extension_direction.is_empty()).then_some(extension_direction),
        reference_images: &reference_images,
        proxy_url: &proxy_url,
    };
    let bytes = match api_mode {
        config::VideoApiMode::ChatCompletions => {
            api_client::call_chat_completions_video_api(
                &api_base,
                &api_key,
                &prompt,
                &model,
                &size,
                &seconds.to_string(),
                &proxy_url,
            )
            .await
        }
        config::VideoApiMode::Videos => {
            api_client::call_video_api("videos", &standard_request).await
        }
        config::VideoApiMode::VideosGenerations => {
            api_client::call_video_api("videos/generations", &standard_request).await
        }
        config::VideoApiMode::VideosEdits => {
            api_client::call_video_api("videos/edits", &standard_request).await
        }
        config::VideoApiMode::VideosExtensions => {
            api_client::call_video_api("videos/extensions", &standard_request).await
        }
    }?;

    progress
        .emit(crate::runtime::ProgressEvent::stage(
            "saving",
            "正在保存生成视频",
        ))
        .map_err(|error| error.to_string())?;
    let file_path = save_generated_video_bytes(&save_dir, &prompt, &bytes)?;
    let file_name = required_file_name(
        Path::new(&file_path),
        "生成视频保存结果",
        "请重新生成视频后再加载。",
    )?;

    let duration_seconds = (start_time.elapsed().as_secs_f64() * 100.0).round() / 100.0;
    progress
        .emit(crate::runtime::ProgressEvent::stage(
            "completed",
            "视频生成完成",
        ))
        .map_err(|error| error.to_string())?;

    Ok(GeneratedVideoResult {
        file_path,
        file_name,
        duration_seconds,
    })
}

struct VideoChannelProgress<'a> {
    channel: &'a Channel<VideoGenerationEvent>,
}

impl crate::runtime::ProgressReporter for VideoChannelProgress<'_> {
    fn emit(&self, event: crate::runtime::ProgressEvent) -> crate::runtime::AppResult<()> {
        let frontend_event = match event.stage.as_str() {
            "submitting" => VideoGenerationEvent::Submitting,
            "saving" => VideoGenerationEvent::Saving,
            "completed" => VideoGenerationEvent::Completed,
            other => {
                return Err(crate::runtime::AppError::internal(format!(
                    "未知视频生成进度阶段：{other}"
                )))
            }
        };
        self.channel.send(frontend_event).map_err(|error| {
            crate::runtime::AppError::internal(format!("发送视频生成进度失败: {error}"))
        })
    }
}

pub(super) fn require_video_size(value: &str) -> Result<String, String> {
    let value = value.trim();
    match value {
        "1280x720" | "720x1280" | "1792x1024" | "1024x1792" => Ok(value.to_string()),
        _ => Err(format!("视频尺寸无效：{value}")),
    }
}

pub(super) fn require_video_seconds(value: u32) -> Result<u32, String> {
    if (1..=15).contains(&value) {
        Ok(value)
    } else {
        Err(format!("视频时长无效：{value}；文档允许 1 到 15 秒"))
    }
}

pub(super) fn validate_video_mode_inputs(
    api_mode: config::VideoApiMode,
    source_video_id: &str,
    extension_direction: &str,
    has_reference_image: bool,
) -> Result<(), String> {
    match api_mode {
        config::VideoApiMode::VideosEdits | config::VideoApiMode::VideosExtensions
            if source_video_id.is_empty() => Err(format!(
            "{} 需要原视频 ID。请填写创建视频任务时返回的 video_id。",
            if api_mode == config::VideoApiMode::VideosEdits {
                "/videos/edits"
            } else {
                "/videos/extensions"
            }
        )),
        config::VideoApiMode::ChatCompletions
            if !source_video_id.is_empty()
                || !extension_direction.is_empty()
                || has_reference_image =>
        {
            Err("Chat Completions 视频模式不支持标准视频端点的 ID、扩展方向或参考图参数。请清空这些参数或切换调用方式。".into())
        }
        _ => Ok(()),
    }
}

fn save_generated_video_bytes(
    save_dir: &str,
    prompt: &str,
    bytes: &[u8],
) -> Result<String, String> {
    std::fs::create_dir_all(save_dir).map_err(|e| format!("创建视频保存目录失败: {e}"))?;
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%f").to_string();
    let prompt_slug = sanitize_video_file_stem(prompt);
    let file_name = if prompt_slug.is_empty() {
        format!("sprite_animte_video_{timestamp}.mp4")
    } else {
        format!("sprite_animte_video_{timestamp}_{prompt_slug}.mp4")
    };
    let path = Path::new(save_dir).join(file_name);
    std::fs::write(&path, bytes).map_err(|e| format!("保存生成视频失败: {e}"))?;
    Ok(path.to_string_lossy().to_string())
}

fn sanitize_video_file_stem(value: &str) -> String {
    let stem: String = value
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch == '_' || ch.is_whitespace() {
                Some('_')
            } else {
                None
            }
        })
        .collect();
    let collapsed = stem
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_");
    collapsed.chars().take(48).collect()
}
