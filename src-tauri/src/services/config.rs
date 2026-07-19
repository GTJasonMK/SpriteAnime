use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::config::{ApiProfile, AppState, UserConfig};
use crate::runtime::{AppError, AppResult, DataLock, LockDomain};

pub const IMAGE_API_KEY_ENV: &str = "SPRITE_ANIME_IMAGE_API_KEY";
pub const VIDEO_API_KEY_ENV: &str = "SPRITE_ANIME_VIDEO_API_KEY";
pub const OPTIMIZER_API_KEY_ENV: &str = "SPRITE_ANIME_OPTIMIZER_API_KEY";

#[derive(Debug, Clone, Copy)]
pub enum SecretKind {
    Image,
    Video,
    Optimizer,
}

impl SecretKind {
    pub fn env_name(self) -> &'static str {
        match self {
            Self::Image => IMAGE_API_KEY_ENV,
            Self::Video => VIDEO_API_KEY_ENV,
            Self::Optimizer => OPTIMIZER_API_KEY_ENV,
        }
    }

    fn assign(self, profile: &mut ApiProfile, value: String) {
        match self {
            Self::Image => profile.api_key = value,
            Self::Video => profile.video_api_key = value,
            Self::Optimizer => profile.prompt_optimizer_api_key = value,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedApiProfile {
    pub id: String,
    pub name: String,
    pub api_key_configured: bool,
    pub api_base: String,
    pub proxy_url: String,
    pub generation_api_mode: String,
    pub last_model: String,
    pub video_api_key_configured: bool,
    pub video_api_base: String,
    pub video_proxy_url: String,
    pub video_model: String,
    pub video_api_mode: String,
    pub prompt_optimizer_api_key_configured: bool,
    pub prompt_optimizer_api_base: String,
    pub prompt_optimizer_api_mode: String,
    pub prompt_optimizer_model: String,
    pub prompt_optimizer_vision: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedactedConfig {
    pub api_profiles: Vec<RedactedApiProfile>,
    pub active_api_profile_id: String,
    pub last_ratio: String,
    pub last_resolution: String,
    pub last_style: String,
    pub last_count: u32,
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub prompt_history: std::collections::VecDeque<String>,
}

pub struct ConfigService<'a> {
    state: &'a AppState,
}

impl<'a> ConfigService<'a> {
    pub fn new(state: &'a AppState) -> Self {
        Self { state }
    }

    pub fn load(&self) -> AppResult<UserConfig> {
        let _lock = DataLock::shared(&self.state.locks_dir, LockDomain::Config)?;
        let config = self.load_disk()?;
        *self.state.config.lock() = config.clone();
        Ok(config)
    }

    pub fn redacted(&self) -> AppResult<RedactedConfig> {
        Ok(redact_config(self.load()?))
    }

    pub fn replace(&self, mut config: UserConfig) -> AppResult<UserConfig> {
        let _lock = DataLock::exclusive(&self.state.locks_dir, LockDomain::Config)?;
        config.normalize_api_profiles().map_err(AppError::config)?;
        self.persist(&config)?;
        *self.state.config.lock() = config.clone();
        Ok(config)
    }

    pub fn import(&self, source: &Path) -> AppResult<UserConfig> {
        let content = std::fs::read_to_string(source).map_err(|error| {
            AppError::filesystem(format!("读取导入配置失败：{} ({error})", source.display()))
        })?;
        let config = serde_json::from_str::<UserConfig>(&content)
            .map_err(|error| AppError::config(format!("解析导入配置失败: {error}")))?;
        self.replace(config)
    }

    pub fn export(&self, target: &Path, include_secrets: bool) -> AppResult<PathBuf> {
        let config = self.load()?;
        self.export_value(config, target, include_secrets)
    }

    pub fn export_value(
        &self,
        mut config: UserConfig,
        target: &Path,
        include_secrets: bool,
    ) -> AppResult<PathBuf> {
        config.normalize_api_profiles().map_err(AppError::config)?;
        let target = ensure_json_extension(target.to_path_buf());
        if include_secrets {
            write_private_json(&target, &config)?;
        } else {
            write_json(&target, &redact_for_export(config))?;
        }
        Ok(target)
    }

    pub fn selected_profile(&self, requested_id: Option<&str>) -> AppResult<ApiProfile> {
        let config = self.load()?;
        let id = requested_id.unwrap_or(&config.active_api_profile_id);
        let mut profile = config
            .api_profiles
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| AppError::config(format!("API 配置不存在：{id}")))?;
        apply_secret_env_overrides(&mut profile);
        Ok(profile)
    }

    pub fn update<F, T>(&self, operation: F) -> AppResult<T>
    where
        F: FnOnce(&mut UserConfig) -> AppResult<T>,
    {
        let _lock = DataLock::exclusive(&self.state.locks_dir, LockDomain::Config)?;
        let mut config = self.load_disk()?;
        let result = operation(&mut config)?;
        config.normalize_api_profiles().map_err(AppError::config)?;
        self.persist(&config)?;
        *self.state.config.lock() = config;
        Ok(result)
    }

    pub fn set_secret_from_env(&self, profile_id: &str, kind: SecretKind) -> AppResult<()> {
        let env_name = kind.env_name();
        let value = std::env::var(env_name)
            .map_err(|_| AppError::validation(format!("环境变量 {env_name} 未设置")))?;
        let value = value.trim().to_string();
        if value.is_empty() {
            return Err(AppError::validation(format!("环境变量 {env_name} 为空")));
        }
        self.update(|config| {
            let profile = require_profile_mut(config, profile_id)?;
            kind.assign(profile, value);
            Ok(())
        })
    }

    pub fn clear_secret(&self, profile_id: &str, kind: SecretKind) -> AppResult<()> {
        self.update(|config| {
            let profile = require_profile_mut(config, profile_id)?;
            kind.assign(profile, String::new());
            Ok(())
        })
    }

    fn load_disk(&self) -> AppResult<UserConfig> {
        UserConfig::load(&self.state.config_path).map_err(AppError::config)
    }

    fn persist(&self, config: &UserConfig) -> AppResult<()> {
        write_json(&self.state.config_path, config)
    }
}

pub fn require_profile_mut<'a>(
    config: &'a mut UserConfig,
    profile_id: &str,
) -> AppResult<&'a mut ApiProfile> {
    config
        .api_profiles
        .iter_mut()
        .find(|profile| profile.id == profile_id)
        .ok_or_else(|| AppError::config(format!("API 配置不存在：{profile_id}")))
}

