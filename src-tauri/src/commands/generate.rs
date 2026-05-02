use serde::{Deserialize, Serialize};
use tauri::ipc::Channel;
use tauri::{command, State};

use crate::api_client::{self, GenerationResult, DEFAULT_API_BASE_URL};
use crate::config::{self, AppState, PresetsPayload, UserConfig};
use crate::events::GenerateEvent;
use crate::image_processor;
use crate::logger::{GenerationLog, JsonLinesLogger};
use crate::workbench::{WorkbenchRecord, WorkbenchStore};

#[derive(Debug, Clone, Serialize)]
pub struct TransparentBackgroundCommandResult {
    pub file_path: String,
    pub file_name: String,
    pub base64_data: String,
    pub background_color: String,
    pub transparent_pixels: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransparentBackgroundCanvasResult {
    pub base64_data: String,
    pub background_color: String,
    pub transparent_pixels: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptOptimizationResult {
    pub prompt: String,
    pub negative_prompt: String,
    pub grid_rows: u32,
    pub grid_cols: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct RawPromptOptimizationResult {
    prompt: Option<String>,
    negative_prompt: Option<String>,
    grid_rows: Option<u32>,
    grid_cols: Option<u32>,
}

const PROMPT_OPTIMIZER_INSTRUCTIONS: &str = r#"
你是 SpriteAnimte 的序列帧提示词优化器。你的目标不是套模板，而是把用户想法改写成更容易生成、切分和播放的单张 sprite sheet 提示词，重点解决每帧细节不稳定、相邻帧跳变、动作不连贯、帧间重叠和定位漂移。

输出协议：
1. 只输出合法 JSON 对象，不要 Markdown、解释、代码块或额外字段。
2. 字段固定为 prompt、negative_prompt、grid_rows、grid_cols。
3. prompt 和 negative_prompt 为中文字符串；grid_rows、grid_cols 为数字。
4. 必须保留用户明确指定的角色、动作、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

帧数与网格决策：
- 用户明确指定行列、总帧数或动画时长时，这些是硬约束，不能私自改变。
- 用户没有明确指定时，当前界面网格只是参考起点，不是硬性限制。先判断动作复杂度，再决定是否沿用或调整 grid_rows、grid_cols。
- 动作简单时使用较少帧，动作复杂、转身幅度大、肢体摆动多或需要循环顺滑时增加中间帧；不要因为习惯、示例或模板固定使用某个帧数，也不要为了变多而盲目增加帧数。
- 返回的 grid_rows * grid_cols 必须等于 prompt 中写明的总帧数，并且网格应便于切分和从左到右、从上到下阅读。

prompt 必须包含的核心约束：
- 单张完整 sprite sheet，严格 N 行 M 列，共 N*M 帧，从左到右、从上到下读取。
- 所有格子等宽等高；无间距、无边框、无编号、无文字、无水印、无可见网格线。
- 每帧角色完整位于自己的格子中，保留安全边距；头发、衣摆、武器、道具、特效和残影不能越过格子边界，不能与相邻帧重叠。
- 背景为统一纯色，默认纯白 #FFFFFF；如果用户指定背景色，使用指定的单一高对比背景色。不要场景、纹理、渐变、投影或地面线。
- 每帧保持同一角色身份、服装、比例、视角、朝向、镜头距离、光照和画风；只改变动作姿态。
- 脚底基线、角色根部或主要支撑点稳定，播放时整体位置和缩放不要跳动。
- 把动作拆成连续阶段，用“第X-Y帧”覆盖全部帧。每个阶段说明起始姿态、结束姿态，以及手臂、手掌、腿脚、重心、头部、道具、衣摆或头发如何逐步变化。
- 相邻帧只能小幅增量变化；上一阶段末尾必须自然接到下一阶段开头，不能出现支撑脚突换、手脚瞬移、身体朝向突变或重心跳跃。
- 如果用户要求循环动画，最后阶段要自然回到第一阶段，第一帧和最后一帧能无缝衔接。
- prompt 要精炼但具体，避免长示例、固定范式、无关美术扩写和逐帧堆砌。

negative_prompt 合并用户已有负面提示词，并只补充核心问题：复杂背景、渐变背景、纹理背景、场景道具、投影、可见网格线、边框、编号、文字、水印、格子尺寸不一致、行列错乱、帧间重叠、角色跨格、身体贴边、身体裁切、安全边距不足、比例变化、服装变化、朝向变化、脚底基线不一致、定位点漂移、重复帧、缺帧、帧顺序混乱、相邻帧无关、动作断裂、阶段突变、手脚瞬移、支撑脚突换、重心跳跃、循环首尾不连贯、模糊、低清晰度。
"#;

/// 获取所有预设选项
#[command]
pub fn get_presets() -> PresetsPayload {
    config::get_presets()
}

/// 加载用户配置
#[command]
pub fn load_config(state: State<'_, AppState>) -> UserConfig {
    let config = state.config.lock();
    config.clone()
}

/// 保存用户配置
#[command]
pub fn save_config(state: State<'_, AppState>, config: UserConfig) -> Result<(), String> {
    let mut current = state.config.lock();
    *current = config.clone();
    current.save(&state.config_path)
}

/// 获取提示词历史
#[command]
pub fn get_prompt_history(state: State<'_, AppState>, limit: usize) -> Vec<String> {
    let history = state.prompt_history.lock();
    history.iter().take(limit).cloned().collect()
}

/// 添加提示词到历史
#[command]
pub fn add_prompt_history(state: State<'_, AppState>, prompt: String) {
    let mut history = state.prompt_history.lock();
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return;
    }
    history.retain(|p| p != &prompt);
    history.push_front(prompt);
    history.truncate(100);
    let mut config = state.config.lock();
    config.prompt_history = history.clone();
    let _ = config.save(&state.config_path);
}

/// 读取工作台图片记录
#[command]
pub fn read_workbench_records(state: State<'_, AppState>, limit: usize) -> Vec<WorkbenchRecord> {
    let store = WorkbenchStore::new(state.workbench_records_path.clone());
    store.read_recent(limit)
}

/// 新增或更新工作台图片记录
#[command]
pub fn upsert_workbench_records(
    state: State<'_, AppState>,
    records: Vec<WorkbenchRecord>,
) -> Result<Vec<WorkbenchRecord>, String> {
    let store = WorkbenchStore::new(state.workbench_records_path.clone());
    store.upsert_many(records)
}

/// 从工作台移除一条记录，不删除实际图片文件
#[command]
pub fn delete_workbench_record(
    state: State<'_, AppState>,
    id: String,
) -> Result<Vec<WorkbenchRecord>, String> {
    let store = WorkbenchStore::new(state.workbench_records_path.clone());
    store.delete(&id)
}

/// 清空工作台记录，不删除实际图片文件
#[command]
pub fn clear_workbench_records(state: State<'_, AppState>) -> Result<(), String> {
    let store = WorkbenchStore::new(state.workbench_records_path.clone());
    store.clear()
}

/// 使用可配置 LLM 优化提示词。
#[command]
#[allow(clippy::too_many_arguments)]
pub async fn optimize_prompt(
    state: State<'_, AppState>,
    api_key: String,
    api_base: String,
    prompt: String,
    neg_prompt: String,
    model: String,
    style: String,
    ratio: String,
    resolution: String,
    grid_rows: u32,
    grid_cols: u32,
) -> Result<PromptOptimizationResult, String> {
    let prompt = prompt.trim().to_string();
    if prompt.is_empty() {
        return Err("请输入需要优化的提示词".into());
    }

    let (api_key, api_base, model, proxy_url) = {
        let config = state.config.lock();
        let api_key = if api_key.trim().is_empty() {
            if config.prompt_optimizer_api_key.trim().is_empty() {
                config.api_key.clone()
            } else {
                config.prompt_optimizer_api_key.clone()
            }
        } else {
            api_key
        };
        let api_base = if api_base.trim().is_empty() {
            if config.prompt_optimizer_api_base.trim().is_empty() {
                "https://api.deepseek.com".into()
            } else {
                config.prompt_optimizer_api_base.clone()
            }
        } else {
            api_base
        };
        let model = if model.trim().is_empty() {
            if config.prompt_optimizer_model.trim().is_empty() {
                config.last_model.clone()
            } else {
                config.prompt_optimizer_model.clone()
            }
        } else {
            model
        };
        (api_key, api_base, model, config.proxy_url.clone())
    };

    if api_key.trim().is_empty() {
        return Err("提示词优化 API Key 为空".into());
    }

    let safe_rows = grid_rows.clamp(1, 20);
    let safe_cols = grid_cols.clamp(1, 20);
    let total_frames = safe_rows * safe_cols;
    let user_input = format!(
        "用户原始提示词：\n{prompt}\n\n当前负面提示词：\n{}\n\n当前画图参数：风格={style}，比例={ratio}，分辨率={resolution}，当前界面网格={}行{}列，共{}帧。\n\n请改写成可直接用于生成 sprite sheet 的最终提示词。要求：\n1. 先识别用户真正想要的角色、动作、循环方式、视角、背景和特殊限制。\n2. 用户明确指定行列数、帧数或动画时长时，必须尊重用户指定值。\n3. 用户没有明确指定行列或帧数时，把当前界面网格作为参考起点；根据动作复杂度判断是否沿用或调整，并返回最终 grid_rows、grid_cols。不要固定套用某个帧数，也不要为了增加帧数而盲目变大。\n4. prompt 必须写清严格行列、总帧数、等尺寸格子、无边框编号文字、纯色背景、帧间不重叠不跨格、角色一致、安全边距、脚底基线和定位参考稳定。\n5. 动作设计要用少量“第X-Y帧”阶段覆盖全部帧，写清每段从什么姿态过渡到什么姿态，以及手臂、手掌、腿脚、重心、头部、道具、衣摆或头发如何逐步变化；相邻帧只能小幅增量变化，阶段之间必须连续丝滑。\n6. 不要逐帧长篇说明，不要套用固定示例，不要加入与切分和动作连续性无关的美术废话。\n7. negative_prompt 合并当前负面提示词，只补充会破坏切分、抠图和播放稳定性的核心问题。\n8. 只返回 JSON。",
        neg_prompt.trim(),
        safe_rows,
        safe_cols,
        total_frames
    );

    eprintln!("[prompt] 开始优化提示词 | 模型={model} API地址={api_base}");
    let text = api_client::call_responses_text_api(
        &api_base,
        &api_key,
        PROMPT_OPTIMIZER_INSTRUCTIONS,
        &user_input,
        &model,
        &proxy_url,
    )
    .await?;

    Ok(parse_prompt_optimization_result(
        &text,
        &neg_prompt,
        safe_rows,
        safe_cols,
    ))
}

/// 将前端当前抠图画布背景转为透明，不写入文件。
#[command]
pub fn apply_canvas_background_transparent(
    data_url: String,
    tolerance: Option<u8>,
    feather_radius: Option<u8>,
    color_key_mode: Option<String>,
) -> Result<TransparentBackgroundCanvasResult, String> {
    let image_data = data_url
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(data_url.as_str());
    let img = image_processor::base64_to_image(image_data)?;
    let result = image_processor::make_background_transparent(
        &img,
        image_processor::TransparentBackgroundOptions {
            tolerance: tolerance.unwrap_or(36).clamp(1, 255),
            feather_radius: feather_radius.unwrap_or(1).min(3),
            color_key_mode: parse_color_key_mode(color_key_mode.as_deref()),
        },
    );
    let [r, g, b] = result.background_rgb;
    Ok(TransparentBackgroundCanvasResult {
        base64_data: image_processor::image_to_base64(&result.image)?,
        background_color: format!("#{r:02X}{g:02X}{b:02X}"),
        transparent_pixels: result.transparent_pixels,
    })
}

/// 将前端抠图画布保存为新的 PNG 文件。
#[command]
pub fn save_matted_image_data_url(
    source_path: String,
    data_url: String,
) -> Result<TransparentBackgroundCommandResult, String> {
    let image_data = data_url
        .split_once(',')
        .map(|(_, data)| data)
        .unwrap_or(data_url.as_str());
    let img = image_processor::base64_to_image(image_data)?;
    let output_path = image_processor::save_transparent_copy(&img, &source_path)?;
    let transparent_pixels = img
        .to_rgba8()
        .pixels()
        .filter(|pixel| pixel.0[3] == 0)
        .count() as u32;
    Ok(transparent_command_result(
        output_path,
        [0, 0, 0],
        transparent_pixels,
    ))
}

/// 读取图片并返回 PNG base64，供前端抠图画布编辑。
#[command]
pub fn read_image_as_base64(path: String) -> Result<String, String> {
    let img = image_processor::load_image(&path)?;
    image_processor::image_to_base64(&img)
}

fn parse_color_key_mode(value: Option<&str>) -> image_processor::ColorKeyMode {
    match value.unwrap_or("auto") {
        "edge" => image_processor::ColorKeyMode::EdgeOnly,
        "global" => image_processor::ColorKeyMode::Global,
        _ => image_processor::ColorKeyMode::Auto,
    }
}

fn transparent_command_result(
    output_path: String,
    background_rgb: [u8; 3],
    transparent_pixels: u32,
) -> TransparentBackgroundCommandResult {
    let file_name = std::path::Path::new(&output_path)
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_default();
    let [r, g, b] = background_rgb;

    TransparentBackgroundCommandResult {
        file_path: output_path,
        file_name,
        base64_data: String::new(),
        background_color: format!("#{r:02X}{g:02X}{b:02X}"),
        transparent_pixels,
    }
}

/// 核心：生成图片（固定使用 /responses + image_generation 工具）
#[command]
#[allow(clippy::too_many_arguments)]
pub async fn generate_image(
    state: State<'_, AppState>,
    channel: Channel<GenerateEvent>,
    api_key: String,
    api_base: String,
    prompt: String,
    neg_prompt: String,
    model: String,
    style: String,
    ratio: String,
    resolution: String,
    count: u32,
) -> Result<GenerationResult, String> {
    let start_time = std::time::Instant::now();

    eprintln!("[generate] 开始生成 | 模型={model} 风格={style} 宽高比={ratio} 分辨率={resolution} 数量={count}");
    eprintln!("[generate] API地址={api_base} 提示词={prompt}");

    // 构建完整提示词
    let style_suffix = {
        let config = state.config.lock();
        config.get_style_suffix(&style)
    };
    let full_prompt = if style_suffix.is_empty() {
        prompt.clone()
    } else {
        format!("{}，{}", prompt, style_suffix)
    };
    let full_prompt = if neg_prompt.is_empty() {
        full_prompt
    } else {
        format!("{}\n\n避免: {}", full_prompt, neg_prompt)
    };

    // 若API地址为空则使用默认值
    let api_base = if api_base.is_empty() {
        DEFAULT_API_BASE_URL.to_string()
    } else {
        api_base
    };

    let ratio_tuple = config::get_ratio_tuple(&ratio);

    // 根据分辨率和宽高比计算生成尺寸
    let size = compute_image_size(&resolution, ratio_tuple);
    eprintln!(
        "[generate] 生成尺寸: {size} (分辨率={resolution} 比例={}:{})",
        ratio_tuple.0, ratio_tuple.1
    );

    let save_dir = {
        let config = state.config.lock();
        if config.save_dir.is_empty() {
            state.default_save_dir.to_string_lossy().to_string()
        } else {
            config.save_dir.clone()
        }
    };

    let proxy_url = {
        let config = state.config.lock();
        config.proxy_url.clone()
    };

    // 推送进度
    let _ = channel.send(GenerateEvent::Started);
    let _ = channel.send(GenerateEvent::SendingRequest);
    let _ = channel.send(GenerateEvent::ReceivingResponse);

    eprintln!("[generate] 请求方式固定为 responses，调用 /responses stream/sized");
    let images_base64 = api_client::call_responses_api(
        &api_base,
        &api_key,
        &full_prompt,
        &model,
        count,
        &size,
        &proxy_url,
    )
    .await?;
    let _ = channel.send(GenerateEvent::ExtractingUrls {
        found: images_base64.len(),
    });

    // 处理每张图片（base64 → 解码 → 缩放 → 保存）
    let mut saved_files: Vec<String> = Vec::new();

    for (i, b64) in images_base64.iter().enumerate() {
        let _ = channel.send(GenerateEvent::ProcessingImage {
            index: i + 1,
            step: "解码base64".into(),
        });

        // base64 → 图片
        let data = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64)
            .inspect_err(|e| {
                eprintln!("[generate] base64解码失败 第{}张: {e}", i + 1);
                let _ = channel.send(GenerateEvent::Error {
                    message: format!("base64解码失败: {}", e),
                });
            })
            .map_err(|e| format!("base64解码失败: {}", e))?;

        let img = image_processor::bytes_to_image(&data).inspect_err(|e| {
            eprintln!("[generate] 字节转图片失败 第{}张: {e}", i + 1);
            let _ = channel.send(GenerateEvent::Error { message: e.clone() });
        })?;

        eprintln!(
            "[generate] Responses模式保留原始画布 第{}张: {}x{}",
            i + 1,
            img.width(),
            img.height()
        );

        let _ = channel.send(GenerateEvent::ProcessingImage {
            index: i + 1,
            step: "缩放".into(),
        });

        let img = image_processor::resize_image(&img, &resolution);

        let _ = channel.send(GenerateEvent::ProcessingImage {
            index: i + 1,
            step: "保存".into(),
        });

        let path = image_processor::save_image(&img, &save_dir, "sprite_animte", (i + 1) as u32)
            .inspect_err(|e| {
                eprintln!("[generate] 保存图片失败 第{}张: {e}", i + 1);
                let _ = channel.send(GenerateEvent::Error { message: e.clone() });
            })?;

        saved_files.push(path);
    }

    // 记录日志
    let duration = start_time.elapsed().as_secs_f64();

    let duration_seconds = (duration * 100.0).round() / 100.0;
    let log_entry = GenerationLog {
        timestamp: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        model: model.clone(),
        style: style.clone(),
        ratio: ratio.clone(),
        resolution: resolution.clone(),
        count,
        prompt: prompt.clone(),
        negative_prompt: neg_prompt.clone(),
        full_prompt: full_prompt.clone(),
        success: true,
        image_paths: saved_files.clone(),
        duration_seconds,
        save_dir: save_dir.clone(),
    };

    if let Ok(logger) = JsonLinesLogger::new(&state.log_dir) {
        let _ = logger.append(&log_entry);
    }

    // 同步到图片生成工作台记录。失败不阻塞图片生成结果返回。
    let workbench_records = saved_files
        .iter()
        .map(|path| WorkbenchRecord {
            id: String::new(),
            path: path.clone(),
            label: String::new(),
            prompt: prompt.clone(),
            model: model.clone(),
            duration_seconds: Some(duration_seconds),
            created_at: log_entry.timestamp.clone(),
            updated_at: log_entry.timestamp.clone(),
        })
        .collect();
    let store = WorkbenchStore::new(state.workbench_records_path.clone());
    if let Err(err) = store.upsert_many(workbench_records) {
        eprintln!("[generate] 写入工作台记录失败: {err}");
    }

    // 添加到提示词历史
    {
        let mut history = state.prompt_history.lock();
        history.retain(|p| p != &prompt);
        history.push_front(prompt.clone());
        history.truncate(100);
        let mut config = state.config.lock();
        config.prompt_history = history.clone();
        let _ = config.save(&state.config_path);
    }

    let _ = channel.send(GenerateEvent::Completed {
        total_images: saved_files.len(),
    });

    Ok(GenerationResult {
        images_base64: Vec::new(),
        image_urls: saved_files,
        duration_seconds: Some(duration_seconds),
    })
}

/// 根据分辨率和宽高比计算生成尺寸（格式: "WxH"）
fn compute_image_size(resolution: &str, ratio: (u32, u32)) -> String {
    let base: u32 = match resolution {
        "2K" => 2048,
        _ => 1024, // "原始" 和 "1K" 都默认1024
    };
    let (rw, rh) = ratio;
    if rw >= rh {
        let h = (base as f64 * rh as f64 / rw as f64).round() as u32;
        format!("{}x{}", base, h)
    } else {
        let w = (base as f64 * rw as f64 / rh as f64).round() as u32;
        format!("{}x{}", w, base)
    }
}

fn parse_prompt_optimization_result(
    text: &str,
    fallback_negative_prompt: &str,
    fallback_rows: u32,
    fallback_cols: u32,
) -> PromptOptimizationResult {
    let trimmed = text.trim();
    let json_text = extract_json_object(trimmed).unwrap_or(trimmed);
    if let Ok(raw) = serde_json::from_str::<RawPromptOptimizationResult>(json_text) {
        if let Some(prompt) = raw.prompt.map(|value| value.trim().to_string()) {
            if !prompt.is_empty() {
                return PromptOptimizationResult {
                    prompt,
                    negative_prompt: raw
                        .negative_prompt
                        .map(|value| value.trim().to_string())
                        .filter(|value| !value.is_empty())
                        .unwrap_or_else(|| fallback_negative_prompt.trim().to_string()),
                    grid_rows: raw.grid_rows.unwrap_or(fallback_rows).clamp(1, 20),
                    grid_cols: raw.grid_cols.unwrap_or(fallback_cols).clamp(1, 20),
                };
            }
        }
    }

    PromptOptimizationResult {
        prompt: trimmed.to_string(),
        negative_prompt: fallback_negative_prompt.trim().to_string(),
        grid_rows: fallback_rows,
        grid_cols: fallback_cols,
    }
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    if end <= start {
        return None;
    }
    Some(&text[start..=end])
}
