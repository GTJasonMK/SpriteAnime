use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

/// 风格选项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleOption {
    pub key: String,
    pub label: String,
    pub prompt_suffix: String,
}

/// 宽高比映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RatioOption {
    pub key: String,
    pub width: u32,
    pub height: u32,
}

/// 预设数据，一次性返回给前端
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetsPayload {
    pub models: Vec<String>,
    pub styles: Vec<StyleOption>,
    pub ratios: Vec<RatioOption>,
    pub resolutions: Vec<String>,
}

/// 获取所有预设选项
pub fn get_presets() -> PresetsPayload {
    PresetsPayload {
        models: vec!["gpt-5.3-codex".into()],
        styles: vec![
            StyleOption {
                key: "none".into(),
                label: "无".into(),
                prompt_suffix: String::new(),
            },
            StyleOption {
                key: "realistic".into(),
                label: "写实摄影".into(),
                prompt_suffix: "写实摄影风格，高清晰度，自然光线，真实质感".into(),
            },
            StyleOption {
                key: "anime".into(),
                label: "动漫卡通".into(),
                prompt_suffix: "动漫卡通风格，鲜艳色彩，清晰线条，角色设计".into(),
            },
            StyleOption {
                key: "oil-painting".into(),
                label: "油画艺术".into(),
                prompt_suffix: "油画艺术风格，丰富笔触，古典构图，艺术光影".into(),
            },
            StyleOption {
                key: "3d-render".into(),
                label: "3D渲染".into(),
                prompt_suffix: "3D渲染风格，逼真材质，全局光照，景深效果".into(),
            },
            StyleOption {
                key: "pixel-art".into(),
                label: "像素艺术".into(),
                prompt_suffix: "像素艺术风格，复古游戏画面，清晰像素块，有限色板".into(),
            },
            StyleOption {
                key: "cyberpunk".into(),
                label: "赛博朋克".into(),
                prompt_suffix: "赛博朋克风格，霓虹灯光，高科技都市，暗色调".into(),
            },
            StyleOption {
                key: "watercolor".into(),
                label: "水彩画".into(),
                prompt_suffix: "水彩画风格，柔和色彩，流动感，艺术留白".into(),
            },
            StyleOption {
                key: "sketch".into(),
                label: "素描速写".into(),
                prompt_suffix: "素描速写风格，黑白线条，明暗对比，手绘质感".into(),
            },
            StyleOption {
                key: "flat-illustration".into(),
                label: "扁平插画".into(),
                prompt_suffix: "扁平插画风格，简洁图形，明快配色，现代设计感".into(),
            },
        ],
        ratios: vec![
            RatioOption {
                key: "1:1".into(),
                width: 1,
                height: 1,
            },
            RatioOption {
                key: "16:9".into(),
                width: 16,
                height: 9,
            },
            RatioOption {
                key: "9:16".into(),
                width: 9,
                height: 16,
            },
            RatioOption {
                key: "4:3".into(),
                width: 4,
                height: 3,
            },
            RatioOption {
                key: "3:4".into(),
                width: 3,
                height: 4,
            },
            RatioOption {
                key: "3:2".into(),
                width: 3,
                height: 2,
            },
            RatioOption {
                key: "2:3".into(),
                width: 2,
                height: 3,
            },
            RatioOption {
                key: "21:9".into(),
                width: 21,
                height: 9,
            },
        ],
        resolutions: vec!["原始".into(), "1K".into(), "2K".into()],
    }
}

/// 一组可切换的 API 配置。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiProfile {
    #[serde(default)]
    pub id: String,
    #[serde(default = "default_api_profile_name")]
    pub name: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_base: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default = "default_generation_api_mode")]
    pub generation_api_mode: String,
    #[serde(default = "default_last_model")]
    pub last_model: String,
    #[serde(default)]
    pub video_api_key: String,
    #[serde(default)]
    pub video_api_base: String,
    #[serde(default)]
    pub video_proxy_url: String,
    #[serde(default = "default_video_model")]
    pub video_model: String,
    #[serde(default = "default_video_api_mode")]
    pub video_api_mode: String,
    #[serde(default)]
    pub prompt_optimizer_api_key: String,
    #[serde(default = "default_prompt_optimizer_api_base")]
    pub prompt_optimizer_api_base: String,
    #[serde(default = "default_prompt_optimizer_model")]
    pub prompt_optimizer_model: String,
    #[serde(default)]
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
            prompt_optimizer_api_base: default_prompt_optimizer_api_base(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            prompt_optimizer_vision: false,
        }
    }
}

