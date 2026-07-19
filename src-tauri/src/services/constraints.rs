use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackgroundMode {
    Preserve,
    Solid,
    Transparent,
    Custom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Framing {
    Preserve,
    FullBody,
    UpperBody,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImageGenerationConstraints {
    pub enabled: bool,
    pub background_mode: BackgroundMode,
    pub background_description: String,
    pub framing: Framing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoGenerationConstraints {
    pub enabled: bool,
    pub background_mode: BackgroundMode,
    pub background_description: String,
    pub framing: Framing,
    pub fixed_camera: bool,
    pub loop_action: bool,
}

pub fn build_sprite_image_prompt(
    prompt: &str,
    constraints: &ImageGenerationConstraints,
    rows: u32,
    cols: u32,
    has_reference: bool,
) -> Result<String, String> {
    let prompt = require_prompt(prompt)?;
    if !constraints.enabled {
        return Ok(prompt.into());
    }
    if rows == 0 || cols == 0 {
        return Err("序列帧行列数必须是正整数".into());
    }
    Ok(join_prompt(
        prompt,
        &[
            "生成用于动画制作的序列帧图。".into(),
            format!(
                "严格输出 {rows} 行 {cols} 列，共 {} 个等尺寸格子，并按从左到右、从上到下排列。",
                rows * cols
            ),
            "每个格子只能包含一帧；不得新增、删除、交换、合并或跨格；不要添加边框、间距、编号、文字或水印。".into(),
            "所有帧保持同一角色身份、脸部、发型、服装、配色、身体比例、镜头距离、脚底基线和安全留白。".into(),
            framing_instruction(constraints.framing).into(),
            image_background_instruction(constraints, has_reference)?,
        ],
    ))
}

pub fn build_redraw_constraint_prompt(
    prompt: &str,
    constraints: &ImageGenerationConstraints,
) -> Result<String, String> {
    let prompt = require_prompt(prompt)?;
    if !constraints.enabled {
        return Ok(prompt.into());
    }
    Ok(join_prompt(
        prompt,
        &[
            "保持同一角色的身份、脸部、发型、服装、配色和身体比例一致。".into(),
            framing_instruction(constraints.framing).into(),
            image_background_instruction(constraints, true)?,
        ],
    ))
}

pub fn build_video_prompt(
    prompt: &str,
    constraints: &VideoGenerationConstraints,
    has_reference: bool,
) -> Result<String, String> {
    let prompt = require_prompt(prompt)?;
    if !constraints.enabled {
        return Ok(prompt.into());
    }
    if matches!(constraints.background_mode, BackgroundMode::Transparent) {
        return Err("视频生成不支持透明背景约束".into());
    }
    let mut instructions = vec![
        "生成便于连续抽帧的单场景、单动作视频。".into(),
        "角色身份、脸部、发型、服装、配色和身体比例必须保持一致，避免闪烁、变形或凭空出现物体。"
            .into(),
        framing_instruction(constraints.framing).into(),
        video_background_instruction(constraints, has_reference)?,
    ];
    if constraints.fixed_camera {
        instructions.push(
            "使用固定机位、固定视角和固定焦距；不要切镜、转场、推拉、摇移或旋转镜头。".into(),
        );
    }
    if constraints.loop_action {
        instructions.push("动作连续自然，首尾姿态尽可能衔接，适合循环播放。".into());
    }
    Ok(join_prompt(prompt, &instructions))
}

fn image_background_instruction(
    constraints: &ImageGenerationConstraints,
    has_reference: bool,
) -> Result<String, String> {
    match constraints.background_mode {
        BackgroundMode::Preserve if has_reference => {
            Ok("背景必须与参考图保持一致，所有格子的背景内容和光照稳定。".into())
        }
        BackgroundMode::Preserve => Ok("所有格子使用统一、静止且光照稳定的背景。".into()),
        BackgroundMode::Transparent => {
            Ok("使用完全透明背景，不要投影、地面、场景或背景装饰。".into())
        }
        BackgroundMode::Solid => Ok(format!(
            "所有格子使用同一纯色背景：{}。",
            require_background_description(&constraints.background_description)?
        )),
        BackgroundMode::Custom => Ok(format!(
            "所有格子使用同一静止背景：{}。",
            require_background_description(&constraints.background_description)?
        )),
    }
}

fn video_background_instruction(
    constraints: &VideoGenerationConstraints,
    has_reference: bool,
) -> Result<String, String> {
    match constraints.background_mode {
        BackgroundMode::Preserve if has_reference => {
            Ok("背景、光照和构图与参考图保持一致，背景全程静止稳定。".into())
        }
        BackgroundMode::Preserve => Ok("使用统一、静止且光照稳定的背景。".into()),
        BackgroundMode::Solid => Ok(format!(
            "全程使用同一纯色静止背景：{}。",
            require_background_description(&constraints.background_description)?
        )),
        BackgroundMode::Custom => Ok(format!(
            "全程使用同一静止背景：{}。",
            require_background_description(&constraints.background_description)?
        )),
        BackgroundMode::Transparent => Err("视频生成不支持透明背景约束".into()),
    }
}

fn framing_instruction(framing: Framing) -> &'static str {
    match framing {
        Framing::Preserve => "保持输入或用户描述中的角色景别和构图。",
        Framing::FullBody => "角色全身必须始终完整显示，四肢不得离开画面或被裁切。",
        Framing::UpperBody => "保持稳定的半身构图，头部、肩部和手臂不得被裁切。",
    }
}

fn require_prompt(prompt: &str) -> Result<&str, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        Err("生成提示词为空".into())
    } else {
        Ok(prompt)
    }
}

