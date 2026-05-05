use serde::{Deserialize, Serialize};
use std::io::Cursor;
use tauri::ipc::Channel;
use tauri::{command, State};

use crate::api_client::{self, ApiCheckResult, GenerationResult, DEFAULT_API_BASE_URL};
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
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
const REFERENCE_PROMPT_OPTIMIZER_INSTRUCTIONS: &str = r#"
你是 SpriteAnimte 的参考图复刻提示词优化器。用户会在生图请求中同时上传参考图，参考图通常是一张已经排好格子的序列帧图。你的任务不是重新设计动画，也不是详细描述参考图，而是把用户想替换的角色/风格/限制整理成最终提示词，让生图模型以参考图为原型，最大可能复刻参考图的序列帧。

输出协议：
1. 只输出合法 JSON 对象，不要 Markdown、解释、代码块或额外字段。
2. 字段固定为 prompt、negative_prompt、grid_rows、grid_cols。
3. prompt 和 negative_prompt 为中文字符串；grid_rows、grid_cols 为数字。
4. 必须保留用户明确指定的目标角色、替换角色、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

参考图复刻模式：
- prompt 的核心必须是：以随请求上传的参考图为唯一动作、构图和分镜模板，用用户指定的新角色替换参考图中的角色，生成一张同结构 sprite sheet。
- 不要声称你已经看见参考图的具体内容；不要编造参考图里不存在或不确定的细节。
- 不要重新设计动作，不要扩写复杂剧情，不要写长篇“第X-Y帧”阶段描述；参考图已经提供动作和帧间关系。
- 必须要求最大程度复刻参考图的行列布局、总帧数、读取顺序、每格姿态、动作节奏、运动幅度、机位、镜头距离、角色占比、画面留白、脚底基线、锚点位置、朝向、比例和帧间连续性。
- 如果用户明确指定行列、帧数或网格，按用户指定返回；否则沿用当前界面网格作为参考，不要因为想象动作复杂度而擅自改大。
- prompt 必须写清：单张完整 sprite sheet，严格 N 行 M 列，共 N*M 帧；从左到右、从上到下读取；所有格子等宽等高；无间距、无边框、无编号、无文字、无水印、无可见网格线。
- prompt 必须写清：每帧角色完整位于自己的格子中，保留安全边距；头发、衣摆、武器、道具、特效和残影不能越过格子边界，不能与相邻帧重叠。
- prompt 必须写清：背景保持为纯色或高对比单色背景，默认纯白 #FFFFFF；如果用户指定背景色，使用用户指定背景色。
- prompt 必须写清：新角色身份、服装、比例和画风在所有帧保持一致；只继承参考图的动作姿态和构图关系。
- prompt 要短而强约束，重点围绕“参考图复刻 + 角色替换 + 可切分 sprite sheet”，不要加入无关美术描述。

negative_prompt 合并用户已有负面提示词，并补充：偏离参考图动作、偏离参考图构图、改变参考图帧顺序、重新设计动作、额外姿势、缺帧、重复帧、行列错乱、格子尺寸不一致、可见网格线、边框、编号、文字、水印、帧间重叠、角色跨格、身体裁切、安全边距不足、比例漂移、角色身份不一致、服装变化、朝向变化、脚底基线漂移、锚点漂移、动作断裂、手脚瞬移、模糊、低清晰度、复杂背景、渐变背景、纹理背景、场景道具、投影。
"#;
const REFERENCE_VISION_PROMPT_OPTIMIZER_INSTRUCTIONS: &str = r#"
你是 SpriteAnimte 的参考图视觉理解提示词优化器。用户会把参考图直接上传给你，参考图通常是一张已经排好格子的序列帧图。你的任务不是写图像赏析，而是理解参考图的动作、构图、网格、姿态和帧间节奏，把用户想替换的角色/风格/限制整理成最终提示词，让生图模型以参考图为原型，最大可能复刻参考图的序列帧。

输出协议：
1. 只输出合法 JSON 对象，不要 Markdown、解释、代码块或额外字段。
2. 字段固定为 prompt、negative_prompt、grid_rows、grid_cols。
3. prompt 和 negative_prompt 为中文字符串；grid_rows、grid_cols 为数字。
4. 必须保留用户明确指定的目标角色、替换角色、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