/// 用户持久化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub api_profiles: Vec<ApiProfile>,
    #[serde(default)]
    pub active_api_profile_id: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_base: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default = "default_generation_api_mode")]
    pub generation_api_mode: String,
    #[serde(default = "default_last_model")]
    pub last_model: String,
    #[serde(default)]
    pub video_api_key: String,
    #[serde(default)]
    pub video_api_base: String,
    #[serde(default)]
    pub video_proxy_url: String,
    #[serde(default = "default_video_model")]
    pub video_model: String,
    #[serde(default = "default_video_api_mode")]
    pub video_api_mode: String,
    #[serde(default = "default_last_ratio")]
    pub last_ratio: String,
    #[serde(default = "default_last_resolution")]
    pub last_resolution: String,
    #[serde(default = "default_last_style")]
    pub last_style: String,
    #[serde(default = "default_last_count")]
    pub last_count: u32,
    #[serde(default)]
    pub prompt_optimizer_api_key: String,
    #[serde(default = "default_prompt_optimizer_api_base")]
    pub prompt_optimizer_api_base: String,
    #[serde(default = "default_prompt_optimizer_model")]
    pub prompt_optimizer_model: String,
    #[serde(default)]
    pub prompt_optimizer_vision: bool,
    #[serde(default)]
    pub save_dir: String,
    #[serde(default)]
    pub ffmpeg_path: String,
    #[serde(default)]
    pub ffprobe_path: String,
    #[serde(default)]
    pub prompt_history: VecDeque<String>,
}