fn require_background_description(value: &str) -> Result<&str, String> {
    let value = value.trim();
    if value.is_empty() {
        Err("当前背景模式需要填写背景说明".into())
    } else {
        Ok(value)
    }
}

fn join_prompt(prompt: &str, instructions: &[String]) -> String {
    std::iter::once(prompt.to_string())
        .chain(std::iter::once("序列帧生成约束：".into()))
        .chain(instructions.iter().cloned())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image_constraints() -> ImageGenerationConstraints {
        ImageGenerationConstraints {
            enabled: true,
            background_mode: BackgroundMode::Preserve,
            background_description: String::new(),
            framing: Framing::FullBody,
        }
    }

    #[test]
    fn disabled_constraints_keep_trimmed_prompt() {
        let mut constraints = image_constraints();
        constraints.enabled = false;
        assert_eq!(
            build_sprite_image_prompt("  角色奔跑  ", &constraints, 4, 4, false).unwrap(),
            "角色奔跑"
        );
    }

    #[test]
    fn sprite_prompt_contains_layout_and_reference_rules() {
        let prompt =
            build_sprite_image_prompt("像素角色奔跑", &image_constraints(), 4, 4, true).unwrap();
        assert!(prompt.contains("4 行 4 列，共 16 个等尺寸格子"));
        assert!(prompt.contains("背景必须与参考图保持一致"));
        assert!(prompt.contains("角色全身必须始终完整显示"));
    }

    #[test]
    fn custom_background_requires_description() {
        let mut constraints = image_constraints();
        constraints.background_mode = BackgroundMode::Custom;
        let error = build_redraw_constraint_prompt("重绘", &constraints).unwrap_err();
        assert!(error.contains("需要填写背景说明"));
    }

    #[test]
    fn video_prompt_contains_selected_motion_rules() {
        let constraints = VideoGenerationConstraints {
            enabled: true,
            background_mode: BackgroundMode::Solid,
            background_description: "纯绿色 #00ff00".into(),
            framing: Framing::FullBody,
            fixed_camera: true,
            loop_action: true,
        };
        let prompt = build_video_prompt("角色挥剑", &constraints, false).unwrap();
        assert!(prompt.contains("固定机位、固定视角和固定焦距"));
        assert!(prompt.contains("首尾姿态尽可能衔接"));
        assert!(prompt.contains("纯绿色 #00ff00"));
    }
}