视觉参考图模式：
- 先理解参考图的行列布局、总帧数、读取顺序、每格姿态、动作节奏、运动幅度、机位、镜头距离、角色占比、画面留白、脚底基线、锚点位置、朝向、比例和帧间连续性。
- prompt 的核心必须是：以参考图为动作、构图和分镜模板，用用户指定的新角色替换参考图中的角色，生成一张同结构 sprite sheet。
- 不要输出长篇参考图描述，不要重新设计动作，不要扩写复杂剧情；只把参考图中对复刻序列帧有用的视觉约束压缩进最终 prompt。
- 如果能从参考图明确判断行列数，返回该行列数；如果用户明确指定行列、帧数或网格，优先遵守用户指定。
- prompt 必须写清：单张完整 sprite sheet，严格 N 行 M 列，共 N*M 帧；从左到右、从上到下读取；所有格子等宽等高；无间距、无边框、无编号、无文字、无水印、无可见网格线。
- prompt 必须写清：每帧角色完整位于自己的格子中，保留安全边距；头发、衣摆、武器、道具、特效和残影不能越过格子边界，不能与相邻帧重叠。
- prompt 必须写清：背景保持为纯色或高对比单色背景，默认纯白 #FFFFFF；如果用户指定背景色，使用用户指定背景色。
- prompt 必须写清：新角色身份、服装、比例和画风在所有帧保持一致；只继承参考图的动作姿态和构图关系。
- prompt 要短而强约束，重点围绕“视觉参考图复刻 + 角色替换 + 可切分 sprite sheet”，不要加入无关美术描述。

negative_prompt 合并用户已有负面提示词，并补充：偏离参考图动作、偏离参考图构图、改变参考图帧顺序、重新设计动作、额外姿势、缺帧、重复帧、行列错乱、格子尺寸不一致、可见网格线、边框、编号、文字、水印、帧间重叠、角色跨格、身体裁切、安全边距不足、比例漂移、角色身份不一致、服装变化、朝向变化、脚底基线漂移、锚点漂移、动作断裂、手脚瞬移、模糊、低清晰度、复杂背景、渐变背景、纹理背景、场景道具、投影。
"#;
const API_CHECK_TEXT_INSTRUCTIONS: &str =
    "你是 SpriteAnimte 的 API 连通性检测器，只需要返回合法 JSON。";
const API_CHECK_TEXT_INPUT: &str = "请只返回 {\"ok\":true}，不要输出解释。";
const REFERENCE_IMAGE_VARIANTS: &[(u32, u8)] = &[(1024, 86), (768, 82), (512, 76), (384, 70)];
const REFERENCE_IMAGE_MAX_PNG_BYTES: usize = 700_000;

#[derive(Debug, Clone)]
struct ReferenceImageVariant {
    data_url: String,
    label: String,
}

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

/// 轻量检测生图 API：只访问 /models，不触发生图。
#[command]
pub async fn check_generation_api(
    state: State<'_, AppState>,
    api_key: String,
    api_base: String,
    model: String,
    proxy_url: String,
) -> Result<ApiCheckResult, String> {
    let (api_key, api_base, model) = {
        let config = state.config.lock();
        let api_key = if api_key.trim().is_empty() {
            config.api_key.clone()
        } else {
            api_key
        };
        let api_base = if api_base.trim().is_empty() {
            if config.api_base.trim().is_empty() {
                DEFAULT_API_BASE_URL.into()
            } else {
                config.api_base.clone()
            }
        } else {
            api_base
        };
        let model = if model.trim().is_empty() {
            config.last_model.clone()
        } else {
            model
        };
        (api_key, api_base, model)
    };

    if api_key.trim().is_empty() {
        return Err("生图 API Key 为空".into());
    }

    match api_client::check_models_api_connection(&api_base, &api_key, &model, &proxy_url).await {
        Ok(result) => Ok(result),
        Err(error) if is_models_endpoint_unavailable(&error) => Ok(ApiCheckResult {
            ok: true,
            status: "warning".into(),
            message: format!(
                "服务有响应，但 /models 检测不可用：{error}。为避免消耗生图额度，本检测没有发送真实生图请求。"
            ),
            endpoint: format!("{}/models", api_base.trim_end_matches('/')),
            model,
            model_found: None,
        }),
        Err(error) => Err(error),
    }
}