impl Default for UserConfig {
    fn default() -> Self {
        let profile = ApiProfile::default();
        Self {
            api_key: profile.api_key.clone(),
            api_base: profile.api_base.clone(),
            proxy_url: profile.proxy_url.clone(),
            generation_api_mode: profile.generation_api_mode.clone(),
            last_model: profile.last_model.clone(),
            video_api_key: profile.video_api_key.clone(),
            video_api_base: profile.video_api_base.clone(),
            video_proxy_url: profile.video_proxy_url.clone(),
            video_model: profile.video_model.clone(),
            video_api_mode: profile.video_api_mode.clone(),
            last_ratio: "1:1".into(),
            last_resolution: "原始".into(),
            last_style: "anime".into(),
            last_count: 1,
            prompt_optimizer_api_key: profile.prompt_optimizer_api_key.clone(),
            prompt_optimizer_api_base: profile.prompt_optimizer_api_base.clone(),
            prompt_optimizer_model: profile.prompt_optimizer_model.clone(),
            prompt_optimizer_vision: profile.prompt_optimizer_vision,
            active_api_profile_id: profile.id.clone(),
            api_profiles: vec![profile],
            save_dir: String::new(),
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

fn normalize_generation_api_mode(value: &str) -> String {
    match value.trim() {
        "chat_completions" | "chat-completions" | "chat/completions" => "chat_completions".into(),
        _ => default_generation_api_mode(),
    }
}

fn normalize_video_api_mode(value: &str) -> String {
    match value.trim() {
        "videos" | "video" | "/videos" | "v1/videos" | "/v1/videos" => "videos".into(),
        "chat_completions"
        | "chat-completions"
        | "chat/completions"
        | "/chat/completions"
        | "v1/chat/completions"
        | "/v1/chat/completions" => "chat_completions".into(),
        _ => default_video_api_mode(),
    }
}

fn default_last_ratio() -> String {
    "1:1".into()
}

fn default_last_resolution() -> String {
    "原始".into()
}

fn default_last_style() -> String {
    "anime".into()
}

fn default_last_count() -> u32 {
    1
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

fn default_prompt_optimizer_api_base() -> String {
    "https://api.deepseek.com".into()
}

impl UserConfig {
    /// 从文件加载配置
    pub fn load(path: &PathBuf) -> Self {
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Self>(&content) {
                Ok(mut config) => {
                    config.normalize_api_profiles();
                    eprintln!("[config] 配置加载成功: {}", path.display());
                    config
                }
                Err(e) => {
                    eprintln!("[config] 配置解析失败，使用默认值: {e}");
                    Self::default()
                }
            },
            Err(e) => {
                eprintln!("[config] 配置文件不存在或无法读取 ({}), 使用默认值", e);
                Self::default()
            }
        }
    }

    /// 迁移旧的单组 API 配置，并保持顶层字段与当前配置组一致。
    pub fn normalize_api_profiles(&mut self) {
        if self.api_profiles.is_empty() {
            self.api_profiles.push(self.legacy_api_profile());
        }

        let mut used_ids = HashSet::new();
        for (index, profile) in self.api_profiles.iter_mut().enumerate() {
            let fallback_id = if index == 0 {
                default_active_api_profile_id()
            } else {
                format!("api-profile-{}", index + 1)
            };
            let base_id = non_empty_trimmed(&profile.id).unwrap_or(fallback_id);
            profile.id = unique_api_profile_id(&base_id, &used_ids);
            used_ids.insert(profile.id.clone());

            profile.name = non_empty_trimmed(&profile.name)
                .unwrap_or_else(|| format!("API 配置 {}", index + 1));
            profile.api_key = profile.api_key.trim().to_string();
            profile.api_base = profile.api_base.trim().to_string();
            profile.proxy_url = profile.proxy_url.trim().to_string();
            profile.generation_api_mode =
                normalize_generation_api_mode(&profile.generation_api_mode);
            profile.last_model =
                non_empty_trimmed(&profile.last_model).unwrap_or_else(default_last_model);
            profile.video_api_key = profile.video_api_key.trim().to_string();
            profile.video_api_base = profile.video_api_base.trim().to_string();
            profile.video_proxy_url = profile.video_proxy_url.trim().to_string();
            profile.video_model =
                non_empty_trimmed(&profile.video_model).unwrap_or_else(default_video_model);
            profile.video_api_mode = normalize_video_api_mode(&profile.video_api_mode);
            profile.prompt_optimizer_api_key = profile.prompt_optimizer_api_key.trim().to_string();
            profile.prompt_optimizer_api_base =
                non_empty_trimmed(&profile.prompt_optimizer_api_base)
                    .unwrap_or_else(default_prompt_optimizer_api_base);
            profile.prompt_optimizer_model = non_empty_trimmed(&profile.prompt_optimizer_model)
                .unwrap_or_else(default_prompt_optimizer_model);
        }

        let active_exists = self
            .api_profiles
            .iter()
            .any(|profile| profile.id == self.active_api_profile_id);
        if !active_exists {
            self.active_api_profile_id = self
                .api_profiles
                .first()
                .map(|profile| profile.id.clone())
                .unwrap_or_else(default_active_api_profile_id);
        }
        self.sync_active_profile_to_legacy_fields();
    }

    /// 固定素材库到应用旁的数据目录，避免用户数据散落到系统目录或外部目录。
    pub fn use_portable_save_dir(&mut self, default_save_dir: &Path) {
        self.save_dir = default_save_dir.to_string_lossy().to_string();
    }

    fn legacy_api_profile(&self) -> ApiProfile {
        ApiProfile {
            id: non_empty_trimmed(&self.active_api_profile_id)
                .unwrap_or_else(default_active_api_profile_id),
            name: default_api_profile_name(),
            api_key: self.api_key.trim().to_string(),
            api_base: self.api_base.trim().to_string(),
            proxy_url: self.proxy_url.trim().to_string(),
            generation_api_mode: normalize_generation_api_mode(&self.generation_api_mode),
            last_model: non_empty_trimmed(&self.last_model).unwrap_or_else(default_last_model),
            video_api_key: self.video_api_key.trim().to_string(),
            video_api_base: self.video_api_base.trim().to_string(),
            video_proxy_url: self.video_proxy_url.trim().to_string(),
            video_model: non_empty_trimmed(&self.video_model).unwrap_or_else(default_video_model),
            video_api_mode: normalize_video_api_mode(&self.video_api_mode),
            prompt_optimizer_api_key: self.prompt_optimizer_api_key.trim().to_string(),
            prompt_optimizer_api_base: non_empty_trimmed(&self.prompt_optimizer_api_base)
                .unwrap_or_else(default_prompt_optimizer_api_base),
            prompt_optimizer_model: non_empty_trimmed(&self.prompt_optimizer_model)
                .unwrap_or_else(default_prompt_optimizer_model),
            prompt_optimizer_vision: self.prompt_optimizer_vision,
        }
    }

    fn sync_active_profile_to_legacy_fields(&mut self) {
        let Some(profile) = self
            .api_profiles
            .iter()
            .find(|profile| profile.id == self.active_api_profile_id)
        else {
            return;
        };

        self.api_key = profile.api_key.clone();
        self.api_base = profile.api_base.clone();
        self.proxy_url = profile.proxy_url.clone();
        self.generation_api_mode = profile.generation_api_mode.clone();
        self.last_model = profile.last_model.clone();
        self.video_api_key = profile.video_api_key.clone();
        self.video_api_base = profile.video_api_base.clone();
        self.video_proxy_url = profile.video_proxy_url.clone();
        self.video_model = profile.video_model.clone();
        self.video_api_mode = profile.video_api_mode.clone();
        self.prompt_optimizer_api_key = profile.prompt_optimizer_api_key.clone();
        self.prompt_optimizer_api_base = profile.prompt_optimizer_api_base.clone();
        self.prompt_optimizer_model = profile.prompt_optimizer_model.clone();
        self.prompt_optimizer_vision = profile.prompt_optimizer_vision;
    }

    /// 保存配置到文件
    pub fn save(&self, path: &PathBuf) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建配置目录失败: {}", e))?;
        }
        let json =
            serde_json::to_string_pretty(self).map_err(|e| format!("序列化配置失败: {}", e))?;
        std::fs::write(path, &json).map_err(|e| format!("写入配置文件失败: {}", e))?;
        eprintln!("[config] 配置已保存: {}", path.display());
        Ok(())
    }

    /// 根据风格key获取prompt后缀
    pub fn get_style_suffix(&self, style_key: &str) -> String {
        let presets = get_presets();
        presets
            .styles
            .iter()
            .find(|s| s.key == style_key)
            .map(|s| s.prompt_suffix.clone())
            .unwrap_or_default()
    }
}

fn non_empty_trimmed(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn unique_api_profile_id(base_id: &str, used_ids: &HashSet<String>) -> String {
    let mut candidate = base_id.trim().to_string();
    if candidate.is_empty() {
        candidate = default_active_api_profile_id();
    }
    if !used_ids.contains(&candidate) {
        return candidate;
    }

    let base = candidate;
    let mut index = 2;
    loop {
        let candidate = format!("{base}-{index}");
        if !used_ids.contains(&candidate) {
            return candidate;
        }
        index += 1;
    }
}

/// 获取宽高比尺寸元组
pub fn get_ratio_tuple(ratio_key: &str) -> (u32, u32) {
    let presets = get_presets();
    presets
        .ratios
        .iter()
        .find(|r| r.key == ratio_key)
        .map(|r| (r.width, r.height))
        .unwrap_or((1, 1))
}

/// 应用全局状态
pub struct AppState {
    pub config: parking_lot::Mutex<UserConfig>,
    pub prompt_history: parking_lot::Mutex<VecDeque<String>>,
    pub app_data_dir: PathBuf,
    pub config_path: PathBuf,
    pub log_dir: PathBuf,
    pub workbench_records_path: PathBuf,
    pub default_save_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_user_config_keeps_existing_api_key() {
        let config: UserConfig = serde_json::from_str(r#"{"api_key":"secret"}"#).unwrap();
        assert_eq!(config.api_key, "secret");
        assert_eq!(config.api_profiles.len(), 0);
        assert_eq!(config.last_model, "gpt-5.3-codex");
        assert_eq!(config.generation_api_mode, "responses");
        assert_eq!(config.last_ratio, "1:1");
        assert_eq!(config.last_resolution, "原始");
        assert_eq!(config.last_style, "anime");
        assert_eq!(config.last_count, 1);
        assert_eq!(config.video_api_key, "");
        assert_eq!(config.video_api_base, "");
        assert_eq!(config.video_proxy_url, "");
        assert_eq!(config.video_model, "sora-2");
        assert_eq!(config.video_api_mode, "chat_completions");
        assert_eq!(config.prompt_optimizer_api_base, "https://api.deepseek.com");
        assert_eq!(config.prompt_optimizer_model, "deepseek-v4-flash");
        assert!(!config.prompt_optimizer_vision);
        assert_eq!(config.ffmpeg_path, "");
        assert_eq!(config.ffprobe_path, "");
    }

    #[test]
    fn normalize_api_profiles_migrates_legacy_fields() {
        let mut config: UserConfig = serde_json::from_str(
            r#"{
                "api_key":"secret",
                "api_base":"http://localhost:8787/v1",
                "proxy_url":"http://127.0.0.1:7890",
                "generation_api_mode":"chat/completions",
                "last_model":"image-model",
                "video_api_key":"video-secret",
                "video_api_base":"https://video.example/v1",
                "video_proxy_url":"http://127.0.0.1:7891",
                "video_model":"video-model",
                "video_api_mode":"videos",
                "prompt_optimizer_api_key":"prompt-secret",
                "prompt_optimizer_api_base":"https://prompt.example/v1",
                "prompt_optimizer_model":"prompt-model",
                "prompt_optimizer_vision":true
            }"#,
        )
        .unwrap();

        config.normalize_api_profiles();

        assert_eq!(config.active_api_profile_id, "default");
        assert_eq!(config.api_profiles.len(), 1);
        let profile = &config.api_profiles[0];
        assert_eq!(profile.name, "默认 API");
        assert_eq!(profile.api_key, "secret");
        assert_eq!(profile.api_base, "http://localhost:8787/v1");
        assert_eq!(profile.proxy_url, "http://127.0.0.1:7890");
        assert_eq!(profile.generation_api_mode, "chat_completions");
        assert_eq!(profile.last_model, "image-model");
        assert_eq!(profile.video_api_key, "video-secret");
        assert_eq!(profile.video_api_base, "https://video.example/v1");
        assert_eq!(profile.video_proxy_url, "http://127.0.0.1:7891");
        assert_eq!(profile.video_model, "video-model");
        assert_eq!(profile.video_api_mode, "videos");
        assert_eq!(profile.prompt_optimizer_api_key, "prompt-secret");
        assert_eq!(
            profile.prompt_optimizer_api_base,
            "https://prompt.example/v1"
        );
        assert_eq!(profile.prompt_optimizer_model, "prompt-model");
        assert!(profile.prompt_optimizer_vision);
        assert_eq!(config.api_key, profile.api_key);
        assert_eq!(config.generation_api_mode, profile.generation_api_mode);
        assert_eq!(config.video_api_key, profile.video_api_key);
        assert_eq!(config.video_api_base, profile.video_api_base);
        assert_eq!(config.video_proxy_url, profile.video_proxy_url);
        assert_eq!(config.video_model, profile.video_model);
        assert_eq!(config.video_api_mode, profile.video_api_mode);
        assert_eq!(
            config.prompt_optimizer_model,
            profile.prompt_optimizer_model
        );
    }

    #[test]
    fn normalize_api_profiles_uses_active_profile_as_legacy_mirror() {
        let mut config: UserConfig = serde_json::from_str(
            r#"{
                "active_api_profile_id":"work",
                "api_profiles":[
                    {"id":"default","name":"Default","api_key":"old"},
                    {"id":"work","name":"Work","api_key":"new","api_base":"https://api.example/v1","generation_api_mode":"chat-completions","last_model":"model-b","video_api_key":"video-new","video_api_base":"https://video.example/v1","video_proxy_url":"http://127.0.0.1:7892","video_model":"video-b","video_api_mode":"videos","prompt_optimizer_vision":true}
                ]
            }"#,
        )
        .unwrap();

        config.normalize_api_profiles();

        assert_eq!(config.active_api_profile_id, "work");
        assert_eq!(config.api_key, "new");
        assert_eq!(config.api_base, "https://api.example/v1");
        assert_eq!(config.generation_api_mode, "chat_completions");
        assert_eq!(config.last_model, "model-b");
        assert_eq!(config.video_api_key, "video-new");
        assert_eq!(config.video_api_base, "https://video.example/v1");
        assert_eq!(config.video_proxy_url, "http://127.0.0.1:7892");
        assert_eq!(config.video_model, "video-b");
        assert_eq!(config.video_api_mode, "videos");
        assert!(config.prompt_optimizer_vision);
    }

    #[test]
    fn normalize_api_profiles_repairs_duplicate_ids_and_missing_active() {
        let mut config: UserConfig = serde_json::from_str(
            r#"{
                "active_api_profile_id":"missing",
                "api_profiles":[
                    {"id":"same","name":"One"},
                    {"id":"same","name":""}
                ]
            }"#,
        )
        .unwrap();

        config.normalize_api_profiles();

        assert_eq!(config.api_profiles[0].id, "same");
        assert_eq!(config.api_profiles[1].id, "same-2");
        assert_eq!(config.api_profiles[1].name, "API 配置 2");
        assert_eq!(config.active_api_profile_id, "same");
    }
}
