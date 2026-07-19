use tauri::ipc::Channel;
use tauri::{command, State};

use crate::api_client::{self, GenerationResult, ImageApiRequest};
use crate::asset_library::{self, AssetCategory};
use crate::config::{self, AppState};
use crate::events::GenerateEvent;
use crate::image_processor;

use super::config_commands::{require_api_setting, IMAGE_API_SETTINGS_RESOLUTION};
use super::reference::{
    build_reference_generation_error, compute_image_size, load_reference_image_payload,
};

const MAX_REFERENCE_IMAGES: usize = 2;

pub(crate) struct ImageGenerationRequest {
    pub api_key: String,
    pub api_base: String,
    pub proxy_url: String,
    pub prompt: String,
    pub neg_prompt: String,
    pub model: String,
    pub style: String,
    pub ratio: String,
    pub resolution: String,
    pub count: u32,
    pub api_mode: String,
    pub reference_image_paths: Vec<String>,
    pub output_dir: Option<std::path::PathBuf>,
}

/// 核心：生成图片（固定使用配置的图片生成协议）。
#[command]
#[allow(clippy::too_many_arguments)]
pub async fn generate_image(
    state: State<'_, AppState>,
    channel: Channel<GenerateEvent>,
    api_key: String,
    api_base: String,
    proxy_url: String,
    prompt: String,
    neg_prompt: String,
    model: String,
    style: String,
    ratio: String,
    resolution: String,
    count: u32,
    api_mode: String,
    reference_image_paths: Vec<String>,
) -> Result<GenerationResult, String> {
    let progress = ImageChannelProgress { channel: &channel };
    generate_image_inner(
        &state,
        &progress,
        ImageGenerationRequest {
            api_key,
            api_base,
            proxy_url,
            prompt,
            neg_prompt,
            model,
            style,
            ratio,
            resolution,
            count,
            api_mode,
            reference_image_paths,
            output_dir: None,
        },
    )
    .await
}

pub(crate) async fn generate_image_inner(
    state: &AppState,
    progress: &dyn crate::runtime::ProgressReporter,
    request: ImageGenerationRequest,
) -> Result<GenerationResult, String> {
    let start_time = std::time::Instant::now();
    let api_mode = config::parse_generation_api_mode(&request.api_mode, "图片生成调用方式")?;

    let reference_image_paths = normalize_reference_image_paths(request.reference_image_paths)?;

    // 构建完整提示词
    let style_suffix = config::get_style_suffix(&request.style)?;
    let full_prompt = if style_suffix.is_empty() {
        request.prompt.clone()
    } else {
        format!("{}，{}", request.prompt, style_suffix)
    };
    let full_prompt = if request.neg_prompt.is_empty() {
        full_prompt
    } else {
        format!("{}\n\n避免: {}", full_prompt, request.neg_prompt)
    };

    let api_key = require_api_setting(
        request.api_key,
        "生图 API Key",
        IMAGE_API_SETTINGS_RESOLUTION,
    )?;
    let api_base = require_api_setting(
        request.api_base,
        "生图 API 地址",
        IMAGE_API_SETTINGS_RESOLUTION,
    )?;
    let model = require_api_setting(request.model, "生图模型", IMAGE_API_SETTINGS_RESOLUTION)?;
    let proxy_url = request.proxy_url.trim().to_string();

    let ratio_tuple = config::get_ratio_tuple(&request.ratio)?;
    let ratio = format!("{}:{}", ratio_tuple.0, ratio_tuple.1);

    // 根据分辨率和宽高比计算生成尺寸
    let size = compute_image_size(&request.resolution, ratio_tuple)?;
    let save_dir = match request.output_dir {
        Some(path) => {
            std::fs::create_dir_all(&path)
                .map_err(|error| format!("创建图片输出目录失败: {error}"))?;
            path
        }
        None => {
            asset_library::category_dir(&state.default_save_dir, AssetCategory::GeneratedImages)?
        }
    }
    .to_string_lossy()
    .to_string();

    let reference_image_payloads = reference_image_paths
        .iter()
        .map(|path| load_reference_image_payload(path))
        .collect::<Result<Vec<_>, _>>()?;
    let reference_data_urls = reference_image_payloads
        .iter()
        .map(|reference| reference.data_url.as_str())
        .collect::<Vec<_>>();
    let standard_request = ImageApiRequest {
        api_base: &api_base,
        api_key: &api_key,
        prompt: &full_prompt,
        model: &model,
        count: request.count,
        size: &size,
        aspect_ratio: &ratio,
        resolution: &request.resolution,
        proxy_url: &proxy_url,
    };

    progress
        .emit(crate::runtime::ProgressEvent::stage(
            "sending_request",
            "正在发送图片生成请求",
        ))
        .map_err(|error| error.to_string())?;

    let images_base64 = match api_mode {
        config::GenerationApiMode::ChatCompletions => {
            api_client::call_chat_completions_image_api(&standard_request, &reference_data_urls)
                .await
                .map_err(|error| {
                    build_reference_generation_error(error, &reference_image_payloads)
                })?
        }
        config::GenerationApiMode::Responses => {
            api_client::call_responses_api(&standard_request, &reference_data_urls)
                .await
                .map_err(|error| {
                    build_reference_generation_error(error, &reference_image_payloads)
                })?
        }
        config::GenerationApiMode::ImagesGenerations => {
            if !reference_image_payloads.is_empty() {
                return Err("/images/generations 只支持纯文本生图。请移除参考图，或把图片生成调用方式改为 /images/edits JSON 或 multipart。".into());
            }
            api_client::call_images_generations_api(&standard_request).await?
        }
        config::GenerationApiMode::ImagesEditsJson => {
            api_client::call_images_edits_json_api(&standard_request, &reference_data_urls).await?
        }
        config::GenerationApiMode::ImagesEditsMultipart => {
            let uploads = reference_image_payloads
                .iter()
                .map(|reference| (reference.bytes.as_slice(), reference.mime))
                .collect::<Vec<_>>();
            api_client::call_images_edits_multipart_api(&standard_request, &uploads).await?
        }
    };
    progress
        .emit(crate::runtime::ProgressEvent::counted(
            "extracting_urls",
            "已解析图片响应",
            images_base64.len() as u64,
            images_base64.len() as u64,
        ))
        .map_err(|error| error.to_string())?;

    // 处理每张图片（base64 → 解码 → 缩放 → 保存）
    let mut saved_files: Vec<String> = Vec::new();

    for (i, b64) in images_base64.iter().enumerate() {
        emit_image_step(progress, i, images_base64.len(), "解码 base64")?;

        // base64 → 图片
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .map_err(|e| format!("base64解码失败: {}", e))?;

        let img = image_processor::bytes_to_image(&data)?;

        emit_image_step(progress, i, images_base64.len(), "缩放")?;

        let img = image_processor::resize_image(&img, &request.resolution)?;

        emit_image_step(progress, i, images_base64.len(), "保存")?;

        let path = image_processor::save_image(&img, &save_dir, "sprite_animte", (i + 1) as u32)?;

        saved_files.push(path);
    }

    let duration_seconds = (start_time.elapsed().as_secs_f64() * 100.0).round() / 100.0;

    progress
        .emit(crate::runtime::ProgressEvent::counted(
            "completed",
            "图片生成完成",
            saved_files.len() as u64,
            saved_files.len() as u64,
        ))
        .map_err(|error| error.to_string())?;

    Ok(GenerationResult {
        image_urls: saved_files,
        duration_seconds,
    })
}

