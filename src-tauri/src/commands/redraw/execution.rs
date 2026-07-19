use std::path::Path;

use super::planning::build_redraw_batch_prompt;
use super::store::validate_file_inside;
use super::types::RedrawRunManifest;

pub(super) fn batch_execution_parameters(
    active_dir: &Path,
    manifest: &RedrawRunManifest,
    batch_index: u32,
) -> Result<(String, Vec<String>), String> {
    let batch = manifest
        .batches
        .get(batch_index as usize)
        .ok_or_else(|| format!("第{}批不存在", batch_index + 1))?;
    if batch.status != "pending" && batch.status != "failed" {
        return Err(format!(
            "第{}批当前状态 `{}` 不允许开始生成",
            batch_index + 1,
            batch.status
        ));
    }
    validate_file_inside(
        &active_dir.join("inputs"),
        Path::new(&batch.input_path),
        "分组输入图",
    )?;
    let mut references = vec![batch.input_path.clone()];
    if batch_index > 0 {
        let previous = &manifest.batches[batch_index as usize - 1];
        if previous.status != "succeeded" {
            return Err(format!(
                "第{}批尚未成功，不能开始第{}批",
                batch_index,
                batch_index + 1
            ));
        }
        if previous.frame_paths.len() != previous.valid_count as usize {
            return Err(format!("第{}批拆分帧数量不完整", batch_index));
        }
        let anchor = previous
            .frame_paths
            .last()
            .ok_or_else(|| format!("第{}批缺少末帧连续性参考", batch_index))?;
        validate_file_inside(
            &active_dir.join("frames"),
            Path::new(anchor),
            "上一批末帧连续性参考",
        )?;
        references.push(anchor.clone());
    }
    Ok((build_redraw_batch_prompt(manifest, batch), references))
}
