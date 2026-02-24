//! metadata sync (pull/push) subcommand

use clap::{Args, Subcommand};
use console::style;
use std::path::PathBuf;

use canaveral_core::config::load_config_or_default;
use canaveral_metadata::sync::{
    AppleMetadataSync, AppleSyncConfig, GooglePlayMetadataSync, GooglePlaySyncConfig, MetadataSync,
};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{parse_locales, SinglePlatform};

/// Apple App Store Connect authentication options
#[derive(Args, Debug, Clone)]
pub struct AppleAuthOptions {
    /// App Store Connect API Key ID
    #[arg(long, env = "APP_STORE_CONNECT_KEY_ID")]
    pub api_key_id: Option<String>,

    /// App Store Connect API Issuer ID
    #[arg(long, env = "APP_STORE_CONNECT_ISSUER_ID")]
    pub api_issuer_id: Option<String>,

    /// Path to App Store Connect API private key (.p8 file)
    #[arg(long, env = "APP_STORE_CONNECT_KEY_PATH")]
    pub api_key_path: Option<PathBuf>,
}

impl AppleAuthOptions {
    pub fn to_config(&self) -> anyhow::Result<AppleSyncConfig> {
        let api_key_id = self
            .api_key_id
            .clone()
            .or_else(|| std::env::var("APP_STORE_CONNECT_KEY_ID").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Key ID. Provide --api-key-id or set APP_STORE_CONNECT_KEY_ID"
                )
            })?;

        let api_issuer_id = self
            .api_issuer_id
            .clone()
            .or_else(|| std::env::var("APP_STORE_CONNECT_ISSUER_ID").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Issuer ID. Provide --api-issuer-id or set APP_STORE_CONNECT_ISSUER_ID"
                )
            })?;

        let key_path = self
            .api_key_path
            .clone()
            .or_else(|| {
                std::env::var("APP_STORE_CONNECT_KEY_PATH")
                    .ok()
                    .map(PathBuf::from)
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Key Path. Provide --api-key-path or set APP_STORE_CONNECT_KEY_PATH"
                )
            })?;

        let api_private_key = std::fs::read_to_string(&key_path).map_err(|e| {
            anyhow::anyhow!(
                "Failed to read API key file '{}': {}",
                key_path.display(),
                e
            )
        })?;

        Ok(AppleSyncConfig {
            api_key_id,
            api_issuer_id,
            api_private_key,
            team_id: None,
        })
    }
}

/// Google Play Console authentication options
#[derive(Args, Debug, Clone)]
pub struct GooglePlayAuthOptions {
    /// Path to Google Play service account JSON key
    #[arg(long, env = "GOOGLE_PLAY_SERVICE_ACCOUNT_KEY")]
    pub service_account_key: Option<PathBuf>,
}

impl GooglePlayAuthOptions {
    pub fn to_config(&self) -> anyhow::Result<GooglePlaySyncConfig> {
        let key_path = self
            .service_account_key
            .clone()
            .or_else(|| {
                std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT_KEY")
                    .ok()
                    .map(PathBuf::from)
            })
            .or_else(|| {
                std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
                    .ok()
                    .map(PathBuf::from)
            });

        if let Some(path) = key_path {
            Ok(GooglePlaySyncConfig::from_key_file(path))
        } else {
            Err(anyhow::anyhow!(
                "Missing service account key. Provide --service-account-key or set GOOGLE_PLAY_SERVICE_ACCOUNT_KEY"
            ))
        }
    }
}

/// Sync commands container
#[derive(Debug, Args)]
pub struct SyncCommand {
    #[command(subcommand)]
    pub command: SyncSubcommand,
}

/// Sync subcommands
#[derive(Debug, Subcommand)]
pub enum SyncSubcommand {
    /// Download metadata from app store
    Pull(SyncPullArgs),
    /// Upload metadata to app store
    Push(SyncPushArgs),
}

/// Pull metadata from app store
#[derive(Debug, Args)]
pub struct SyncPullArgs {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locales to pull (comma-separated, or "all")
    #[arg(long, default_value = "all")]
    pub locales: String,

    /// Also download screenshots
    #[arg(long)]
    pub include_assets: bool,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apple authentication options
    #[command(flatten)]
    pub apple_auth: AppleAuthOptions,

    /// Google Play authentication options
    #[command(flatten)]
    pub google_auth: GooglePlayAuthOptions,
}

/// Push metadata to app store
#[derive(Debug, Args)]
pub struct SyncPushArgs {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locales to push (comma-separated, or "all")
    #[arg(long, default_value = "all")]
    pub locales: String,

    /// Also upload screenshots
    #[arg(long)]
    pub include_assets: bool,

    /// Only update text, skip screenshots
    #[arg(long)]
    pub skip_screenshots: bool,