/// 检测提示词优化 API：必须跑一次极小文本请求，/models 只作为模型名提示。
#[command]
pub async fn check_prompt_optimizer_api(
    state: State<'_, AppState>,
    api_key: String,
    api_base: String,
    model: String,
    proxy_url: String,
) -> Result<ApiCheckResult, String> {
    let (api_key, api_base, model) = {
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
        (api_key, api_base, model)
    };

    if api_key.trim().is_empty() {
        return Err("提示词优化 API Key 为空".into());
    }

    let models_check =
        api_client::check_models_api_connection(&api_base, &api_key, &model, &proxy_url).await;
    let text_check = api_client::call_responses_text_api(
        &api_base,
        &api_key,
        API_CHECK_TEXT_INSTRUCTIONS,
        API_CHECK_TEXT_INPUT,
        None,
        &model,
        &proxy_url,
    )
    .await;

    match (models_check, text_check) {
        (Ok(models_result), Ok(_)) => {
            let model_note = if models_result.status == "warning" {
                format!("；{}", models_result.message)
            } else if matches!(models_result.model_found, Some(true)) {
                format!("；模型 `{}` 已在 /models 中确认存在", models_result.model)
            } else {
                String::new()
            };
            Ok(ApiCheckResult {
                ok: true,
                status: models_result.status,
                message: format!("文本调用成功，提示词优化 API 可用{model_note}。"),
                endpoint: format!(
                    "{}/responses 或 /chat/completions",
                    api_base.trim_end_matches('/')
                ),
                model,
                model_found: models_result.model_found,
            })
        }
        (Err(models_error), Ok(_)) => {
            eprintln!("[api-check] 提示词优化 /models 检测失败，但文本探测成功: {models_error}");
            Ok(ApiCheckResult {
                ok: true,
                status: "warning".into(),
                message: format!("文本请求成功，但 /models 检测不可用：{models_error}"),
                endpoint: format!(
                    "{}/responses 或 /chat/completions",
                    api_base.trim_end_matches('/')
                ),
                model,
                model_found: None,
            })
        }
        (Ok(models_result), Err(text_error)) => Err(format!(
            "基础连接成功，但文本调用失败：{text_error}；/models 检测结果：{}",
            models_result.message
        )),
        (Err(models_error), Err(text_error)) => Err(format!(
            "/models 检测失败：{models_error}；文本探测失败：{text_error}"
        )),
    }
}

