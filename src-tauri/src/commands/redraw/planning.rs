use super::store::validate_range;
use super::types::*;

const MAX_TOTAL_FRAMES: u32 = 64;
pub(super) const MAX_FINAL_COLS: u32 = 20;
const MAX_GROUP_AXIS: u32 = 4;
const MAX_GROUP_FRAMES: u32 = 9;

pub(super) fn build_manifest(request: CreateRedrawRunRequest) -> Result<RedrawRunManifest, String> {
    validate_create_request(&request)?;
    let group_capacity = request.group_rows * request.group_cols;
    let batch_count = request.total_frames.div_ceil(group_capacity);
    let final_rows = request.total_frames.div_ceil(request.final_cols);
    let id = format!("redraw-{}", chrono::Local::now().timestamp_micros());
    let batches = (0..batch_count)
        .map(|index| {
            let global_start = index * group_capacity;
            let valid_count = group_capacity.min(request.total_frames - global_start);
            RedrawBatchRecord {
                index,
                global_start,
                valid_count,
                status: "pending_input".into(),
                input_path: String::new(),
                output_path: String::new(),
                frame_paths: Vec::new(),
                cell_width: None,
                cell_height: None,
                error: String::new(),
            }
        })
        .collect();
    Ok(RedrawRunManifest {
        id,
        status: "preparing".into(),
        source_name: request.source_name.trim().to_string(),
        total_frames: request.total_frames,
        final_cols: request.final_cols,
        final_rows,
        group_rows: request.group_rows,
        group_cols: request.group_cols,
        prompt: request.prompt.trim().to_string(),
        negative_prompt: request.negative_prompt.trim().to_string(),
        style: request.style.trim().to_string(),
        resolution: request.resolution.trim().to_string(),
        api: request.api,
        extraction: request.extraction,
        batches,
        final_output_path: String::new(),
    })
}

fn validate_create_request(request: &CreateRedrawRunRequest) -> Result<(), String> {
    validate_range(request.total_frames, 2, MAX_TOTAL_FRAMES, "总帧数")?;
    validate_range(request.final_cols, 1, MAX_FINAL_COLS, "最终列数")?;
    validate_range(request.group_rows, 1, MAX_GROUP_AXIS, "分组行数")?;
    validate_range(request.group_cols, 1, MAX_GROUP_AXIS, "分组列数")?;
    let capacity = request.group_rows * request.group_cols;
    if capacity > MAX_GROUP_FRAMES {
        return Err(format!(
            "每组最多 {MAX_GROUP_FRAMES} 帧，当前为 {capacity} 帧"
        ));
    }
    if capacity > request.total_frames {
        return Err(format!(
            "每组容量 {capacity} 不能大于总帧数 {}",
            request.total_frames
        ));
    }
    let api_mode =
        crate::config::parse_generation_api_mode(&request.api.api_mode, "分组重绘图片调用方式")?;
    if api_mode == crate::config::GenerationApiMode::ImagesGenerations {
        return Err(
            "分组重绘需要上传当前批次网格和上一批末帧参考，不支持 /images/generations 纯文本生图模式。请切换到 Responses、Chat Completions 或 /images/edits。".into(),
        );
    }
    for (value, label) in [
        (&request.source_name, "视频名称"),
        (&request.prompt, "重绘提示词"),
        (&request.style, "重绘风格"),
        (&request.resolution, "重绘分辨率"),
        (&request.api.profile_id, "API 配置组 ID"),
        (&request.api.api_base, "图片 API 地址"),
        (&request.api.model, "图片模型"),
    ] {
        if value.trim().is_empty() {
            return Err(format!("{label}为空"));
        }
    }
    if !request.extraction.start_seconds.is_finite()
        || !request.extraction.end_seconds.is_finite()
        || request.extraction.end_seconds <= request.extraction.start_seconds
    {
        return Err("抽帧起止时间无效".into());
    }
    Ok(())
}

pub(super) fn build_redraw_batch_prompt(
    manifest: &RedrawRunManifest,
    batch: &RedrawBatchRecord,
) -> String {
    let padding_count = manifest.group_rows * manifest.group_cols - batch.valid_count;
    let padding_note = if padding_count > 0 {
        format!(
            "本批最后 {padding_count} 格是末帧复制的占位格，请保持与它前面的末帧一致，不要创造新动作。"
        )
    } else {
        "本批没有占位格。".into()
    };
    let reference_note = if batch.index == 0 {
        "第一张参考图是本批需要重绘的目标网格。本批负责建立后续批次必须沿用的角色身份、服装、画风、背景、镜头和脚底基线。".into()
    } else {
        "第一张参考图是本批需要重绘的目标网格。第二张参考图是上一批已经生成的最后一帧，只用于连续性参考。只输出第一张参考图对应的固定行列网格，不得把第二张参考图作为额外格子输出。本批第一帧必须从第二张参考图自然延续，保持角色身份、脸型、发型、服装、颜色、身体比例、线条、背景、镜头距离、角色占比和脚底基线一致。不得复制上一帧代替当前动作，不得重置动作或改变帧顺序。".into()
    };
    [
        manifest.prompt.trim().to_string(),
        format!(
            "这是第 {}/{} 批动画帧。",
            batch.index + 1,
            manifest.batches.len()
        ),
        format!(
            "严格输出 {} 行 {} 列，共 {} 个等尺寸格子。",
            manifest.group_rows,
            manifest.group_cols,
            manifest.group_rows * manifest.group_cols
        ),
        "必须按参考图从左到右、从上到下逐格复刻动作，不得新增、删除、交换、合并或跨格。".into(),
        "所有格子保持同一角色身份、服装、比例、画风、镜头距离、背景、脚底基线和安全留白。".into(),
        "只输出完整网格图片；不要边框、间距、编号、文字或水印。".into(),
        reference_note,
        padding_note,
    ]
    .join("\n")
}
