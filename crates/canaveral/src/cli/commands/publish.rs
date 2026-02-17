//! Publish CLI commands
//!
//! Unified publishing interface for app stores and package registries.

use clap::{Args, Subcommand};
use console::style;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

use canaveral_stores::{
    apple::AppStoreConnect,
    google_play::GooglePlayStore,
    microsoft::MicrosoftStore,
    registries::{
        crates_io::CratesIoRegistry,
        npm::{NpmRegistry, TagSupport},
    },
    AppleStoreConfig, CratesIoConfig, GooglePlayConfig, MicrosoftStoreConfig, NpmConfig,
    StoreAdapter, UploadOptions,
};

use crate::cli::{Cli, OutputFormat};

/// Publish to app stores or package registries
#[derive(Debug, Args)]
pub struct PublishCommand {
    #[command(subcommand)]
    pub target: PublishTarget,
}

/// Publishing targets
#[derive(Debug, Subcommand)]
pub enum PublishTarget {
    /// Publish to NPM registry
    Npm(NpmPublishCommand),

    /// Publish to Crates.io registry
    Crates(CratesPublishCommand),

    /// Publish to Apple App Store
    Apple(ApplePublishCommand),

    /// Publish to Google Play Store
    #[command(name = "google-play")]
    GooglePlay(GooglePlayPublishCommand),

    /// Publish to Microsoft Store
    Microsoft(MicrosoftPublishCommand),
}

/// Publish to NPM registry
#[derive(Debug, Args)]
pub struct NpmPublishCommand {
    /// Path to .tgz package file
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// NPM registry URL
    #[arg(long, default_value = "https://registry.npmjs.org")]
    pub registry: String,

    /// NPM token (or use NPM_TOKEN env var)
    #[arg(long, env = "NPM_TOKEN")]
    pub token: Option<String>,

    /// Dist-tag to publish with (default: latest)
    #[arg(long, default_value = "latest")]
    pub tag: String,

    /// Dry run - validate but don't publish
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Publish to Crates.io registry
#[derive(Debug, Args)]
pub struct CratesPublishCommand {
    /// Path to .crate file
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Crates.io token (or use CARGO_REGISTRY_TOKEN env var)
    #[arg(long, env = "CARGO_REGISTRY_TOKEN")]
    pub token: Option<String>,

    /// Registry URL
    #[arg(long, default_value = "https://crates.io")]
    pub registry: String,

    /// Dry run - validate but don't publish
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Publish to Apple App Store (delegates to store apple command)
#[derive(Debug, Args)]
pub struct ApplePublishCommand {
    /// Path to artifact (ipa, app, pkg, dmg)
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// App Store Connect API Key ID
    #[arg(long, env = "APP_STORE_CONNECT_KEY_ID")]
    pub api_key_id: String,

    /// API Key Issuer ID
    #[arg(long, env = "APP_STORE_CONNECT_ISSUER_ID")]
    pub api_issuer_id: String,

    /// Path to .p8 key file or key contents
    #[arg(long, env = "APP_STORE_CONNECT_KEY")]
    pub api_key: String,

    /// Apple Team ID (optional)
    #[arg(long, env = "APPLE_TEAM_ID")]
    pub team_id: Option<String>,

    /// Notarize before upload (macOS only)
    #[arg(long)]
    pub notarize: bool,

    /// Staple notarization ticket
    #[arg(long)]
    pub staple: bool,

    /// Dry run - validate but don't upload
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Publish to Google Play Store
#[derive(Debug, Args)]
pub struct GooglePlayPublishCommand {
    /// Path to artifact (apk or aab)
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Package name
    #[arg(long, required = true)]
    pub package_name: String,

    /// Path to service account JSON key
    #[arg(long, env = "GOOGLE_PLAY_SERVICE_ACCOUNT")]
    pub service_account: PathBuf,

    /// Release track (internal, alpha, beta, production)
    #[arg(long, default_value = "internal")]
    pub track: String,

    /// Staged rollout percentage (0.0-1.0)
    #[arg(long)]
    pub rollout: Option<f64>,

