mod presets;

pub use presets::*;

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// 一组可切换的 API 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiProfile {
    pub id: String,
    pub name: String,
    pub api_key: String,
    pub api_base: String,
    pub proxy_url: String,
    pub generation_api_mode: String,
    pub last_model: String,
    pub video_api_key: String,
    pub video_api_base: String,
    pub video_proxy_url: String,
    pub video_model: String,
    pub video_api_mode: String,
    pub prompt_optimizer_api_key: String,
    pub prompt_optimizer_api_base: String,
    pub prompt_optimizer_api_mode: String,
    pub prompt_optimizer_model: String,
    pub prompt_optimizer_vision: bool,
}

impl Default for ApiProfile {
    fn default() -> Self {
        Self {
            id: default_active_api_profile_id(),
            name: default_api_profile_name(),
            api_key: String::new(),
            api_base: String::new(),
            proxy_url: String::new(),
            generation_api_mode: default_generation_api_mode(),
            last_model: default_last_model(),
            video_api_key: String::new(),
            video_api_base: String::new(),
            video_proxy_url: String::new(),
            video_model: default_video_model(),
            video_api_mode: default_video_api_mode(),
            prompt_optimizer_api_key: String::new(),
            prompt_optimizer_api_base: String::new(),
            prompt_optimizer_api_mode: default_prompt_optimizer_api_mode(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            prompt_optimizer_vision: false,
        }
    }
}

/// 用户持久化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserConfig {
    pub api_profiles: Vec<ApiProfile>,
    pub active_api_profile_id: String,
    pub last_ratio: String,
    pub last_resolution: String,
    pub last_style: String,
    pub last_count: u32,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub prompt_history: VecDeque<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        let profile = ApiProfile::default();
        Self {
            last_ratio: "1:1".into(),
            last_resolution: "原始".into(),
            last_style: "anime".into(),
            last_count: 1,
            active_api_profile_id: profile.id.clone(),
            api_profiles: vec![profile],
            ffmpeg_path: String::new(),
            ffprobe_path: String::new(),
            prompt_history: VecDeque::with_capacity(100),
        }
    }
}

fn default_last_model() -> String {
    "gpt-5.3-codex".into()
}

fn default_video_model() -> String {
    "sora-2".into()
}

fn default_video_api_mode() -> String {
    "chat_completions".into()
}

fn default_generation_api_mode() -> String {
    "responses".into()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GenerationApiMode {
    Responses,
    ChatCompletions,
    ImagesGenerations,
    ImagesEditsJson,
    ImagesEditsMultipart,
}

impl GenerationApiMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
            Self::ImagesGenerations => "images_generations",
            Self::ImagesEditsJson => "images_edits_json",
            Self::ImagesEditsMultipart => "images_edits_multipart",
        }
    }
}

pub(crate) fn parse_generation_api_mode(
    value: &str,
    context: &str,
) -> Result<GenerationApiMode, String> {
    match value.trim() {
        "responses" => Ok(GenerationApiMode::Responses),
        "chat_completions" => Ok(GenerationApiMode::ChatCompletions),
        "images_generations" => Ok(GenerationApiMode::ImagesGenerations),
        "images_edits_json" => Ok(GenerationApiMode::ImagesEditsJson),
        "images_edits_multipart" => Ok(GenerationApiMode::ImagesEditsMultipart),
        "" => Err(format!(
            "{context}为空。请在设置 > API 配置 > 图片生成中选择调用方式后重试。"
        )),
        other => Err(format!(
            "{context}无效：{other}。配置文件中的 generation_api_mode 必须使用设置界面列出的值。"
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum VideoApiMode {
    ChatCompletions,
    Videos,
    VideosGenerations,
    VideosEdits,
    VideosExtensions,
}

impl VideoApiMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ChatCompletions => "chat_completions",
            Self::Videos => "videos",
            Self::VideosGenerations => "videos_generations",
            Self::VideosEdits => "videos_edits",
            Self::VideosExtensions => "videos_extensions",
        }
    }
}

pub(crate) fn parse_video_api_mode(value: &str, context: &str) -> Result<VideoApiMode, String> {
    match value.trim() {
        "chat_completions" => Ok(VideoApiMode::ChatCompletions),
        "videos" => Ok(VideoApiMode::Videos),
        "videos_generations" => Ok(VideoApiMode::VideosGenerations),
        "videos_edits" => Ok(VideoApiMode::VideosEdits),
        "videos_extensions" => Ok(VideoApiMode::VideosExtensions),
        "" => Err(format!(
            "{context}为空。请在设置 > API 配置 > 视频生成中选择调用方式后重试。"
        )),
        other => Err(format!(
            "{context}无效：{other}。配置文件中的 video_api_mode 必须使用设置界面列出的值。"
        )),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PromptOptimizerApiMode {
    Responses,
    ChatCompletions,
}

impl PromptOptimizerApiMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Responses => "responses",
            Self::ChatCompletions => "chat_completions",
        }
    }
}

pub(crate) fn parse_prompt_optimizer_api_mode(
    value: &str,
    context: &str,
) -> Result<PromptOptimizerApiMode, String> {
    match value.trim() {
        "responses" => Ok(PromptOptimizerApiMode::Responses),
        "chat_completions" => Ok(PromptOptimizerApiMode::ChatCompletions),
        "" => Err(format!(
            "{context}为空。解决方法：请在设置 > API 配置 > 提示词优化中选择 Responses 或 Chat Completions 后重试。"
        )),
        other => Err(format!(
            "{context}无效：{other}。解决方法：请在设置 > API 配置 > 提示词优化中重新选择调用方式；配置文件中 prompt_optimizer_api_mode 只能是 responses 或 chat_completions。"
        )),
    }
}

