use serde::Serialize;

/// 风格选项
#[derive(Debug, Clone, Serialize)]
pub struct StyleOption {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing)]
    pub prompt_suffix: String,
}

/// 宽高比映射
#[derive(Debug, Clone, Serialize)]
pub struct RatioOption {
    pub key: String,
}

/// 预设数据，一次性返回给前端
#[derive(Debug, Clone, Serialize)]
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
            RatioOption { key: "1:1".into() },
            RatioOption { key: "16:9".into() },
            RatioOption { key: "9:16".into() },
            RatioOption { key: "4:3".into() },
            RatioOption { key: "3:4".into() },
            RatioOption { key: "3:2".into() },
            RatioOption { key: "2:3".into() },
            RatioOption { key: "21:9".into() },
        ],
        resolutions: vec!["原始".into(), "1K".into(), "2K".into()],
    }
}