    /// Release notes (format: "en-US:notes,de-DE:notes")
    #[arg(long)]
    pub release_notes: Option<String>,

    /// Dry run - validate but don't upload
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Publish to Microsoft Store
#[derive(Debug, Args)]
pub struct MicrosoftPublishCommand {
    /// Path to artifact (msix, msixbundle, appx, appxbundle)
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Azure AD Tenant ID
    #[arg(long, env = "MS_TENANT_ID")]
    pub tenant_id: String,

    /// Azure AD Client ID
    #[arg(long, env = "MS_CLIENT_ID")]
    pub client_id: String,

    /// Azure AD Client Secret
    #[arg(long, env = "MS_CLIENT_SECRET")]
    pub client_secret: String,

    /// Partner Center Application ID
    #[arg(long, env = "MS_APP_ID")]
    pub app_id: String,

    /// Package flight name (optional)
    #[arg(long)]
    pub flight: Option<String>,

    /// Release notes (format: "en-US:notes,de-DE:notes")
    #[arg(long)]
    pub release_notes: Option<String>,

    /// Dry run - validate but don't upload
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

impl PublishCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let target_name = match &self.target {
            PublishTarget::Npm(_) => "npm",
            PublishTarget::Crates(_) => "crates",
            PublishTarget::Apple(_) => "apple",
            PublishTarget::GooglePlay(_) => "google-play",
            PublishTarget::Microsoft(_) => "microsoft",
        };
        info!(target = target_name, "executing publish command");
        let rt = tokio::runtime::Runtime::new()?;
        match &self.target {
            PublishTarget::Npm(cmd) => rt.block_on(cmd.execute(cli)),
            PublishTarget::Crates(cmd) => rt.block_on(cmd.execute(cli)),
            PublishTarget::Apple(cmd) => rt.block_on(cmd.execute(cli)),
            PublishTarget::GooglePlay(cmd) => rt.block_on(cmd.execute(cli)),
            PublishTarget::Microsoft(cmd) => rt.block_on(cmd.execute(cli)),
        }
    }
}

impl NpmPublishCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = NpmConfig {
            registry_url: self.registry.clone(),
            token: self.token.clone(),
        };

        let registry = NpmRegistry::new(config)?;

        if !cli.quiet {
            println!(
                "{} {} to NPM{}",
                if self.dry_run {
                    style("Validating").yellow()
                } else {
                    style("Publishing").cyan()
                },
                style(self.artifact.display()).bold(),
                if self.tag != "latest" {
                    format!(" (tag: {})", self.tag)
                } else {
                    String::new()
                }
            );
        }

        let options = UploadOptions {
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            ..Default::default()
        };

        let result = registry.upload(&self.artifact, &options).await?;