    /// Preview changes without actually pushing
    #[arg(long)]
    pub dry_run: bool,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apple authentication options
    #[command(flatten)]
    pub apple_auth: AppleAuthOptions,

    /// Google Play authentication options
    #[command(flatten)]
    pub google_auth: GooglePlayAuthOptions,
}

pub async fn execute(cmd: &SyncCommand, cli: &Cli) -> anyhow::Result<()> {
    match &cmd.command {
        SyncSubcommand::Pull(args) => execute_pull(args, cli).await,
        SyncSubcommand::Push(args) => execute_push(args, cli).await,
    }
}

async fn execute_pull(cmd: &SyncPullArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let cwd = std::env::current_dir()?;
    let (config, _) = load_config_or_default(&cwd);

    let metadata_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| config.metadata.storage.path.clone());

    let locales = parse_locales(&cmd.locales)?;

    let store_name = match cmd.platform {
        SinglePlatform::Apple => "App Store Connect",
        SinglePlatform::GooglePlay => "Google Play Console",
    };

    ui.step(&format!("Pulling metadata from {}", store_name));
    ui.key_value("App ID", &cmd.app_id);
    ui.key_value("Path", &metadata_path.display().to_string());
    if let Some(ref locs) = locales {
        ui.key_value(
            "Locales",
            &locs.iter().map(|l| l.code()).collect::<Vec<_>>().join(", "),
        );
    } else {
        ui.key_value("Locales", "all");
    }

    match cmd.platform {
        SinglePlatform::Apple => {
            let config = cmd.apple_auth.to_config()?;
            let sync = AppleMetadataSync::new(config, metadata_path).await?;
            sync.pull(&cmd.app_id, locales.as_deref()).await?;
        }
        SinglePlatform::GooglePlay => {
            let config = cmd.google_auth.to_config()?;
            let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
            sync.pull(&cmd.app_id, locales.as_deref()).await?;
        }
    }

    ui.json(&serde_json::json!({
        "success": true,
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "operation": "pull",
    }))?;

    ui.blank();
    ui.success("Metadata pulled successfully!");

    Ok(())
}

async fn execute_push(cmd: &SyncPushArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let cwd = std::env::current_dir()?;
    let (config, _) = load_config_or_default(&cwd);

    let metadata_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| config.metadata.storage.path.clone());

    let locales = parse_locales(&cmd.locales)?;

    let store_name = match cmd.platform {
        SinglePlatform::Apple => "App Store Connect",
        SinglePlatform::GooglePlay => "Google Play Console",
    };

    let dry_run_suffix = if cmd.dry_run { " (dry run)" } else { "" };
    ui.step(&format!(
        "Pushing metadata to {}{}",
        store_name, dry_run_suffix
    ));
    ui.key_value("App ID", &cmd.app_id);
    ui.key_value("Path", &metadata_path.display().to_string());
    if let Some(ref locs) = locales {
        ui.key_value(
            "Locales",
            &locs.iter().map(|l| l.code()).collect::<Vec<_>>().join(", "),
        );
    } else {
        ui.key_value("Locales", "all");
    }

    let result = match cmd.platform {
        SinglePlatform::Apple => {
            let config = cmd.apple_auth.to_config()?;
            let sync = AppleMetadataSync::new(config, metadata_path).await?;
            sync.push(&cmd.app_id, locales.as_deref(), cmd.dry_run)
                .await?
        }
        SinglePlatform::GooglePlay => {
            let config = cmd.google_auth.to_config()?;
            let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
            sync.push(&cmd.app_id, locales.as_deref(), cmd.dry_run)
                .await?
        }
    };

    ui.json(&serde_json::json!({
        "success": true,
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "operation": "push",
        "dry_run": cmd.dry_run,
        "updated_locales": result.updated_locales,
        "updated_fields": result.updated_fields,
        "screenshots_uploaded": result.screenshots_uploaded,
        "screenshots_removed": result.screenshots_removed,
        "warnings": result.warnings,
    }))?;

    if ui.is_text() {
        println!();
        if result.has_changes() {
            if cmd.dry_run {
                println!("{} {}", style("Would push:").yellow().bold(), result);
            } else {
                println!("{} {}", style("Pushed:").green().bold(), result);
            }

            if !result.updated_locales.is_empty() {
                println!();
                println!("  Updated locales:");
                for locale in &result.updated_locales {
                    println!("    {} {}", style("-").dim(), locale);
                }
            }

            if !result.warnings.is_empty() {
                println!();
                println!("  {}:", style("Warnings").yellow());
                for warning in &result.warnings {
                    println!("    {} {}", style("!").yellow(), warning);
                }
            }
        } else {
            println!("{}", style("No changes to push.").dim());
        }
    }

    Ok(())
}