fn is_models_endpoint_unavailable(error: &str) -> bool {
    error.contains("HTTP 404") || error.contains("HTTP 405") || error.contains("HTTP 501")
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
    reference_image_path: String,
    use_reference_image_understanding: bool,
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
    let reference_image_path = reference_image_path.trim().to_string();
    let has_reference_image = !reference_image_path.is_empty();
    let uses_reference_image_understanding =
        has_reference_image && use_reference_image_understanding;
    if has_reference_image && !std::path::Path::new(&reference_image_path).is_file() {
        return Err("参考图文件不存在，无法按参考图优化提示词".into());
    }
    let reference_image_data_url = if uses_reference_image_understanding {
        let variants = load_reference_image_variants(&reference_image_path)?;
        variants.first().map(|variant| variant.data_url.clone())
    } else {
        None
    };
    let user_input = build_prompt_optimizer_input(
        &prompt,
        &neg_prompt,
        &style,
        &ratio,
        &resolution,
        safe_rows,
        safe_cols,
        has_reference_image,
        uses_reference_image_understanding,
    );

    eprintln!(
        "[prompt] 开始优化提示词 | 模型={model} API地址={api_base} 参考图模式={has_reference_image} 视觉理解={uses_reference_image_understanding}"
    );
    let instructions = if uses_reference_image_understanding {
        REFERENCE_VISION_PROMPT_OPTIMIZER_INSTRUCTIONS
    } else if has_reference_image {
        REFERENCE_PROMPT_OPTIMIZER_INSTRUCTIONS
    } else {
        PROMPT_OPTIMIZER_INSTRUCTIONS
    };

    let text_result = api_client::call_responses_text_api(
        &api_base,
        &api_key,
        instructions,
        &user_input,
        reference_image_data_url.as_deref(),
        &model,
        &proxy_url,
    )
    .await;

    let text = match text_result {
        Ok(text) => text,
        Err(err)
            if uses_reference_image_understanding
                && should_fallback_reference_vision_to_text_mode(&err) =>
        {
            eprintln!("[prompt] 视觉理解不可用，降级为不传图的参考图复刻模式: {err}");
            let fallback_input = build_prompt_optimizer_input(
                &prompt,
                &neg_prompt,
                &style,
                &ratio,
                &resolution,
                safe_rows,
                safe_cols,
                has_reference_image,
                false,
            );
            let fallback_text = api_client::call_responses_text_api(
                &api_base,
                &api_key,
                REFERENCE_PROMPT_OPTIMIZER_INSTRUCTIONS,
                &fallback_input,
                None,
                &model,
                &proxy_url,
            )
            .await
            .map_err(|fallback_err| {
                format!(
                    "视觉理解失败：{err}；已尝试降级为不传图的参考图复刻模式，但降级优化也失败：{fallback_err}"
                )
            })?;
            let mut parsed =
                parse_prompt_optimization_result(&fallback_text, &neg_prompt, safe_rows, safe_cols);
            parsed.warning = Some(
                "当前提示词优化模型无法处理参考图，已自动降级为不传图的参考图复刻模式。后续生图仍会上传参考图。"
                    .into(),
            );
            return Ok(parsed);
        }
        Err(err) => return Err(err),
    };

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

#[allow(clippy::too_many_arguments)]
fn build_prompt_optimizer_input(
    prompt: &str,
    neg_prompt: &str,
    style: &str,
    ratio: &str,
    resolution: &str,
    safe_rows: u32,
    safe_cols: u32,
    has_reference_image: bool,
    uses_reference_image_understanding: bool,
) -> String {
    let total_frames = safe_rows * safe_cols;
    if uses_reference_image_understanding {
        return format!(
            "用户原始提示词：\n{prompt}\n\n当前负面提示词：\n{}\n\n当前画图参数：风格={style}，比例={ratio}，分辨率={resolution}，当前界面网格={}行{}列，共{}帧。\n\n当前已选择参考图，且本次提示词优化请求会直接上传这张参考图。请进入视觉参考图复刻模式，改写成可直接用于生成 sprite sheet 的最终提示词。要求：\n1. 用户提示词中的角色、替换目标、服装、画风和限制是新角色设定；参考图是动作、构图、网格、姿态和帧间节奏模板。\n2. 先理解参考图的行列布局、总帧数、读取顺序、每格姿态、动作节奏、角色占比、留白、脚底基线和锚点位置。\n3. prompt 必须明确要求：以参考图为动作和构图模板，最大程度复刻参考图的序列帧布局、每格姿态、动作节奏、镜头距离、角色占比、留白、脚底基线和锚点位置。\n4. prompt 必须明确要求：用用户指定角色替换参考图中的角色；新角色身份、服装、比例、画风在所有帧保持一致。\n5. 不要重新设计动作，不要写长篇图像描述，不要扩写复杂剧情；只保留对复刻序列帧有用的视觉约束。\n6. 如果能从参考图明确判断行列数，返回该行列数；如果用户明确指定行列或帧数，遵守用户指定。\n7. prompt 仍需包含严格 sprite sheet 切分约束：单张完整图片、严格行列、共 N*M 帧、从左到右从上到下读取、等尺寸格子、无间距边框编号文字水印、纯色背景、角色不跨格不重叠、安全边距充足。\n8. negative_prompt 合并当前负面提示词，重点禁止偏离参考图、重新设计动作、改变帧顺序、行列错乱、跨格、重叠、身份漂移和背景干扰。\n9. 只返回 JSON。",
            neg_prompt.trim(),
            safe_rows,
            safe_cols,
            total_frames
        );
    }
    if has_reference_image {
        return format!(
            "用户原始提示词：\n{prompt}\n\n当前负面提示词：\n{}\n\n当前画图参数：风格={style}，比例={ratio}，分辨率={resolution}，当前界面网格={}行{}列，共{}帧。\n\n当前已选择参考图，后续生图请求会同时上传这张参考图。请进入参考图复刻模式，改写成可直接用于生成 sprite sheet 的最终提示词。要求：\n1. 用户提示词中的角色、替换目标、服装、画风和限制是新角色设定；参考图是动作、构图、网格、姿态和帧间节奏模板。\n2. prompt 必须明确要求：以随请求上传的参考图为唯一动作和构图模板，最大程度复刻参考图的序列帧布局、每格姿态、动作节奏、镜头距离、角色占比、留白、脚底基线和锚点位置。\n3. prompt 必须明确要求：用用户指定角色替换参考图中的角色；新角色身份、服装、比例、画风在所有帧保持一致。\n4. 不要重新设计动作，不要写长篇动作阶段，不要臆测参考图具体内容；参考图会由生图模型直接看到。\n5. 如果用户明确指定行列或帧数，遵守用户指定；否则沿用当前界面网格，不要擅自扩展。\n6. prompt 仍需包含严格 sprite sheet 切分约束：单张完整图片、严格行列、共 N*M 帧、从左到右从上到下读取、等尺寸格子、无间距边框编号文字水印、纯色背景、角色不跨格不重叠、安全边距充足。\n7. negative_prompt 合并当前负面提示词，重点禁止偏离参考图、重新设计动作、改变帧顺序、行列错乱、跨格、重叠、身份漂移和背景干扰。\n8. 只返回 JSON。",
            neg_prompt.trim(),
            safe_rows,
            safe_cols,
            total_frames
        );
    }

    format!(
        "用户原始提示词：\n{prompt}\n\n当前负面提示词：\n{}\n\n当前画图参数：风格={style}，比例={ratio}，分辨率={resolution}，当前界面网格={}行{}列，共{}帧。\n\n请改写成可直接用于生成 sprite sheet 的最终提示词。要求：\n1. 先识别用户真正想要的角色、动作、循环方式、视角、背景和特殊限制。\n2. 用户明确指定行列数、帧数或动画时长时，必须尊重用户指定值。\n3. 用户没有明确指定行列或帧数时，把当前界面网格作为参考起点；根据动作复杂度判断是否沿用或调整，并返回最终 grid_rows、grid_cols。不要固定套用某个帧数，也不要为了增加帧数而盲目变大。\n4. prompt 必须写清严格行列、总帧数、等尺寸格子、无边框编号文字、纯色背景、帧间不重叠不跨格、角色一致、安全边距、脚底基线和定位参考稳定。\n5. 动作设计要用少量“第X-Y帧”阶段覆盖全部帧，写清每段从什么姿态过渡到什么姿态，以及手臂、手掌、腿脚、重心、头部、道具、衣摆或头发如何逐步变化；相邻帧只能小幅增量变化，阶段之间必须连续丝滑。\n6. 不要逐帧长篇说明，不要套用固定示例，不要加入与切分和动作连续性无关的美术废话。\n7. negative_prompt 合并当前负面提示词，只补充会破坏切分、抠图和播放稳定性的核心问题。\n8. 只返回 JSON。",
        neg_prompt.trim(),
        safe_rows,
        safe_cols,
        total_frames
    )
}

fn should_fallback_reference_vision_to_text_mode(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    let image_related = lower.contains("image")
        || lower.contains("vision")
        || lower.contains("visual")
        || lower.contains("input_image")
        || lower.contains("image_url")
        || lower.contains("multimodal")
        || lower.contains("multi-modal")
        || lower.contains("图像")
        || lower.contains("图片")
        || lower.contains("视觉");
    let multimodal_shape_related =
        (lower.contains("input") || lower.contains("content") || lower.contains("message"))
            && (lower.contains("must be a string")
                || lower.contains("should be a string")
                || lower.contains("expected string")
                || lower.contains("must be string")
                || lower.contains("must be a list")
                || lower.contains("should be a list")
                || lower.contains("expected list")
                || lower.contains("invalid type"));

    (image_related || multimodal_shape_related)
        && (lower.contains("not support")
            || lower.contains("does not support")
            || lower.contains("unsupported")
            || lower.contains("not allowed")
            || lower.contains("invalid")
            || lower.contains("bad request")
            || lower.contains("http 400")
            || lower.contains("不支持")
            || lower.contains("无法")
            || lower.contains("不能"))
}

/// 读取图片并返回 PNG base64，供前端抠图画布编辑。
#[command]
pub async fn read_image_as_base64(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = image_processor::load_image(&path)?;
        image_processor::image_to_base64(&img)
    })
    .await
    .map_err(|e| format!("读取图片任务执行失败: {e}"))?
}