fn default_active_api_profile_id() -> String {
    "default".into()
}

fn default_api_profile_name() -> String {
    "默认 API".into()
}

fn default_prompt_optimizer_model() -> String {
    "deepseek-v4-flash".into()
}

fn default_prompt_optimizer_api_mode() -> String {
    "chat_completions".into()
}

fn build_config_load_error(path: &Path, reason: &str) -> String {
    format!(
        "读取配置文件失败：{}。原因：{}。解决方法：请关闭应用，备份并修复该 JSON 文件；如果不需要保留旧配置，请手动删除该文件后重启应用。",
        path.display(),
        reason
    )
}

impl UserConfig {
    /// 从文件加载配置
    pub fn load(path: &Path) -> Result<Self, String> {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(err) => {
                return Err(build_config_load_error(path, &format!("读取失败：{err}")));
            }
        };

        let mut config = serde_json::from_str::<Self>(&content)
            .map_err(|err| build_config_load_error(path, &format!("JSON 解析失败：{err}")))?;
        config
            .normalize_api_profiles()
            .map_err(|err| build_config_load_error(path, &err))?;
        Ok(config)
    }

    /// 校验并规范当前配置组。
    pub fn normalize_api_profiles(&mut self) -> Result<(), String> {
        if self.api_profiles.is_empty() {
            return Err("API 配置组为空。请至少保留一组 API 配置。".into());
        }

        let mut used_ids = HashSet::new();
        for (index, profile) in self.api_profiles.iter_mut().enumerate() {
            profile.id = profile.id.trim().to_string();
            if profile.id.is_empty() {
                return Err(format!("第{}组 API 配置缺少 id。", index + 1));
            }
            if !used_ids.insert(profile.id.clone()) {
                return Err(format!("API 配置 id 重复：{}。", profile.id));
            }
            profile.name = profile.name.trim().to_string();
            if profile.name.is_empty() {
                return Err(format!("第{}组 API 配置缺少名称。", index + 1));
            }
            let profile_context = format!("API 配置「{}」", profile.name);
            profile.api_key = profile.api_key.trim().to_string();
            profile.api_base = profile.api_base.trim().to_string();
            profile.proxy_url = profile.proxy_url.trim().to_string();
            profile.generation_api_mode = parse_generation_api_mode(
                &profile.generation_api_mode,
                &format!("{profile_context}图片生成调用方式"),
            )?
            .as_str()
            .into();
            profile.last_model = profile.last_model.trim().to_string();
            profile.video_api_key = profile.video_api_key.trim().to_string();
            profile.video_api_base = profile.video_api_base.trim().to_string();
            profile.video_proxy_url = profile.video_proxy_url.trim().to_string();
            profile.video_model = profile.video_model.trim().to_string();
            profile.video_api_mode = parse_video_api_mode(
                &profile.video_api_mode,
                &format!("{profile_context}视频生成调用方式"),
            )?
            .as_str()
            .into();
            profile.prompt_optimizer_api_key = profile.prompt_optimizer_api_key.trim().to_string();
            profile.prompt_optimizer_api_base =
                profile.prompt_optimizer_api_base.trim().to_string();
            profile.prompt_optimizer_api_mode = parse_prompt_optimizer_api_mode(
                &profile.prompt_optimizer_api_mode,
                &format!("{profile_context}提示词优化调用方式"),
            )?
            .as_str()
            .into();
            profile.prompt_optimizer_model = profile.prompt_optimizer_model.trim().to_string();
        }

        self.active_api_profile_id = self.active_api_profile_id.trim().to_string();
        if !self
            .api_profiles
            .iter()
            .any(|profile| profile.id == self.active_api_profile_id)
        {
            return Err(format!(
                "活动 API 配置不存在：{}。",
                self.active_api_profile_id
            ));
        }
        Ok(())
    }

    /// 保存配置到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(path, &json).map_err(|e| format!("写入配置文件失败: {}", e))
    }
}

pub fn get_style_suffix(style_key: &str) -> Result<String, String> {
    get_presets()
        .styles
        .into_iter()
        .find(|style| style.key == style_key)
        .map(|style| style.prompt_suffix)
        .ok_or_else(|| format!("生成风格不存在：{style_key}"))
}

/// 获取宽高比尺寸元组
pub fn get_ratio_tuple(ratio_key: &str) -> Result<(u32, u32), String> {
    let (width, height) = ratio_key
        .trim()
        .split_once(':')
        .ok_or_else(|| format!("生成宽高比格式无效：{ratio_key}"))?;
    let width = width
        .parse::<u32>()
        .map_err(|_| format!("生成宽高比格式无效：{ratio_key}"))?;
    let height = height
        .parse::<u32>()
        .map_err(|_| format!("生成宽高比格式无效：{ratio_key}"))?;
    if width == 0 || height == 0 {
        return Err(format!("生成宽高比必须大于 0：{ratio_key}"));
    }
    let divisor = greatest_common_divisor(width, height);
    Ok((width / divisor, height / divisor))
}

fn greatest_common_divisor(mut left: u32, mut right: u32) -> u32 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

/// 应用全局状态
pub struct AppState {
    pub config: parking_lot::Mutex<UserConfig>,
    pub app_data_dir: PathBuf,
    pub config_path: PathBuf,
    pub log_dir: PathBuf,
    pub workbench_records_path: PathBuf,
    pub workspace_path: PathBuf,
    pub default_save_dir: PathBuf,
    pub locks_dir: PathBuf,
}

#[cfg(test)]
mod tests;