pub fn redact_profile(profile: ApiProfile) -> RedactedApiProfile {
    RedactedApiProfile {
        id: profile.id,
        name: profile.name,
        api_key_configured: !profile.api_key.is_empty(),
        api_base: profile.api_base,
        proxy_url: profile.proxy_url,
        generation_api_mode: profile.generation_api_mode,
        last_model: profile.last_model,
        video_api_key_configured: !profile.video_api_key.is_empty(),
        video_api_base: profile.video_api_base,
        video_proxy_url: profile.video_proxy_url,
        video_model: profile.video_model,
        video_api_mode: profile.video_api_mode,
        prompt_optimizer_api_key_configured: !profile.prompt_optimizer_api_key.is_empty(),
        prompt_optimizer_api_base: profile.prompt_optimizer_api_base,
        prompt_optimizer_api_mode: profile.prompt_optimizer_api_mode,
        prompt_optimizer_model: profile.prompt_optimizer_model,
        prompt_optimizer_vision: profile.prompt_optimizer_vision,
    }
}

fn redact_config(config: UserConfig) -> RedactedConfig {
    RedactedConfig {
        api_profiles: config
            .api_profiles
            .into_iter()
            .map(redact_profile)
            .collect(),
        active_api_profile_id: config.active_api_profile_id,
        last_ratio: config.last_ratio,
        last_resolution: config.last_resolution,
        last_style: config.last_style,
        last_count: config.last_count,
        ffmpeg_path: config.ffmpeg_path,
        ffprobe_path: config.ffprobe_path,
        prompt_history: config.prompt_history,
    }
}

fn redact_for_export(mut config: UserConfig) -> UserConfig {
    for profile in &mut config.api_profiles {
        profile.api_key.clear();
        profile.video_api_key.clear();
        profile.prompt_optimizer_api_key.clear();
    }
    config
}

fn apply_secret_env_overrides(profile: &mut ApiProfile) {
    for (kind, name) in [
        (SecretKind::Image, IMAGE_API_KEY_ENV),
        (SecretKind::Video, VIDEO_API_KEY_ENV),
        (SecretKind::Optimizer, OPTIMIZER_API_KEY_ENV),
    ] {
        if let Ok(value) = std::env::var(name) {
            if !value.trim().is_empty() {
                kind.assign(profile, value.trim().to_string());
            }
        }
    }
}

fn ensure_json_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
    {
        path.set_extension("json");
    }
    path
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::config(format!("序列化配置失败: {error}")))?;
    write_bytes(path, &bytes, false)
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| AppError::config(format!("序列化配置失败: {error}")))?;
    write_bytes(path, &bytes, true)
}

fn write_bytes(path: &Path, bytes: &[u8], private: bool) -> AppResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| AppError::filesystem("配置输出路径缺少父目录"))?;
    std::fs::create_dir_all(parent).map_err(|error| {
        AppError::filesystem(format!(
            "创建配置输出目录失败：{} ({error})",
            parent.display()
        ))
    })?;
    let mut options = OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    if private {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| {
        AppError::filesystem(format!(
            "打开配置输出文件失败：{} ({error})",
            path.display()
        ))
    })?;
    file.write_all(bytes).map_err(|error| {
        AppError::filesystem(format!(
            "写入配置输出文件失败：{} ({error})",
            path.display()
        ))
    })?;
    if private {
        set_private_permissions(path)?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_private_permissions(path: &Path) -> AppResult<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).map_err(|error| {
        AppError::filesystem(format!(
            "设置私密配置权限失败：{} ({error})",
            path.display()
        ))
    })
}

#[cfg(windows)]
fn set_private_permissions(path: &Path) -> AppResult<()> {
    let username = std::env::var("USERNAME")
        .map_err(|_| AppError::filesystem("无法读取 Windows 当前用户名，不能设置私密配置权限"))?;
    let grant = format!("{username}:F");
    let output = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r", &grant])
        .output()
        .map_err(|error| AppError::filesystem(format!("运行 icacls 失败: {error}")))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(AppError::filesystem(format!(
            "设置私密配置 ACL 失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

#[cfg(not(any(unix, windows)))]
fn set_private_permissions(_path: &Path) -> AppResult<()> {
    Err(AppError::filesystem("当前平台不支持设置私密配置文件权限"))
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn export_value_does_not_replace_active_configuration() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "sprite_anime_config_export_{}_{}",
            std::process::id(),
            stamp
        ));
        let state = crate::runtime::create_app_state(Some(root.clone())).unwrap();
        let service = ConfigService::new(&state);
        let active = service.load().unwrap();
        let mut exported = active.clone();
        exported.last_style = "export-only".into();

        let path = service
            .export_value(exported, &root.join("export"), false)
            .unwrap();

        assert_eq!(path.extension().unwrap(), "json");
        assert_eq!(service.load().unwrap().last_style, active.last_style);
        let written: UserConfig =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(written.last_style, "export-only");
        std::fs::remove_dir_all(root).unwrap();
    }
}