struct ImageChannelProgress<'a> {
    channel: &'a Channel<GenerateEvent>,
}

impl crate::runtime::ProgressReporter for ImageChannelProgress<'_> {
    fn emit(&self, event: crate::runtime::ProgressEvent) -> crate::runtime::AppResult<()> {
        let frontend_event = match event.stage.as_str() {
            "sending_request" => GenerateEvent::SendingRequest,
            "extracting_urls" => GenerateEvent::ExtractingUrls {
                found: required_progress_count(&event, "图片响应数量")?,
            },
            "processing_image" => GenerateEvent::ProcessingImage {
                index: required_progress_count(&event, "图片处理序号")?,
                step: event.message,
            },
            "completed" => GenerateEvent::Completed {
                total_images: required_progress_count(&event, "图片完成数量")?,
            },
            other => {
                return Err(crate::runtime::AppError::internal(format!(
                    "未知图片生成进度阶段：{other}"
                )))
            }
        };
        self.channel.send(frontend_event).map_err(|error| {
            crate::runtime::AppError::internal(format!("发送图片生成进度失败: {error}"))
        })
    }
}

fn required_progress_count(
    event: &crate::runtime::ProgressEvent,
    label: &str,
) -> crate::runtime::AppResult<usize> {
    event
        .current
        .map(|value| value as usize)
        .ok_or_else(|| crate::runtime::AppError::internal(format!("{label}缺失")))
}

fn emit_image_step(
    progress: &dyn crate::runtime::ProgressReporter,
    index: usize,
    total: usize,
    step: &str,
) -> Result<(), String> {
    progress
        .emit(crate::runtime::ProgressEvent::counted(
            "processing_image",
            step,
            (index + 1) as u64,
            total as u64,
        ))
        .map_err(|error| error.to_string())
}

pub(super) fn normalize_reference_image_paths(paths: Vec<String>) -> Result<Vec<String>, String> {
    if paths.len() > MAX_REFERENCE_IMAGES {
        return Err(format!(
            "图片生成最多支持 {MAX_REFERENCE_IMAGES} 张参考图，当前为 {} 张",
            paths.len()
        ));
    }
    let mut normalized = Vec::with_capacity(paths.len());
    for (index, path) in paths.into_iter().enumerate() {
        let path = path.trim().to_string();
        if path.is_empty() {
            return Err(format!("第{}张参考图路径为空", index + 1));
        }
        if normalized.contains(&path) {
            return Err(format!("第{}张参考图与前面的路径重复", index + 1));
        }
        normalized.push(path);
    }
    Ok(normalized)
}
