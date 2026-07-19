use tauri::command;

use crate::api_client;
use crate::logger::summarize_log_text;

use super::config_commands::require_prompt_optimizer_api_settings;
use super::reference::load_reference_image_payload;
use super::types::{PromptOptimizationResult, RawPromptOptimizationResult};

const JSON_OUTPUT_PROTOCOL: &str = r#"输出协议：
1. 只输出合法 JSON 对象，不要 Markdown、解释、代码块或额外字段。
2. 字段固定为 prompt、negative_prompt、grid_rows、grid_cols。
3. prompt 和 negative_prompt 为中文字符串；grid_rows、grid_cols 为数字。
"#;

const SPRITE_SHEET_CORE_RULES: &str = r#"- 单张完整 sprite sheet，严格 N 行 M 列，共 N*M 帧；从左到右、从上到下读取；所有格子等宽等高；无间距、无边框、无编号、无文字、无水印、无可见网格线。
- 每帧角色完整位于自己的格子中，保留安全边距；头发、衣摆、武器、道具、特效和残影不能越过格子边界，不能与相邻帧重叠。
- 背景保持为纯色或高对比单色背景，默认纯白 #FFFFFF；如果用户指定背景色，使用用户指定背景色。
- 新角色身份、服装、比例和画风在所有帧保持一致；只继承参考图的动作姿态和构图关系。
"#;

const COMMON_NEGATIVE_PROMPT_RULES: &str = "缺帧、重复帧、行列错乱、格子尺寸不一致、可见网格线、边框、编号、文字、水印、帧间重叠、角色跨格、身体裁切、安全边距不足、服装变化、动作断裂、手脚瞬移、模糊、低清晰度、复杂背景、渐变背景、纹理背景、场景道具、投影";
const REFERENCE_NEGATIVE_PROMPT_RULES: &str = "偏离参考图动作、偏离参考图构图、改变参考图帧顺序、重新设计动作、额外姿势、比例漂移、角色身份不一致、脚底基线漂移、锚点漂移";

fn prompt_optimizer_instructions() -> String {
    format!(
        r#"你是 SpriteAnimte 的序列帧提示词优化器。你的目标不是套模板，而是把用户想法改写成更容易生成、切分和播放的单张 sprite sheet 提示词，重点解决每帧细节不稳定、相邻帧跳变、动作不连贯、帧间重叠和定位漂移。

{JSON_OUTPUT_PROTOCOL}4. 必须保留用户明确指定的角色、动作、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

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
"#
    )
}

fn reference_prompt_optimizer_instructions() -> String {
    format!(
        r#"你是 SpriteAnimte 的参考图复刻提示词优化器。用户会在生图请求中同时上传参考图，参考图通常是一张已经排好格子的序列帧图。你的任务不是重新设计动画，也不是详细描述参考图，而是把用户想替换的角色/风格/限制整理成最终提示词，让生图模型以参考图为原型，最大可能复刻参考图的序列帧。

{JSON_OUTPUT_PROTOCOL}4. 必须保留用户明确指定的目标角色、替换角色、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

参考图复刻模式：
- prompt 的核心必须是：以随请求上传的参考图为唯一动作、构图和分镜模板，用用户指定的新角色替换参考图中的角色，生成一张同结构 sprite sheet。
- 不要声称你已经看见参考图的具体内容；不要编造参考图里不存在或不确定的细节。
- 不要重新设计动作，不要扩写复杂剧情，不要写长篇“第X-Y帧”阶段描述；参考图已经提供动作和帧间关系。
- 必须要求最大程度复刻参考图的行列布局、总帧数、读取顺序、每格姿态、动作节奏、运动幅度、机位、镜头距离、角色占比、画面留白、脚底基线、锚点位置、朝向、比例和帧间连续性。
- 如果用户明确指定行列、帧数或网格，按用户指定返回；否则沿用当前界面网格作为参考，不要因为想象动作复杂度而擅自改大。
{SPRITE_SHEET_CORE_RULES}- prompt 要短而强约束，重点围绕“参考图复刻 + 角色替换 + 可切分 sprite sheet”，不要加入无关美术描述。

negative_prompt 合并用户已有负面提示词，并补充：{REFERENCE_NEGATIVE_PROMPT_RULES}、{COMMON_NEGATIVE_PROMPT_RULES}。
"#
    )
}

