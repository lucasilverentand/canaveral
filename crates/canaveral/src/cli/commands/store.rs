//! Store upload CLI commands

use clap::{Args, Subcommand};
use console::style;
use std::collections::HashMap;
use std::path::PathBuf;

use canaveral_stores::{
    apple::{AppStoreConnect, Notarizer},
    google_play::GooglePlayStore,
    microsoft::MicrosoftStore,
    AppleStoreConfig, GooglePlayConfig, MicrosoftStoreConfig, StoreAdapter, UploadOptions,
};

use crate::cli::{Cli, OutputFormat};

/// Store upload commands
#[derive(Debug, Args)]
pub struct StoreCommand {
    #[command(subcommand)]
    pub command: StoreSubcommand,
}

/// Store subcommands
#[derive(Debug, Subcommand)]
pub enum StoreSubcommand {
    /// Upload to Apple App Store / macOS
    Apple(AppleCommand),

    /// Upload to Google Play Store
    GooglePlay(GooglePlayCommand),

    /// Upload to Microsoft Store
    Microsoft(MicrosoftCommand),

    /// Notarize a macOS app
    Notarize(NotarizeCommand),

    /// Validate an artifact for a store
    Validate(ValidateCommand),
}

/// Apple App Store commands
#[derive(Debug, Args)]
pub struct AppleCommand {
    #[command(subcommand)]
    pub command: AppleSubcommand,
}

/// Apple subcommands
#[derive(Debug, Subcommand)]
pub enum AppleSubcommand {
    /// Upload an artifact to App Store Connect
    Upload(AppleUploadCommand),
}

/// Upload to Apple App Store
#[derive(Debug, Args)]
pub struct AppleUploadCommand {
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

    // --- Metadata validation options ---

    /// Validate metadata before upload
    #[arg(long)]
    pub validate_metadata: bool,

    /// Sync metadata after successful upload
    #[arg(long)]
    pub sync_metadata: bool,

    /// Path to metadata directory (e.g., ./fastlane/metadata)
    #[arg(long)]
    pub metadata_path: Option<PathBuf>,

    /// Fail upload if metadata validation has errors
    #[arg(long)]
    pub require_valid_metadata: bool,
}

/// Google Play Store commands
#[derive(Debug, Args)]
pub struct GooglePlayCommand {
    #[command(subcommand)]
    pub command: GooglePlaySubcommand,
}

/// Google Play subcommands
#[derive(Debug, Subcommand)]
pub enum GooglePlaySubcommand {
    /// Upload an artifact to Google Play
    Upload(GooglePlayUploadCommand),

    /// Update rollout percentage
    Rollout(RolloutCommand),

    /// Promote a build between tracks
    Promote(PromoteCommand),
}

/// Upload to Google Play
#[derive(Debug, Args)]
pub struct GooglePlayUploadCommand {
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

    // --- Metadata validation options ---

    /// Validate metadata before upload
    #[arg(long)]
    pub validate_metadata: bool,

    /// Sync metadata after successful upload
    #[arg(long)]
    pub sync_metadata: bool,

    /// Path to metadata directory (e.g., ./fastlane/metadata)
    #[arg(long)]
    pub metadata_path: Option<PathBuf>,

    /// Fail upload if metadata validation has errors
    #[arg(long)]
    pub require_valid_metadata: bool,
}

/// Update rollout percentage
#[derive(Debug, Args)]
pub struct RolloutCommand {
    /// Build ID / version code
    #[arg(required = true)]
    pub build_id: String,

    /// New rollout percentage (0.0-1.0)
    #[arg(required = true)]
    pub percentage: f64,

    /// Package name
    #[arg(long, required = true)]
    pub package_name: String,

    /// Path to service account JSON key
    #[arg(long, env = "GOOGLE_PLAY_SERVICE_ACCOUNT")]
    pub service_account: PathBuf,

    /// Track (default: production)
    #[arg(long, default_value = "production")]
    pub track: String,
}

/// Promote a build between tracks
#[derive(Debug, Args)]
pub struct PromoteCommand {
    /// Build ID / version code
    #[arg(required = true)]
    pub build_id: String,

    /// Source track
    #[arg(long, required = true)]
    pub from: String,

    /// Destination track
    #[arg(long, required = true)]
    pub to: String,

    /// Package name
    #[arg(long, required = true)]
    pub package_name: String,

    /// Path to service account JSON key
    #[arg(long, env = "GOOGLE_PLAY_SERVICE_ACCOUNT")]
    pub service_account: PathBuf,
}

