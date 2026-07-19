mod config;
mod local;
mod output;
mod redraw;
mod remote;
mod sprite;
mod video;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use std::path::PathBuf;

use crate::runtime::{create_app_state, AppResult};
use output::CliOutput;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Human,
    Json,
}

#[derive(Debug, Parser)]
#[command(
    name = "sprite-anime-cli",
    version,
    about = "SpriteAnime command-line interface"
)]
struct Cli {
    #[arg(long, global = true, env = "SPRITE_ANIME_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[arg(long, global = true)]
    profile: Option<String>,
    #[arg(long, global = true, value_enum, default_value = "human")]
    format: OutputFormat,
    #[arg(long, global = true)]
    json: bool,
    #[arg(long, global = true)]
    quiet: bool,
    #[arg(long, global = true)]
    no_record: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Config {
        #[command(subcommand)]
        command: config::ConfigCommand,
    },
    Presets {
        #[command(subcommand)]
        command: PresetsCommand,
    },
    History {
        #[command(subcommand)]
        command: local::HistoryCommand,
    },
    Workbench {
        #[command(subcommand)]
        command: local::WorkbenchCommand,
    },
    Assets {
        #[command(subcommand)]
        command: local::AssetsCommand,
    },
    Tools {
        #[command(subcommand)]
        command: local::ToolsCommand,
    },
    Workspace {
        #[command(subcommand)]
        command: local::WorkspaceCommand,
    },
    Logs {
        #[command(subcommand)]
        command: local::LogsCommand,
    },
    Api {
        #[command(subcommand)]
        command: remote::ApiCommand,
    },
    Prompt {
        #[command(subcommand)]
        command: remote::PromptCommand,
    },
    Image {
        #[command(subcommand)]
        command: remote::ImageCommand,
    },
    Video {
        #[command(subcommand)]
        command: video::VideoCommand,
    },
    Redraw {
        #[command(subcommand)]
        command: redraw::RedrawCommand,
    },
    Sprite {
        #[command(subcommand)]
        command: sprite::SpriteCommand,
    },
}

pub struct CommandResult {
    name: &'static str,
    data: serde_json::Value,
}

#[derive(Debug, Subcommand)]
enum PresetsCommand {
    List,
}

impl CommandResult {
    pub fn serializable<T: Serialize>(name: &'static str, value: T) -> AppResult<Self> {
        let data = serde_json::to_value(value).map_err(|error| {
            crate::runtime::AppError::internal(format!("序列化命令结果失败: {error}"))
        })?;
        Ok(Self { name, data })
    }
}

pub async fn run() -> i32 {
    let cli = Cli::parse();
    let command_name = cli.command.name();
    let format = if cli.json {
        OutputFormat::Json
    } else {
        cli.format
    };
    let output = CliOutput::new(format, cli.quiet);
    match execute(cli).await {
        Ok(result) => match output.success(result.name, result.data) {
            Ok(()) => 0,
            Err(error) => {
                if let Err(output_error) = output.error(result.name, &error) {
                    eprintln!("错误: {error}; 输出错误信息失败: {output_error}");
                }
                error.exit_code()
            }
        },
        Err(error) => {
            if let Err(output_error) = output.error(command_name, &error) {
                eprintln!("错误: {error}; 输出错误信息失败: {output_error}");
            }
            error.exit_code()
        }
    }
}

impl Command {
    fn name(&self) -> &'static str {
        match self {
            Self::Config { .. } => "config",
            Self::Presets { .. } => "presets.list",
            Self::History { .. } => "history",
            Self::Workbench { .. } => "workbench",
            Self::Assets { .. } => "assets",
            Self::Tools { .. } => "tools",
            Self::Workspace { .. } => "workspace",
            Self::Logs { .. } => "logs",
            Self::Api { .. } => "api",
            Self::Prompt { .. } => "prompt",
            Self::Image { .. } => "image",
            Self::Video { .. } => "video",
            Self::Redraw { .. } => "redraw",
            Self::Sprite { .. } => "sprite",
        }
    }
}

async fn execute(cli: Cli) -> AppResult<CommandResult> {
    let state = create_app_state(cli.data_dir)?;
    match cli.command {
        Command::Config { command } => config::execute(&state, cli.profile.as_deref(), command),
        Command::Presets {
            command: PresetsCommand::List,
        } => CommandResult::serializable("presets.list", crate::config::get_presets()),
        Command::History { command } => local::execute_history(&state, command),
        Command::Workbench { command } => local::execute_workbench(&state, command),
        Command::Assets { command } => local::execute_assets(&state, command),
        Command::Tools { command } => local::execute_tools(&state, command).await,
        Command::Workspace { command } => local::execute_workspace(&state, command),
        Command::Logs { command } => local::execute_logs(&state, command),
        Command::Api { command } => {
            remote::execute_api(&state, cli.profile.as_deref(), command).await
        }
        Command::Prompt { command } => {
            remote::execute_prompt(&state, cli.profile.as_deref(), command).await
        }
        Command::Image { command } => {
            remote::execute_image(
                &state,
                cli.profile.as_deref(),
                command,
                cli.no_record,
                cli.quiet,
            )
            .await
        }
        Command::Video { command } => {
            video::execute(&state, cli.profile.as_deref(), command, cli.quiet).await
        }
        Command::Redraw { command } => {
            redraw::execute(&state, cli.profile.as_deref(), command, cli.quiet).await
        }
        Command::Sprite { command } => sprite::execute(&state, command),
    }
}