fn reference_vision_prompt_optimizer_instructions() -> String {
    format!(
        r#"你是 SpriteAnimte 的参考图视觉理解提示词优化器。用户会把参考图直接上传给你，参考图通常是一张已经排好格子的序列帧图。你的任务不是写图像赏析，而是理解参考图的动作、构图、网格、姿态和帧间节奏，把用户想替换的角色/风格/限制整理成最终提示词，让生图模型以参考图为原型，最大可能复刻参考图的序列帧。

{JSON_OUTPUT_PROTOCOL}4. 必须保留用户明确指定的目标角色、替换角色、服装、画风、背景色、行列数、帧数、视角、朝向和特殊限制。

视觉参考图模式：
- 先理解参考图的行列布局、总帧数、读取顺序、每格姿态、动作节奏、运动幅度、机位、镜头距离、角色占比、画面留白、脚底基线、锚点位置、朝向、比例和帧间连续性。
- prompt 的核心必须是：以参考图为动作、构图和分镜模板，用用户指定的新角色替换参考图中的角色，生成一张同结构 sprite sheet。
- 不要输出长篇参考图描述，不要重新设计动作，不要扩写复杂剧情；只把参考图中对复刻序列帧有用的视觉约束压缩进最终 prompt。
- 如果能从参考图明确判断行列数，返回该行列数；如果用户明确指定行列、帧数或网格，优先遵守用户指定。
{SPRITE_SHEET_CORE_RULES}- prompt 要短而强约束，重点围绕“视觉参考图复刻 + 角色替换 + 可切分 sprite sheet”，不要加入无关美术描述。

negative_prompt 合并用户已有负面提示词，并补充：{REFERENCE_NEGATIVE_PROMPT_RULES}、{COMMON_NEGATIVE_PROMPT_RULES}。
"#
    )
}
pub(super) const API_CHECK_TEXT_INSTRUCTIONS: &str =
    "你是 SpriteAnimte 的 API 连通性检测器，只需要返回合法 JSON。";