/// Microsoft Store commands
#[derive(Debug, Args)]
pub struct MicrosoftCommand {
    #[command(subcommand)]
    pub command: MicrosoftSubcommand,
}

/// Microsoft subcommands
#[derive(Debug, Subcommand)]
pub enum MicrosoftSubcommand {
    /// Upload an artifact to Microsoft Store
    Upload(MicrosoftUploadCommand),

    /// List package flights
    Flights(MicrosoftFlightsCommand),

    /// Get submission status
    Status(MicrosoftStatusCommand),
}

/// Upload to Microsoft Store
#[derive(Debug, Args)]
pub struct MicrosoftUploadCommand {
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

    // --- Metadata validation options ---
    // Note: Microsoft Store metadata validation is not yet supported

    /// Validate metadata before upload (not yet supported for Microsoft Store)
    #[arg(long, hide = true)]
    pub validate_metadata: bool,

    /// Path to metadata directory
    #[arg(long, hide = true)]
    pub metadata_path: Option<PathBuf>,
}

/// List Microsoft Store package flights
#[derive(Debug, Args)]
pub struct MicrosoftFlightsCommand {
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
}

/// Get Microsoft Store submission status
#[derive(Debug, Args)]
pub struct MicrosoftStatusCommand {
    /// Submission ID
    #[arg(required = true)]
    pub submission_id: String,

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
}

/// Notarize a macOS artifact
#[derive(Debug, Args)]
pub struct NotarizeCommand {
    /// Path to artifact
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

    /// Staple notarization ticket after success
    #[arg(long)]
    pub staple: bool,

    /// Wait for notarization to complete
    #[arg(long, default_value = "true")]
    pub wait: bool,

    /// Timeout in seconds
    #[arg(long, default_value = "3600")]
    pub timeout: u64,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Validate an artifact
#[derive(Debug, Args)]
pub struct ValidateCommand {
    /// Path to artifact
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Store type (apple, google-play, microsoft)
    #[arg(long, required = true)]
    pub store: String,

    /// Package name (for Google Play)
    #[arg(long)]
    pub package_name: Option<String>,
}

impl StoreCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        match &self.command {
            StoreSubcommand::Apple(cmd) => rt.block_on(cmd.execute(cli)),
            StoreSubcommand::GooglePlay(cmd) => rt.block_on(cmd.execute(cli)),
            StoreSubcommand::Microsoft(cmd) => rt.block_on(cmd.execute(cli)),
            StoreSubcommand::Notarize(cmd) => rt.block_on(cmd.execute(cli)),
            StoreSubcommand::Validate(cmd) => rt.block_on(cmd.execute(cli)),
        }
    }
}

impl AppleCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            AppleSubcommand::Upload(cmd) => cmd.execute(cli).await,
        }
    }
}

impl AppleUploadCommand {
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
                if self.dry_run { style("Validating").yellow() } else { style("Uploading").cyan() },
                style(self.artifact.display()).bold()
            );
        }

        let options = UploadOptions {
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            validate_metadata: self.validate_metadata,
            sync_metadata: self.sync_metadata,
            metadata_path: self.metadata_path.clone(),
            require_valid_metadata: self.require_valid_metadata,
            ..Default::default()
        };

        // Run metadata validation if enabled
        #[cfg(feature = "metadata-integration")]
        if canaveral_stores::should_validate_metadata(&options) {
            use canaveral_stores::{run_pre_upload_validation, MetadataPlatform};

            if !cli.quiet {
                println!("{}", style("Validating metadata...").dim());
            }

            // Extract bundle ID from artifact for validation
            let app_info = canaveral_stores::apple::extract_app_info(&self.artifact).await?;
            let bundle_id = &app_info.identifier;

            let validation_summary = run_pre_upload_validation(
                MetadataPlatform::Apple,
                bundle_id,
                &options,
                false, // not strict by default
            ).await?;

            if !cli.quiet {
                if validation_summary.valid {
                    println!("{}", style("Metadata validation passed").green());
                } else {
                    println!(
                        "{}: {} error(s), {} warning(s)",
                        style("Metadata validation").yellow(),
                        validation_summary.error_count,
                        validation_summary.warning_count
                    );
                }
            }
        }

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Upload successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Build ID: {}", style(build_id).cyan());
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Console:  {}", style(url).dim());
                    }
                    println!("  Status:   {}", result.status);
                } else {
                    println!("{}", style("Upload failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl GooglePlayCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            GooglePlaySubcommand::Upload(cmd) => cmd.execute(cli).await,
            GooglePlaySubcommand::Rollout(cmd) => cmd.execute(cli).await,
            GooglePlaySubcommand::Promote(cmd) => cmd.execute(cli).await,
        }
    }
}

