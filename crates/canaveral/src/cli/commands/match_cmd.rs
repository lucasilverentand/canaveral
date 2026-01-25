//! Match command - Certificate and profile synchronization

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use console::style;

use canaveral_signing::sync::{MatchConfig, MatchSync, ProfileType, SyncStorage};
use canaveral_signing::team::generate_keypair;

use crate::cli::{Cli, OutputFormat};

/// Certificate and profile synchronization (match-style)
#[derive(Debug, Args)]
pub struct MatchCommand {
    #[command(subcommand)]
    pub command: MatchSubcommand,
}

/// Match subcommands
#[derive(Debug, Subcommand)]
pub enum MatchSubcommand {
    /// Initialize a new match repository
    Init(InitCommand),

    /// Sync certificates and profiles
    Sync(SyncCommand),

    /// Remove certificates and profiles
    Nuke(NukeCommand),

    /// Show match status
    Status(StatusCommand),
}

/// Initialize match repository
#[derive(Debug, Args)]
pub struct InitCommand {
    /// Storage type
    #[arg(short, long, default_value = "git")]
    pub storage: StorageType,

    /// Git repository URL
    #[arg(long, required_if_eq("storage", "git"))]
    pub git_url: Option<String>,

    /// Git branch
    #[arg(long, default_value = "main")]
    pub branch: String,

    /// S3 bucket name
    #[arg(long, required_if_eq("storage", "s3"))]
    pub bucket: Option<String>,

    /// S3 prefix
    #[arg(long, default_value = "match")]
    pub prefix: String,

    /// AWS region
    #[arg(long, default_value = "us-east-1")]
    pub region: String,

    /// Team ID
    #[arg(short, long)]
    pub team_id: String,

    /// Output directory for keys
    #[arg(short, long, default_value = ".canaveral/match")]
    pub output: PathBuf,
}

/// Sync certificates and profiles
#[derive(Debug, Args)]
pub struct SyncCommand {
    /// Profile type to sync
    #[arg(short, long, default_value = "development")]
    pub profile_type: ProfileTypeArg,

    /// Read-only mode (don't modify storage)
    #[arg(long)]
    pub readonly: bool,

    /// Force re-download even if cached
    #[arg(long)]
    pub force: bool,

    /// App IDs to sync (comma-separated)
    #[arg(long)]
    pub app_ids: Option<String>,

    /// Private key file
    #[arg(long, default_value = ".canaveral/match/match.key")]
    pub keyfile: PathBuf,

    /// Storage configuration file
    #[arg(short, long, default_value = ".canaveral/match/config.yaml")]
    pub config: PathBuf,
}

/// Nuke (remove) certificates and profiles
#[derive(Debug, Args)]
pub struct NukeCommand {
    /// Profile type to remove (or all if not specified)
    #[arg(short, long)]
    pub profile_type: Option<ProfileTypeArg>,

    /// Private key file
    #[arg(long, default_value = ".canaveral/match/match.key")]
    pub keyfile: PathBuf,

    /// Storage configuration file
    #[arg(short, long, default_value = ".canaveral/match/config.yaml")]
    pub config: PathBuf,

    /// Skip confirmation prompt
    #[arg(long)]
    pub yes: bool,
}

/// Show status
#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Private key file
    #[arg(long, default_value = ".canaveral/match/match.key")]
    pub keyfile: PathBuf,

    /// Storage configuration file
    #[arg(short, long, default_value = ".canaveral/match/config.yaml")]
    pub config: PathBuf,
}

/// Storage type argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum StorageType {
    /// Git repository
    Git,
    /// AWS S3
    S3,
    /// Google Cloud Storage
    Gcs,
    /// Azure Blob Storage
    Azure,
}

/// Profile type argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ProfileTypeArg {
    Development,
    Adhoc,
    Appstore,
    Enterprise,
}

impl From<ProfileTypeArg> for ProfileType {
    fn from(p: ProfileTypeArg) -> Self {
        match p {
            ProfileTypeArg::Development => Self::Development,
            ProfileTypeArg::Adhoc => Self::AdHoc,
            ProfileTypeArg::Appstore => Self::AppStore,
            ProfileTypeArg::Enterprise => Self::Enterprise,
        }
    }
}

impl MatchCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            MatchSubcommand::Init(cmd) => cmd.execute(cli).await,
            MatchSubcommand::Sync(cmd) => cmd.execute(cli).await,
            MatchSubcommand::Nuke(cmd) => cmd.execute(cli).await,
            MatchSubcommand::Status(cmd) => cmd.execute(cli).await,
        }
    }
}