        // If not dry run and tag is not "latest", add the custom tag
        if !self.dry_run && self.tag != "latest" {
            if !cli.quiet {
                println!("{}", style("Adding custom dist-tag...").dim());
            }

            // Extract package name from validation result
            let validation = registry.validate_artifact(&self.artifact).await?;
            if let Some(app_info) = validation.app_info {
                registry
                    .add_tag(&app_info.identifier, &app_info.version, &self.tag)
                    .await?;
            }
        }

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Publish successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Version:  {}", style(build_id).cyan());
                    }
                    println!("  Tag:      {}", self.tag);
                    if let Some(url) = &result.console_url {
                        println!("  Package:  {}", style(url).dim());
                    }
                } else {
                    println!("{}", style("Publish failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl CratesPublishCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = CratesIoConfig {
            registry_url: self.registry.clone(),
            token: self.token.clone(),
        };

        let registry = CratesIoRegistry::new(config)?;

        if !cli.quiet {
            println!(
                "{} {} to Crates.io",
                if self.dry_run {
                    style("Validating").yellow()
                } else {
                    style("Publishing").cyan()
                },
                style(self.artifact.display()).bold()
            );
        }

        let options = UploadOptions {
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            ..Default::default()
        };

        let result = registry.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Publish successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Crate:    {}", style(build_id).cyan());
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Page:     {}", style(url).dim());
                    }
                    if !result.warnings.is_empty() {
                        println!("  Warnings:");
                        for warning in &result.warnings {
                            println!("    - {}", style(warning).yellow());
                        }
                    }
                } else {
                    println!("{}", style("Publish failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl ApplePublishCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = AppleStoreConfig {
            api_key_id: self.api_key_id.clone(),
            api_issuer_id: self.api_issuer_id.clone(),
            api_key: self.api_key.clone(),
            team_id: self.team_id.clone(),
            app_id: None,
            notarize: self.notarize,
            staple: self.staple,
            primary_locale: None,
        };

        let store = AppStoreConnect::new(config)?;

        if !cli.quiet {
            println!(
                "{} {} to App Store Connect",
                if self.dry_run {
                    style("Validating").yellow()
                } else {
                    style("Publishing").cyan()
                },
                style(self.artifact.display()).bold()
            );
        }

        let options = UploadOptions {
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            ..Default::default()
        };

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Publish successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Build ID: {}", style(build_id).cyan());
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Console:  {}", style(url).dim());
                    }
                    println!("  Status:   {}", result.status);
                } else {
                    println!("{}", style("Publish failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl GooglePlayPublishCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = GooglePlayConfig {
            package_name: self.package_name.clone(),
            service_account_key: self.service_account.clone(),
            default_track: Some(self.track.clone()),
        };

        let store = GooglePlayStore::new(config)?;

        if !cli.quiet {
            println!(
                "{} {} to Google Play ({})",
                if self.dry_run {
                    style("Validating").yellow()
                } else {
                    style("Publishing").cyan()
                },
                style(self.artifact.display()).bold(),
                self.track
            );
        }

        // Parse release notes
        let release_notes = self
            .release_notes
            .as_ref()
            .map(|notes| {
                notes
                    .split(',')
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, ':');
                        match (parts.next(), parts.next()) {
                            (Some(lang), Some(text)) => Some((lang.to_string(), text.to_string())),
                            _ => None,
                        }
                    })
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        let options = UploadOptions {
            track: Some(self.track.clone()),
            rollout_percentage: self.rollout,
            release_notes,
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            ..Default::default()
        };

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Publish successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Version Code: {}", style(build_id).cyan());
                    }
                    println!("  Track:        {}", self.track);
                    if let Some(rollout) = self.rollout {
                        println!("  Rollout:      {}%", (rollout * 100.0) as u32);
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Console:      {}", style(url).dim());
                    }
                } else {
                    println!("{}", style("Publish failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl MicrosoftPublishCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = MicrosoftStoreConfig {
            tenant_id: self.tenant_id.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            app_id: self.app_id.clone(),
            default_flight: self.flight.clone(),
        };

        let store = MicrosoftStore::new(config)?;

        if !cli.quiet {
            println!(
                "{} {} to Microsoft Store",
                if self.dry_run {
                    style("Validating").yellow()
                } else {
                    style("Publishing").cyan()
                },
                style(self.artifact.display()).bold()
            );
            if let Some(flight) = &self.flight {
                println!("  Flight: {}", style(flight).dim());
            }
        }

        // Parse release notes
        let release_notes = self
            .release_notes
            .as_ref()
            .map(|notes| {
                notes
                    .split(',')
                    .filter_map(|pair| {
                        let mut parts = pair.splitn(2, ':');
                        match (parts.next(), parts.next()) {
                            (Some(lang), Some(text)) => Some((lang.to_string(), text.to_string())),
                            _ => None,
                        }
                    })
                    .collect::<HashMap<String, String>>()
            })
            .unwrap_or_default();

        let options = UploadOptions {
            track: self.flight.clone(),
            release_notes,
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            ..Default::default()
        };

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Publish successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Submission ID: {}", style(build_id).cyan());
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Console:       {}", style(url).dim());
                    }
                    println!("  Status:        {}", result.status);
                } else {
                    println!("{}", style("Publish failed").red().bold());
                }
            }
        }

        Ok(())
    }
}