impl GooglePlayUploadCommand {
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
                if self.dry_run { style("Validating").yellow() } else { style("Uploading").cyan() },
                style(self.artifact.display()).bold(),
                self.track
            );
        }

        // Parse release notes
        let release_notes = self.release_notes.as_ref()
            .map(|notes| {
                notes.split(',')
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
            validate_metadata: self.validate_metadata,
            sync_metadata: self.sync_metadata,
            metadata_path: self.metadata_path.clone(),
            require_valid_metadata: self.require_valid_metadata,
            ..Default::default()
        };

        // Run metadata validation if enabled
        #[cfg(feature = "metadata-integration")]
        if canaveral_stores::should_validate_metadata(&options) {
            use canaveral_stores::{run_pre_upload_validation, MetadataPlatform};

            if !cli.quiet {
                println!("{}", style("Validating metadata...").dim());
            }

            let validation_summary = run_pre_upload_validation(
                MetadataPlatform::GooglePlay,
                &self.package_name,
                &options,
                false, // not strict by default
            ).await?;

            if !cli.quiet {
                if validation_summary.valid {
                    println!("{}", style("Metadata validation passed").green());
                } else {
                    println!(
                        "{}: {} error(s), {} warning(s)",
                        style("Metadata validation").yellow(),
                        validation_summary.error_count,
                        validation_summary.warning_count
                    );
                }
            }
        }

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Upload successful!").green().bold());
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
                    println!("{}", style("Upload failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl RolloutCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_stores::traits::StagedRolloutSupport;

        let config = GooglePlayConfig {
            package_name: self.package_name.clone(),
            service_account_key: self.service_account.clone(),
            default_track: Some(self.track.clone()),
        };

        let store = GooglePlayStore::new(config)?;

        if !cli.quiet {
            println!(
                "Updating rollout for {} to {}%",
                style(&self.build_id).cyan(),
                (self.percentage * 100.0) as u32
            );
        }

        store.update_rollout(&self.build_id, self.percentage).await?;

        if !cli.quiet {
            println!("{}", style("Rollout updated").green().bold());
        }

        Ok(())
    }
}

impl PromoteCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_stores::traits::TrackSupport;

        let config = GooglePlayConfig {
            package_name: self.package_name.clone(),
            service_account_key: self.service_account.clone(),
            default_track: None,
        };

        let store = GooglePlayStore::new(config)?;

        if !cli.quiet {
            println!(
                "Promoting {} from {} to {}",
                style(&self.build_id).cyan(),
                style(&self.from).yellow(),
                style(&self.to).green()
            );
        }

        store.promote_build(&self.build_id, &self.from, &self.to).await?;

        if !cli.quiet {
            println!("{}", style("Build promoted").green().bold());
        }

        Ok(())
    }
}

impl MicrosoftCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            MicrosoftSubcommand::Upload(cmd) => cmd.execute(cli).await,
            MicrosoftSubcommand::Flights(cmd) => cmd.execute(cli).await,
            MicrosoftSubcommand::Status(cmd) => cmd.execute(cli).await,
        }
    }
}

impl MicrosoftUploadCommand {
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
                if self.dry_run { style("Validating").yellow() } else { style("Uploading").cyan() },
                style(self.artifact.display()).bold()
            );
            if let Some(flight) = &self.flight {
                println!("  Flight: {}", style(flight).dim());
            }
        }

        // Parse release notes
        let release_notes = self.release_notes.as_ref()
            .map(|notes| {
                notes.split(',')
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
            validate_metadata: self.validate_metadata,
            metadata_path: self.metadata_path.clone(),
            ..Default::default()
        };

        // Note: Microsoft Store metadata validation is not yet supported
        if self.validate_metadata && !cli.quiet {
            println!(
                "{}",
                style("Warning: Metadata validation is not yet supported for Microsoft Store").yellow()
            );
        }

        let result = store.upload(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if result.success {
                    println!("{}", style("Upload successful!").green().bold());
                    if let Some(build_id) = &result.build_id {
                        println!("  Submission ID: {}", style(build_id).cyan());
                    }
                    if let Some(url) = &result.console_url {
                        println!("  Console:       {}", style(url).dim());
                    }
                    println!("  Status:        {}", result.status);
                } else {
                    println!("{}", style("Upload failed").red().bold());
                }
            }
        }

        Ok(())
    }
}

