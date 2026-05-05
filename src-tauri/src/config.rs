use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::PathBuf;

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

/// 用户持久化配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_base: String,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default = "default_last_model")]
    pub last_model: String,
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
        Self {
            api_key: String::new(),
            api_base: String::new(),
            proxy_url: String::new(),
            last_model: "gpt-5.3-codex".into(),
            last_ratio: "1:1".into(),
            last_resolution: "原始".into(),
            last_style: "anime".into(),
            last_count: 1,
            prompt_optimizer_api_key: String::new(),
            prompt_optimizer_api_base: default_prompt_optimizer_api_base(),
            prompt_optimizer_model: default_prompt_optimizer_model(),
            prompt_optimizer_vision: false,
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
            Ok(content) => match serde_json::from_str(&content) {
                Ok(config) => {
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
        assert_eq!(config.last_model, "gpt-5.3-codex");
        assert_eq!(config.last_ratio, "1:1");
        assert_eq!(config.last_resolution, "原始");
        assert_eq!(config.last_style, "anime");
        assert_eq!(config.last_count, 1);
        assert_eq!(config.prompt_optimizer_api_base, "https://api.deepseek.com");
        assert_eq!(config.prompt_optimizer_model, "deepseek-v4-flash");
        assert!(!config.prompt_optimizer_vision);
        assert_eq!(config.ffmpeg_path, "");
        assert_eq!(config.ffprobe_path, "");
    }
}
