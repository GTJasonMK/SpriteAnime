use clap::{Args, Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::config::{ApiProfile, AppState};
use crate::runtime::{AppError, AppResult};
use crate::services::config::{redact_profile, require_profile_mut, ConfigService, SecretKind};

use super::CommandResult;

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    Validate,
    Show,
    Import {
        path: PathBuf,
        #[arg(long)]
        yes: bool,
    },
    Export {
        path: PathBuf,
        #[arg(long)]
        include_secrets: bool,
        #[arg(long)]
        yes: bool,
    },
    Profile {
        #[command(subcommand)]
        command: Box<ProfileCommand>,
    },
    Secret {
        #[command(subcommand)]
        command: SecretCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum ProfileCommand {
    List,
    Show {
        id: Option<String>,
    },
    Add {
        id: String,
        name: String,
        #[command(flatten)]
        fields: ProfileFields,
    },
    Clone {
        source_id: String,
        new_id: String,
        #[arg(long)]
        name: Option<String>,
    },
    Update {
        id: String,
        #[command(flatten)]
        fields: ProfileFields,
    },
    Activate {
        id: String,
    },
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum SecretCommand {
    Set {
        kind: SecretType,
        #[arg(long)]
        profile: Option<String>,
    },
    Clear {
        kind: SecretType,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum SecretType {
    Image,
    Video,
    Optimizer,
}

impl From<SecretType> for SecretKind {
    fn from(value: SecretType) -> Self {
        match value {
            SecretType::Image => Self::Image,
            SecretType::Video => Self::Video,
            SecretType::Optimizer => Self::Optimizer,
        }
    }
}

#[derive(Debug, Default, Args)]
pub struct ProfileFields {
    #[arg(long = "name")]
    profile_name: Option<String>,
    #[arg(long)]
    image_base: Option<String>,
    #[arg(long)]
    image_proxy: Option<String>,
    #[arg(long)]
    image_mode: Option<String>,
    #[arg(long)]
    image_model: Option<String>,
    #[arg(long)]
    video_base: Option<String>,
    #[arg(long)]
    video_proxy: Option<String>,
    #[arg(long)]
    video_mode: Option<String>,
    #[arg(long)]
    video_model: Option<String>,
    #[arg(long)]
    optimizer_base: Option<String>,
    #[arg(long)]
    optimizer_mode: Option<String>,
    #[arg(long)]
    optimizer_model: Option<String>,
    #[arg(long)]
    optimizer_vision: Option<bool>,
}

pub fn execute(
    state: &AppState,
    selected_profile: Option<&str>,
    command: ConfigCommand,
) -> AppResult<CommandResult> {
    let service = ConfigService::new(state);
    match command {
        ConfigCommand::Validate => validate(state, &service),
        ConfigCommand::Show => CommandResult::serializable("config.show", service.redacted()?),
        ConfigCommand::Import { path, yes } => {
            require_yes(yes, "导入配置会替换当前配置")?;
            let config = service.import(&path)?;
            CommandResult::serializable(
                "config.import",
                serde_json::json!({
                    "path": path,
                    "activeProfileId": config.active_api_profile_id,
                    "profileCount": config.api_profiles.len(),
                }),
            )
        }
        ConfigCommand::Export {
            path,
            include_secrets,
            yes,
        } => {
            if include_secrets {
                require_yes(yes, "导出完整配置会把 API 密钥写入文件")?;
            }
            let path = service.export(&path, include_secrets)?;
            CommandResult::serializable(
                "config.export",
                serde_json::json!({"path": path, "includesSecrets": include_secrets}),
            )
        }
        ConfigCommand::Profile { command } => execute_profile(&service, selected_profile, *command),
        ConfigCommand::Secret { command } => execute_secret(&service, selected_profile, command),
    }
}

fn validate(state: &AppState, service: &ConfigService<'_>) -> AppResult<CommandResult> {
    let config = service.load()?;
    CommandResult::serializable(
        "config.validate",
        serde_json::json!({
            "valid": true,
            "activeProfileId": config.active_api_profile_id,
            "profileCount": config.api_profiles.len(),
            "dataDir": state.app_data_dir,
        }),
    )
}

fn execute_profile(
    service: &ConfigService<'_>,
    selected_profile: Option<&str>,
    command: ProfileCommand,
) -> AppResult<CommandResult> {
    match command {
        ProfileCommand::List => {
            let config = service.load()?;
            let profiles = config
                .api_profiles
                .into_iter()
                .map(redact_profile)
                .collect::<Vec<_>>();
            CommandResult::serializable(
                "config.profile.list",
                serde_json::json!({
                    "activeProfileId": config.active_api_profile_id,
                    "profiles": profiles,
                }),
            )
        }
        ProfileCommand::Show { id } => {
            let id = id.as_deref().or(selected_profile);
            let profile = service.selected_profile(id)?;
            CommandResult::serializable("config.profile.show", redact_profile(profile))
        }
        ProfileCommand::Add { id, name, fields } => {
            let profile = service.update(|config| {
                if config.api_profiles.iter().any(|profile| profile.id == id) {
                    return Err(AppError::config(format!("API 配置已存在：{id}")));
                }
                let mut profile = ApiProfile {
                    id,
                    name,
                    ..ApiProfile::default()
                };
                fields.apply(&mut profile);
                config.api_profiles.push(profile.clone());
                Ok(profile)
            })?;
            CommandResult::serializable("config.profile.add", redact_profile(profile))
        }
        ProfileCommand::Clone {
            source_id,
            new_id,
            name,
        } => {
            let profile = service.update(|config| {
                if config
                    .api_profiles
                    .iter()
                    .any(|profile| profile.id == new_id)
                {
                    return Err(AppError::config(format!("API 配置已存在：{new_id}")));
                }
                let mut profile = config
                    .api_profiles
                    .iter()
                    .find(|profile| profile.id == source_id)
                    .cloned()
                    .ok_or_else(|| AppError::config(format!("API 配置不存在：{source_id}")))?;
                profile.id = new_id;
                if let Some(name) = name {
                    profile.name = name;
                }
                config.api_profiles.push(profile.clone());
                Ok(profile)
            })?;
            CommandResult::serializable("config.profile.clone", redact_profile(profile))
        }
        ProfileCommand::Update { id, fields } => {
            let profile = service.update(|config| {
                let profile = require_profile_mut(config, &id)?;
                fields.apply(profile);
                Ok(profile.clone())
            })?;
            CommandResult::serializable("config.profile.update", redact_profile(profile))
        }
        ProfileCommand::Activate { id } => {
            service.update(|config| {
                if !config.api_profiles.iter().any(|profile| profile.id == id) {
                    return Err(AppError::config(format!("API 配置不存在：{id}")));
                }
                config.active_api_profile_id = id.clone();
                Ok(())
            })?;
            CommandResult::serializable(
                "config.profile.activate",
                serde_json::json!({"activeProfileId": id}),
            )
        }
        ProfileCommand::Delete { id, yes } => {
            require_yes(yes, "删除 API 配置不可撤销")?;
            service.update(|config| {
                if config.active_api_profile_id == id {
                    return Err(AppError::validation("不能删除当前活动 API 配置"));
                }
                let before = config.api_profiles.len();
                config.api_profiles.retain(|profile| profile.id != id);
                if config.api_profiles.len() == before {
                    return Err(AppError::config(format!("API 配置不存在：{id}")));
                }
                Ok(())
            })?;
            CommandResult::serializable(
                "config.profile.delete",
                serde_json::json!({"deletedProfileId": id}),
            )
        }
    }
}

fn execute_secret(
    service: &ConfigService<'_>,
    selected_profile: Option<&str>,
    command: SecretCommand,
) -> AppResult<CommandResult> {
    match command {
        SecretCommand::Set { kind, profile } => {
            let id = resolve_profile_id(service, profile.as_deref().or(selected_profile))?;
            let secret_kind = SecretKind::from(kind);
            service.set_secret_from_env(&id, secret_kind)?;
            CommandResult::serializable(
                "config.secret.set",
                serde_json::json!({"profileId": id, "environmentVariable": secret_kind.env_name()}),
            )
        }
        SecretCommand::Clear { kind, profile, yes } => {
            require_yes(yes, "清除持久化 API 密钥不可撤销")?;
            let id = resolve_profile_id(service, profile.as_deref().or(selected_profile))?;
            service.clear_secret(&id, kind.into())?;
            CommandResult::serializable("config.secret.clear", serde_json::json!({"profileId": id}))
        }
    }
}

fn resolve_profile_id(service: &ConfigService<'_>, requested: Option<&str>) -> AppResult<String> {
    let config = service.load()?;
    Ok(requested
        .map(str::to_string)
        .unwrap_or(config.active_api_profile_id))
}

fn require_yes(yes: bool, message: &str) -> AppResult<()> {
    if yes {
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "{message}；请添加 --yes 明确确认"
        )))
    }
}

impl ProfileFields {
    fn apply(self, profile: &mut ApiProfile) {
        assign(&mut profile.name, self.profile_name);
        assign(&mut profile.api_base, self.image_base);
        assign(&mut profile.proxy_url, self.image_proxy);
        assign(&mut profile.generation_api_mode, self.image_mode);
        assign(&mut profile.last_model, self.image_model);
        assign(&mut profile.video_api_base, self.video_base);
        assign(&mut profile.video_proxy_url, self.video_proxy);
        assign(&mut profile.video_api_mode, self.video_mode);
        assign(&mut profile.video_model, self.video_model);
        assign(&mut profile.prompt_optimizer_api_base, self.optimizer_base);
        assign(&mut profile.prompt_optimizer_api_mode, self.optimizer_mode);
        assign(&mut profile.prompt_optimizer_model, self.optimizer_model);
        if let Some(value) = self.optimizer_vision {
            profile.prompt_optimizer_vision = value;
        }
    }
}

fn assign(target: &mut String, value: Option<String>) {
    if let Some(value) = value {
        *target = value;
    }
}