impl MicrosoftFlightsCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_stores::traits::TrackSupport;

        let config = MicrosoftStoreConfig {
            tenant_id: self.tenant_id.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            app_id: self.app_id.clone(),
            default_flight: None,
        };

        let store = MicrosoftStore::new(config)?;

        if !cli.quiet {
            println!("{}", style("Package Flights").bold());
        }

        let tracks = store.list_tracks().await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&tracks)?);
            }
            OutputFormat::Text => {
                for track in tracks {
                    println!("  - {}", style(&track).cyan());
                }
            }
        }

        Ok(())
    }
}

impl MicrosoftStatusCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = MicrosoftStoreConfig {
            tenant_id: self.tenant_id.clone(),
            client_id: self.client_id.clone(),
            client_secret: self.client_secret.clone(),
            app_id: self.app_id.clone(),
            default_flight: None,
        };

        let store = MicrosoftStore::new(config)?;

        if !cli.quiet {
            println!(
                "{} {}",
                style("Checking status of submission").cyan(),
                style(&self.submission_id).bold()
            );
        }

        let status = store.get_build_status(&self.submission_id).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&status)?);
            }
            OutputFormat::Text => {
                println!();
                println!("  Status: {}", style(status.status.to_string()).cyan());
                if let Some(details) = &status.details {
                    println!("  Details:\n{}", details);
                }
            }
        }

        Ok(())
    }
}

impl NotarizeCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let config = AppleStoreConfig {
            api_key_id: self.api_key_id.clone(),
            api_issuer_id: self.api_issuer_id.clone(),
            api_key: self.api_key.clone(),
            team_id: self.team_id.clone(),
            app_id: None,
            notarize: true,
            staple: self.staple,
            primary_locale: None,
        };

        let notarizer = Notarizer::new(&config)?;

        if !cli.quiet {
            println!(
                "{} {}",
                style("Notarizing").cyan(),
                style(self.artifact.display()).bold()
            );
        }

        let result = if self.wait {
            notarizer.notarize(&self.artifact, Some(self.timeout)).await?
        } else {
            let submission_id = notarizer.submit(&self.artifact).await?;
            notarizer.status(&submission_id).await?
        };

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                match result.status {
                    canaveral_stores::NotarizationStatus::Accepted => {
                        println!("{}", style("Notarization accepted!").green().bold());
                        if self.staple {
                            println!("  Ticket stapled to artifact");
                        }
                    }
                    canaveral_stores::NotarizationStatus::InProgress => {
                        println!("{}", style("Notarization in progress").yellow());
                        println!("  Submission ID: {}", result.submission_id);
                    }
                    _ => {
                        println!(
                            "{}: {:?}",
                            style("Notarization failed").red().bold(),
                            result.status
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

impl ValidateCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_stores::apple::extract_app_info;

        if !cli.quiet {
            println!(
                "{} {}",
                style("Validating").cyan(),
                style(self.artifact.display()).bold()
            );
        }

        let result = match self.store.to_lowercase().as_str() {
            "apple" | "appstore" | "app-store" => {
                extract_app_info(&self.artifact).await?
            }
            "google-play" | "googleplay" | "play" => {
                // For Google Play, we'd need aapt2 installed
                anyhow::bail!("Google Play validation requires aapt2 to be installed");
            }
            "microsoft" | "ms-store" | "msstore" => {
                // Create a dummy config for validation
                let config = MicrosoftStoreConfig {
                    tenant_id: String::new(),
                    client_id: String::new(),
                    client_secret: String::new(),
                    app_id: String::new(),
                    default_flight: None,
                };
                let store = MicrosoftStore::new(config)?;
                let validation = store.validate_artifact(&self.artifact).await?;
                validation.app_info.ok_or_else(|| anyhow::anyhow!("Failed to extract app info"))?
            }
            _ => {
                anyhow::bail!("Unknown store: {}. Use 'apple', 'google-play', or 'microsoft'", self.store);
            }
        };

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                println!("{}", style("App Information").bold());
                println!();
                println!("  Bundle ID:    {}", style(&result.identifier).cyan());
                println!("  Version:      {}", &result.version);
                println!("  Build:        {}", &result.build_number);
                if let Some(name) = &result.name {
                    println!("  Name:         {}", name);
                }
                if let Some(min_os) = &result.min_os_version {
                    println!("  Min OS:       {}", min_os);
                }
                println!("  Platforms:    {}", result.platforms.join(", "));
                println!("  Size:         {} bytes", result.size);
            }
        }

        Ok(())
    }
}