impl InitCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Create storage configuration
        let storage = match self.storage {
            StorageType::Git => {
                let url = self.git_url.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Git URL required for git storage"))?;
                SyncStorage::Git {
                    url: url.clone(),
                    branch: self.branch.clone(),
                }
            }
            StorageType::S3 => {
                let bucket = self.bucket.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Bucket required for S3 storage"))?;
                SyncStorage::S3 {
                    bucket: bucket.clone(),
                    prefix: self.prefix.clone(),
                    region: self.region.clone(),
                }
            }
            StorageType::Gcs => {
                let bucket = self.bucket.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Bucket required for GCS storage"))?;
                SyncStorage::GoogleCloudStorage {
                    bucket: bucket.clone(),
                    prefix: self.prefix.clone(),
                }
            }
            StorageType::Azure => {
                let container = self.bucket.as_ref()
                    .ok_or_else(|| anyhow::anyhow!("Container required for Azure storage"))?;
                SyncStorage::AzureBlob {
                    container: container.clone(),
                    prefix: self.prefix.clone(),
                }
            }
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Initializing match repository...").bold());
            println!("  Team ID: {}", style(&self.team_id).cyan());
            println!("  Storage: {:?}", style(&self.storage).cyan());
            println!();
        }

        // Generate encryption keypair
        let keypair = generate_keypair();

        // Create output directory
        std::fs::create_dir_all(&self.output)?;

        // Save keypair
        let keyfile = self.output.join("match.key");
        std::fs::write(&keyfile, &keypair.private_key)?;

        let pubfile = self.output.join("match.pub");
        std::fs::write(&pubfile, &keypair.public_key)?;

        // Save configuration
        let config = MatchConfig {
            storage: storage.clone(),
            team_id: self.team_id.clone(),
            encryption_key: Some(keypair.public_key.clone()),
            ..Default::default()
        };

        let config_file = self.output.join("config.yaml");
        let config_yaml = serde_yaml::to_string(&config)?;
        std::fs::write(&config_file, &config_yaml)?;

        // Initialize storage
        let sync = MatchSync::new(config)?
            .with_keypair(keypair);
        sync.init().await?;

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!("{} Match repository initialized", style("✓").green());
            println!();
            println!("Important files:");
            println!(
                "  {} - Keep this secret, needed to decrypt",
                style(keyfile.display()).cyan()
            );
            println!(
                "  {} - Share with team members",
                style(pubfile.display()).cyan()
            );
            println!(
                "  {} - Storage configuration",
                style(config_file.display()).cyan()
            );
            println!();
            println!("Next steps:");
            println!("  1. Add {} to .gitignore", style("match.key").cyan());
            println!(
                "  2. Run {} to sync certificates",
                style("canaveral match sync").cyan()
            );
        }

        Ok(())
    }
}

impl SyncCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load configuration
        let config_content = std::fs::read_to_string(&self.config)
            .map_err(|_| anyhow::anyhow!("Config not found. Run 'canaveral match init' first."))?;
        let mut config: MatchConfig = serde_yaml::from_str(&config_content)?;

        // Apply command-line options
        config.readonly = self.readonly;
        config.force = self.force;
        config.profile_types = vec![self.profile_type.into()];

        if let Some(ref app_ids) = self.app_ids {
            config.app_ids = app_ids.split(',').map(|s| s.trim().to_string()).collect();
        }

        // Load keypair
        let private_key = std::fs::read_to_string(&self.keyfile)
            .map_err(|_| anyhow::anyhow!("Key file not found: {}", self.keyfile.display()))?;

        let public_key = config.encryption_key.clone()
            .ok_or_else(|| anyhow::anyhow!("No encryption key in config"))?;

        let keypair = canaveral_signing::team::KeyPair {
            public_key,
            private_key,
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Syncing certificates and profiles...").bold());
            println!("  Profile type: {}", style(format!("{:?}", self.profile_type)).cyan());
            if self.readonly {
                println!("  Mode: {}", style("read-only").yellow());
            }
            println!();
        }

        // Run sync
        let sync = MatchSync::new(config)?
            .with_keypair(keypair);
        let manifest = sync.sync().await?;

        // Output results
        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        } else if !cli.quiet {
            let cert_count: usize = manifest.certificates.values().map(|v| v.len()).sum();
            let profile_count: usize = manifest.profiles.values().map(|v| v.len()).sum();

            println!("{} Sync complete", style("✓").green());
            println!(
                "  {} certificates, {} profiles",
                style(cert_count).cyan(),
                style(profile_count).cyan()
            );

            // Show certificate info
            for (cert_type, certs) in &manifest.certificates {
                for cert in certs {
                    println!(
                        "  {} {} ({}) - expires {}",
                        style("•").dim(),
                        style(&cert.name).cyan(),
                        cert_type,
                        style(&cert.expires).yellow()
                    );
                }
            }

            // Show profile info
            for (app_id, profiles) in &manifest.profiles {
                for (profile_type, profile) in profiles {
                    println!(
                        "  {} {} ({}) - {} - expires {}",
                        style("•").dim(),
                        style(&profile.name).cyan(),
                        profile_type,
                        app_id,
                        style(&profile.expires).yellow()
                    );
                }
            }
        }

        Ok(())
    }
}