pub(super) const API_CHECK_TEXT_INPUT: &str = "请只返回 {\"ok\":true}，不要输出解释。";
pub(super) const REFERENCE_IMAGE_MAX_EDGE: u32 = 1024;
pub(super) const REFERENCE_IMAGE_JPEG_QUALITY: u8 = 86;
pub(super) const REFERENCE_IMAGE_MAX_PNG_BYTES: usize = 700_000;
/// 使用可配置 LLM 优化提示词。
#[command]
#[allow(clippy::too_many_arguments)]
pub async fn optimize_prompt(
    api_key: String,
    api_base: String,
    api_mode: String,
    proxy_url: String,
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

    let settings =
        require_prompt_optimizer_api_settings(api_key, api_base, api_mode, model, proxy_url)?;
    let api_key = settings.api_key;
    let api_base = settings.api_base;
    let api_mode = settings.api_mode;
    let model = settings.model;
    let proxy_url = settings.proxy_url;

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
        Some(load_reference_image_payload(&reference_image_path)?.data_url)
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

    let instructions = if uses_reference_image_understanding {
        reference_vision_prompt_optimizer_instructions()
    } else if has_reference_image {
        reference_prompt_optimizer_instructions()
    } else {
        prompt_optimizer_instructions()
    };

    let text_result =
        api_client::call_prompt_optimizer_text_api(api_client::PromptOptimizerTextRequest {
            api_mode,
            api_base: &api_base,
            api_key: &api_key,
            instructions: &instructions,
            input: &user_input,
            input_image_data_url: reference_image_data_url.as_deref(),
            model: &model,
            proxy_url: &proxy_url,
        })
        .await;

    let text = text_result.map_err(|err| {
        if uses_reference_image_understanding && is_reference_vision_input_error(&err) {
            build_reference_vision_error(&err, &model)
        } else {
            err
        }
    })?;

    parse_prompt_optimization_result(&text)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn build_prompt_optimizer_input(
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

pub(super) fn is_reference_vision_input_error(err: &str) -> bool {
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

pub(super) fn build_reference_vision_error(err: &str, model: &str) -> String {
    format!(
        "参考图视觉理解失败：{err}。解决方法：请把提示词优化模型 `{}` 更换为支持图像输入的多模态模型，并确认 API 地址支持 Responses 多模态输入；如果只想按参考图结构复刻但不需要优化器读取图片，请关闭“参考图视觉理解”后重试。",
        model.trim()
    )
}

pub(super) fn parse_prompt_optimization_result(
    text: &str,
) -> Result<PromptOptimizationResult, String> {
    let trimmed = text.trim();
    let raw = serde_json::from_str::<RawPromptOptimizationResult>(trimmed).map_err(|err| {
        format!(
            "提示词优化结果解析失败：模型必须只返回合法 JSON 对象，字段为 prompt、negative_prompt、grid_rows、grid_cols，不能包含 Markdown、代码块或额外说明。解析错误：{err}。响应预览：{}",
            summarize_log_text(trimmed)
        )
    })?;

    let prompt = require_non_empty_optimizer_text(raw.prompt, "prompt", trimmed)?;
    let negative_prompt = require_optimizer_text(raw.negative_prompt, "negative_prompt", trimmed)?;
    let grid_rows = require_optimizer_grid_size(raw.grid_rows, "grid_rows", trimmed)?;
    let grid_cols = require_optimizer_grid_size(raw.grid_cols, "grid_cols", trimmed)?;

    Ok(PromptOptimizationResult {
        prompt,
        negative_prompt,
        grid_rows,
        grid_cols,
    })
}

fn require_optimizer_text(
    value: Option<String>,
    field_name: &str,
    raw_response: &str,
) -> Result<String, String> {
    value.map(|value| value.trim().to_string()).ok_or_else(|| {
        build_prompt_optimizer_contract_error(format!("缺少 `{field_name}` 字段"), raw_response)
    })
}

fn require_non_empty_optimizer_text(
    value: Option<String>,
    field_name: &str,
    raw_response: &str,
) -> Result<String, String> {
    let value = require_optimizer_text(value, field_name, raw_response)?;
    if value.is_empty() {
        return Err(build_prompt_optimizer_contract_error(
            format!("`{field_name}` 字段不能为空"),
            raw_response,
        ));
    }
    Ok(value)
}

fn require_optimizer_grid_size(
    value: Option<u32>,
    field_name: &str,
    raw_response: &str,
) -> Result<u32, String> {
    let value = value.ok_or_else(|| {
        build_prompt_optimizer_contract_error(format!("缺少 `{field_name}` 字段"), raw_response)
    })?;
    if !(1..=20).contains(&value) {
        return Err(build_prompt_optimizer_contract_error(
            format!("`{field_name}` 必须是 1 到 20 之间的整数，实际为 {value}"),
            raw_response,
        ));
    }
    Ok(value)
}

fn build_prompt_optimizer_contract_error(reason: String, raw_response: &str) -> String {
    format!(
        "提示词优化结果格式无效：{reason}。解决方法：请更换或调整提示词优化模型，要求它严格只返回 JSON 对象，且包含 prompt、negative_prompt、grid_rows、grid_cols 四个字段；不要返回 Markdown、代码块、解释文字或缺失字段。响应预览：{}",
        summarize_log_text(raw_response)
    )
}