/// 直接读取文件字节并返回 base64，用于已经是 PNG 的临时帧，避免图片解码再编码。
#[command]
pub async fn read_file_as_base64(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|e| format!("读取文件失败: {}", e))?;
        Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            bytes,
        ))
    })
    .await
    .map_err(|e| format!("读取文件任务执行失败: {e}"))?
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
    reference_image_path: String,
) -> Result<GenerationResult, String> {
    let start_time = std::time::Instant::now();

    let reference_image_path = reference_image_path.trim().to_string();
    eprintln!(
        "[generate] 开始生成 | 模型={model} 风格={style} 宽高比={ratio} 分辨率={resolution} 数量={count} 参考图={}",
        !reference_image_path.is_empty()
    );
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

    let reference_image_variants = if reference_image_path.is_empty() {
        Vec::new()
    } else {
        load_reference_image_variants(&reference_image_path).inspect_err(|e| {
            eprintln!("[generate] 读取参考图失败: {e}");
            let _ = channel.send(GenerateEvent::Error { message: e.clone() });
        })?
    };

    // 推送进度
    let _ = channel.send(GenerateEvent::Started);
    let _ = channel.send(GenerateEvent::SendingRequest);
    let _ = channel.send(GenerateEvent::ReceivingResponse);

    eprintln!("[generate] 请求方式固定为 responses，调用 /responses stream/sized");
    let images_base64 = call_responses_with_reference_fallback(
        &api_base,
        &api_key,
        &full_prompt,
        &reference_image_variants,
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

#[allow(clippy::too_many_arguments)]
async fn call_responses_with_reference_fallback(
    api_base: &str,
    api_key: &str,
    full_prompt: &str,
    reference_image_variants: &[ReferenceImageVariant],
    model: &str,
    count: u32,
    size: &str,
    proxy_url: &str,
) -> Result<Vec<String>, String> {
    if reference_image_variants.is_empty() {
        return api_client::call_responses_api(
            api_base,
            api_key,
            full_prompt,
            None,
            model,
            count,
            size,
            proxy_url,
        )
        .await;
    }

    let mut last_error = String::new();
    for (index, variant) in reference_image_variants.iter().enumerate() {
        eprintln!(
            "[generate] 使用参考图变体 {}/{}: {}",
            index + 1,
            reference_image_variants.len(),
            variant.label
        );
        match api_client::call_responses_api(
            api_base,
            api_key,
            full_prompt,
            Some(variant.data_url.as_str()),
            model,
            count,
            size,
            proxy_url,
        )
        .await
        {
            Ok(images) => return Ok(images),
            Err(err)
                if should_try_smaller_reference_image(&err)
                    && index + 1 < reference_image_variants.len() =>
            {
                eprintln!("[generate] 参考图请求失败，降级到更小参考图后重试: {err}");
                last_error = err;
            }
            Err(err) => return Err(err),
        }
    }

    Err(last_error)
}

fn should_try_smaller_reference_image(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("连接中断")
        || lower.contains("connection closed before message completed")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("unexpected eof")
        || lower.contains("payload too large")
        || lower.contains("request entity too large")
        || lower.contains("body too large")
        || lower.contains("http 413")
}

fn load_reference_image_variants(path: &str) -> Result<Vec<ReferenceImageVariant>, String> {
    let img = image::open(path).map_err(|e| format!("加载参考图失败: {}", e))?;
    let original_width = img.width();
    let original_height = img.height();
    let has_transparency = image_has_transparency(&img);
    let mut variants = Vec::new();

    if has_transparency {
        let png_image = resize_reference_image_to(img.clone(), REFERENCE_IMAGE_VARIANTS[0].0);
        let png = encode_reference_png(&png_image)?;
        if png.len() <= REFERENCE_IMAGE_MAX_PNG_BYTES {
            variants.push(build_reference_variant(
                original_width,
                original_height,
                &png_image,
                "image/png",
                png,
                "png",
            ));
        } else {
            eprintln!(
                "[generate] 参考图 PNG 体积过大({:.1} KiB)，跳过 PNG 上传变体",
                png.len() as f64 / 1024.0
            );
        }
    }

    for (max_edge, quality) in REFERENCE_IMAGE_VARIANTS {
        let resized = resize_reference_image_to(img.clone(), *max_edge);
        let jpeg = encode_reference_jpeg(&resized, *quality)?;
        variants.push(build_reference_variant(
            original_width,
            original_height,
            &resized,
            "image/jpeg",
            jpeg,
            &format!("jpeg-q{quality}"),
        ));
    }

    if variants.is_empty() {
        return Err("参考图编码失败：没有可用上传变体".into());
    }
    Ok(variants)
}

fn build_reference_variant(
    original_width: u32,
    original_height: u32,
    img: &image::DynamicImage,
    mime: &'static str,
    bytes: Vec<u8>,
    mode: &str,
) -> ReferenceImageVariant {
    let base64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    let label = format!(
        "{mode} {}x{} -> {}x{} {} {:.1} KiB",
        original_width,
        original_height,
        img.width(),
        img.height(),
        mime,
        base64.len() as f64 / 1024.0
    );
    eprintln!("[generate] 参考图已编码: {label}");
    ReferenceImageVariant {
        data_url: format!("data:{mime};base64,{base64}"),
        label,
    }
}

fn resize_reference_image_to(img: image::DynamicImage, max_edge_limit: u32) -> image::DynamicImage {
    let width = img.width();
    let height = img.height();
    let max_edge = width.max(height);
    if max_edge <= max_edge_limit {
        return img;
    }

    let new_width = ((width as u64 * max_edge_limit as u64) / max_edge as u64).max(1) as u32;
    let new_height = ((height as u64 * max_edge_limit as u64) / max_edge as u64).max(1) as u32;
    img.resize(new_width, new_height, image::imageops::FilterType::Lanczos3)
}

fn image_has_transparency(img: &image::DynamicImage) -> bool {
    img.to_rgba8().pixels().any(|pixel| pixel.0[3] < 255)
}

fn encode_reference_png(img: &image::DynamicImage) -> Result<Vec<u8>, String> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| format!("编码参考图 PNG 失败: {}", e))?;
    Ok(buf.into_inner())
}