impl NukeCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Confirmation prompt
        if !self.yes {
            eprintln!(
                "{} This will remove {} from the match repository.",
                style("WARNING:").red().bold(),
                if self.profile_type.is_some() {
                    format!("{:?} profiles", self.profile_type.unwrap())
                } else {
                    "ALL certificates and profiles".to_string()
                }
            );
            eprintln!("This action cannot be undone.");
            eprintln!();
            eprint!("Type 'yes' to confirm: ");

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;

            if input.trim() != "yes" {
                eprintln!("Aborted.");
                return Ok(());
            }
        }

        // Load configuration
        let config_content = std::fs::read_to_string(&self.config)?;
        let config: MatchConfig = serde_yaml::from_str(&config_content)?;

        // Load keypair
        let private_key = std::fs::read_to_string(&self.keyfile)?;
        let public_key = config.encryption_key.clone()
            .ok_or_else(|| anyhow::anyhow!("No encryption key in config"))?;

        let keypair = canaveral_signing::team::KeyPair {
            public_key,
            private_key,
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Removing certificates and profiles...").bold());
        }

        // Run nuke
        let sync = MatchSync::new(config)?
            .with_keypair(keypair);
        sync.nuke(self.profile_type.map(|p| p.into())).await?;

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!("{} Certificates and profiles removed", style("✓").green());
            println!();
            println!(
                "Run {} to regenerate",
                style("canaveral match sync").cyan()
            );
        }

        Ok(())
    }
}

impl StatusCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load configuration
        let config_content = std::fs::read_to_string(&self.config)
            .map_err(|_| anyhow::anyhow!("Config not found. Run 'canaveral match init' first."))?;
        let mut config: MatchConfig = serde_yaml::from_str(&config_content)?;
        config.readonly = true;

        // Load keypair
        let private_key = std::fs::read_to_string(&self.keyfile)
            .map_err(|_| anyhow::anyhow!("Key file not found: {}", self.keyfile.display()))?;

        let public_key = config.encryption_key.clone()
            .ok_or_else(|| anyhow::anyhow!("No encryption key in config"))?;

        let keypair = canaveral_signing::team::KeyPair {
            public_key,
            private_key,
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Match Status").bold());
            println!();
            println!("Team ID: {}", style(&config.team_id).cyan());
            println!("Storage: {:?}", config.storage);
            println!();
        }

        // Read manifest
        let sync = MatchSync::new(config)?
            .with_keypair(keypair);
        let manifest = sync.sync().await?;

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&manifest)?);
        } else if !cli.quiet {
            println!("{}", style("Certificates:").bold().underlined());
            for (cert_type, certs) in &manifest.certificates {
                for cert in certs {
                    let expired = chrono::DateTime::parse_from_rfc3339(&cert.expires)
                        .map(|d| d < chrono::Utc::now())
                        .unwrap_or(false);

                    let status = if expired {
                        style("EXPIRED").red()
                    } else {
                        style("valid").green()
                    };

                    println!(
                        "  {} ({}) - {} - {}",
                        style(&cert.name).cyan(),
                        cert_type,
                        &cert.expires,
                        status
                    );
                }
            }

            if manifest.certificates.is_empty() {
                println!("  {}", style("No certificates").dim());
            }

            println!();
            println!("{}", style("Profiles:").bold().underlined());
            for (app_id, profiles) in &manifest.profiles {
                for (profile_type, profile) in profiles {
                    let expired = chrono::DateTime::parse_from_rfc3339(&profile.expires)
                        .map(|d| d < chrono::Utc::now())
                        .unwrap_or(false);

                    let status = if expired {
                        style("EXPIRED").red()
                    } else {
                        style("valid").green()
                    };

                    println!(
                        "  {} ({}) - {} - {} - {}",
                        style(&profile.name).cyan(),
                        profile_type,
                        app_id,
                        &profile.expires,
                        status
                    );
                }
            }

            if manifest.profiles.is_empty() {
                println!("  {}", style("No profiles").dim());
            }

            println!();
            println!("Last sync: {}", style(&manifest.last_sync).dim());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_type_conversion() {
        let dev: ProfileType = ProfileTypeArg::Development.into();
        assert!(matches!(dev, ProfileType::Development));
    }
}