fn encode_reference_jpeg(img: &image::DynamicImage, quality: u8) -> Result<Vec<u8>, String> {
    let rgb = flatten_reference_to_rgb(img);
    let mut buf = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality)
        .encode_image(&image::DynamicImage::ImageRgb8(rgb))
        .map_err(|e| format!("编码参考图 JPEG 失败: {}", e))?;
    Ok(buf)
}

fn flatten_reference_to_rgb(img: &image::DynamicImage) -> image::RgbImage {
    let rgba = img.to_rgba8();
    let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
    for (x, y, pixel) in rgba.enumerate_pixels() {
        let alpha = f32::from(pixel.0[3]) / 255.0;
        let inv_alpha = 1.0 - alpha;
        let r = (f32::from(pixel.0[0]) * alpha + 255.0 * inv_alpha).round() as u8;
        let g = (f32::from(pixel.0[1]) * alpha + 255.0 * inv_alpha).round() as u8;
        let b = (f32::from(pixel.0[2]) * alpha + 255.0 * inv_alpha).round() as u8;
        rgb.put_pixel(x, y, image::Rgb([r, g, b]));
    }
    rgb
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
                    warning: None,
                };
            }
        }
    }

    PromptOptimizationResult {
        prompt: trimmed.to_string(),
        negative_prompt: fallback_negative_prompt.trim().to_string(),
        grid_rows: fallback_rows,
        grid_cols: fallback_cols,
        warning: None,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_image_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sprite_anime_{name}_{}_{}.png",
            std::process::id(),
            stamp
        ))
    }

    fn decode_data_url_image(data_url: &str) -> image::DynamicImage {
        let (_, b64) = data_url.split_once("base64,").unwrap();
        let bytes =
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD, b64).unwrap();
        image::load_from_memory(&bytes).unwrap()
    }

    #[test]
    fn prompt_optimizer_input_uses_reference_replication_mode() {
        let input = build_prompt_optimizer_input(
            "换成红衣女剑士",
            "低清晰度",
            "像素风",
            "1:1",
            "1024",
            2,
            3,
            true,
            false,
        );

        assert!(input.contains("当前已选择参考图"));
        assert!(input.contains("参考图是动作、构图、网格、姿态和帧间节奏模板"));
        assert!(input.contains("以随请求上传的参考图为唯一动作和构图模板"));
        assert!(input.contains("用用户指定角色替换参考图中的角色"));
        assert!(input.contains("不要重新设计动作"));
        assert!(input.contains("不要写长篇动作阶段"));
        assert!(input.contains("当前界面网格=2行3列，共6帧"));
        assert!(!input.contains("先识别用户真正想要的角色、动作、循环方式"));
    }

    #[test]
    fn prompt_optimizer_input_uses_reference_vision_mode() {
        let input = build_prompt_optimizer_input(
            "换成蓝衣骑士",
            "",
            "动画",
            "1:1",
            "1024",
            2,
            4,
            true,
            true,
        );

        assert!(input.contains("本次提示词优化请求会直接上传这张参考图"));
        assert!(input.contains("视觉参考图复刻模式"));
        assert!(input.contains("先理解参考图的行列布局"));
        assert!(input.contains("不要写长篇图像描述"));
        assert!(input.contains("当前界面网格=2行4列，共8帧"));
    }

    #[test]
    fn reference_vision_fallback_detects_image_input_errors() {
        assert!(should_fallback_reference_vision_to_text_mode(
            "HTTP 400: This model does not support image input"
        ));
        assert!(should_fallback_reference_vision_to_text_mode(
            "HTTP 400: invalid type for input content"
        ));
        assert!(!should_fallback_reference_vision_to_text_mode(
            "HTTP 401: invalid API key"
        ));
        assert!(!should_fallback_reference_vision_to_text_mode(
            "HTTP 429: rate limit exceeded"
        ));
    }

    #[test]
    fn prompt_optimizer_input_keeps_normal_design_mode_without_reference() {
        let input = build_prompt_optimizer_input(
            "小机器人跑步循环",
            "",
            "手绘",
            "1:1",
            "1024",
            4,
            4,
            false,
            false,
        );

        assert!(input.contains("先识别用户真正想要的角色、动作、循环方式"));
        assert!(input.contains("动作设计要用少量“第X-Y帧”阶段覆盖全部帧"));
        assert!(input.contains("当前界面网格=4行4列，共16帧"));
        assert!(!input.contains("当前已选择参考图"));
        assert!(!input.contains("参考图复刻模式"));
    }

    #[test]
    fn api_check_treats_missing_models_endpoint_as_unavailable() {
        assert!(is_models_endpoint_unavailable("HTTP 404: not found"));
        assert!(is_models_endpoint_unavailable(
            "HTTP 405: method not allowed"
        ));
        assert!(!is_models_endpoint_unavailable("HTTP 401: unauthorized"));
    }

    #[test]
    fn reference_image_data_url_compacts_large_opaque_image_to_jpeg() {
        let path = temp_image_path("opaque");
        let img = image::RgbImage::from_pixel(2048, 1024, image::Rgb([42, 80, 220]));
        image::DynamicImage::ImageRgb8(img).save(&path).unwrap();

        let variants = load_reference_image_variants(path.to_str().unwrap()).unwrap();
        let data_url = &variants[0].data_url;
        let decoded = decode_data_url_image(data_url);
        let _ = std::fs::remove_file(&path);

        assert!(data_url.starts_with("data:image/jpeg;base64,"));
        assert!(decoded.width().max(decoded.height()) <= REFERENCE_IMAGE_VARIANTS[0].0);
        assert!(variants.len() >= 3);
    }

    #[test]
    fn reference_image_data_url_keeps_small_transparent_image_as_png() {
        let path = temp_image_path("transparent");
        let mut img = image::RgbaImage::from_pixel(32, 32, image::Rgba([220, 42, 42, 0]));
        img.put_pixel(16, 16, image::Rgba([220, 42, 42, 255]));
        image::DynamicImage::ImageRgba8(img).save(&path).unwrap();

        let variants = load_reference_image_variants(path.to_str().unwrap()).unwrap();
        let data_url = &variants[0].data_url;
        let decoded = decode_data_url_image(data_url);
        let _ = std::fs::remove_file(&path);

        assert!(data_url.starts_with("data:image/png;base64,"));
        assert_eq!(decoded.width(), 32);
        assert_eq!(decoded.height(), 32);
        assert!(variants
            .iter()
            .any(|variant| variant.data_url.starts_with("data:image/jpeg;base64,")));
    }
}
