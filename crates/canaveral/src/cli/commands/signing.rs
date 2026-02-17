//! Signing command

use clap::{Args, Subcommand};
use console::style;
use std::path::PathBuf;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_signing::{
    providers::{create_provider, ProviderType},
    SignOptions, VerifyOptions,
};

use crate::cli::{Cli, OutputFormat};

use super::signing_team::TeamCommand;

/// Code signing commands
#[derive(Debug, Args)]
pub struct SigningCommand {
    #[command(subcommand)]
    pub command: SigningSubcommand,
}

/// Signing subcommands
#[derive(Debug, Subcommand)]
pub enum SigningSubcommand {
    /// List available signing identities
    List(ListCommand),

    /// Sign an artifact
    Sign(SignCommand),

    /// Verify a signature
    Verify(VerifyCommand),

    /// Show signing identity details
    Info(InfoCommand),

    /// Team vault management
    Team(TeamCommand),
}

/// List available signing identities
#[derive(Debug, Args)]
pub struct ListCommand {
    /// Signing provider (macos, windows, android, gpg)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Show only valid (non-expired) identities
    #[arg(long)]
    pub valid_only: bool,
}

/// Sign an artifact
#[derive(Debug, Args)]
pub struct SignCommand {
    /// Path to artifact to sign
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Signing identity (name, fingerprint, or key ID)
    #[arg(short, long)]
    pub identity: Option<String>,

    /// Signing provider (macos, windows, android, gpg)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Path to entitlements file (macOS)
    #[arg(long)]
    pub entitlements: Option<String>,

    /// Enable hardened runtime (macOS)
    #[arg(long)]
    pub hardened_runtime: bool,

    /// Timestamp the signature
    #[arg(long, default_value = "true")]
    pub timestamp: bool,

    /// Force re-signing
    #[arg(short, long)]
    pub force: bool,

    /// Deep signing (macOS)
    #[arg(long)]
    pub deep: bool,

    /// Dry run - don't actually sign
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// Create detached signature (GPG)
    #[arg(long)]
    pub detached: bool,

    /// ASCII armor output (GPG)
    #[arg(long)]
    pub armor: bool,

    /// Keystore path (Android)
    #[arg(long)]
    pub keystore: Option<PathBuf>,

    /// Key alias (Android)
    #[arg(long)]
    pub key_alias: Option<String>,
}

/// Verify a signature
#[derive(Debug, Args)]
pub struct VerifyCommand {
    /// Path to artifact to verify
    #[arg(required = true)]
    pub artifact: PathBuf,

    /// Signing provider (macos, windows, android, gpg)
    #[arg(short, long)]
    pub provider: Option<String>,

    /// Deep verification (macOS)
    #[arg(long)]
    pub deep: bool,

    /// Strict verification
    #[arg(long)]
    pub strict: bool,

    /// Check notarization status (macOS)
    #[arg(long)]
    pub check_notarization: bool,

    /// Verbose output
    #[arg(short, long)]
    pub verbose: bool,
}

/// Show signing identity details
#[derive(Debug, Args)]
pub struct InfoCommand {
    /// Identity to look up (name, fingerprint, or key ID)
    #[arg(required = true)]
    pub identity: String,

    /// Signing provider (macos, windows, android, gpg)
    #[arg(short, long)]
    pub provider: Option<String>,
}

impl SigningCommand {
    /// Execute the signing command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.command {
            SigningSubcommand::List(_) => "list",
            SigningSubcommand::Sign(_) => "sign",
            SigningSubcommand::Verify(_) => "verify",
            SigningSubcommand::Info(_) => "info",
            SigningSubcommand::Team(_) => "team",
        };
        info!(subcommand = subcommand_name, "executing signing command");
        // Create tokio runtime for async operations
        let rt = tokio::runtime::Runtime::new()?;

        match &self.command {
            SigningSubcommand::List(cmd) => rt.block_on(cmd.execute(cli)),
            SigningSubcommand::Sign(cmd) => rt.block_on(cmd.execute(cli)),
            SigningSubcommand::Verify(cmd) => rt.block_on(cmd.execute(cli)),
            SigningSubcommand::Info(cmd) => rt.block_on(cmd.execute(cli)),
            SigningSubcommand::Team(cmd) => cmd.execute(cli),
        }
    }
}

impl ListCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Determine provider
        let provider_type = self
            .provider
            .as_ref()
            .or(config.signing.provider.as_ref())
            .map(|p| p.parse::<ProviderType>())
            .transpose()?
            .unwrap_or({
                #[cfg(target_os = "macos")]
                {
                    ProviderType::MacOS
                }
                #[cfg(target_os = "windows")]
                {
                    ProviderType::Windows
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    ProviderType::Gpg
                }
            });

        let provider = create_provider(provider_type)?;

        if !provider.is_available() {
            anyhow::bail!(
                "Signing provider '{}' is not available on this system",
                provider.name()
            );
        }

        let identities = provider.list_identities().await?;

        let identities: Vec<_> = if self.valid_only {
            identities
                .into_iter()
                .filter(|id| id.is_valid && !id.is_expired())
                .collect()
        } else {
            identities
        };

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "provider": provider.name(),
                    "identities": identities
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!(
                    "{} ({})",
                    style("Signing Identities").bold(),
                    provider.name()
                );
                println!();

                if identities.is_empty() {
                    println!("  {}", style("No signing identities found").dim());
                } else {
                    for id in &identities {
                        let status = if id.is_expired() {
                            style("EXPIRED").red()
                        } else if !id.is_valid {
                            style("INVALID").red()
                        } else if id.expires_within_days(30) {
                            style("EXPIRING SOON").yellow()
                        } else {
                            style("VALID").green()
                        };

                        println!("  {} [{}]", style(&id.name).cyan(), status);

                        if let Some(fp) = &id.fingerprint {
                            let short_fp = if fp.len() > 16 { &fp[..16] } else { fp };
                            println!("    Fingerprint: {}...", style(short_fp).dim());
                        }

                        if let Some(team) = &id.team_id {
                            println!("    Team ID:     {}", team);
                        }

                        if let Some(exp) = id.expires_at {
                            println!("    Expires:     {}", exp.format("%Y-%m-%d"));
                        }

                        println!();
                    }
                }
            }
        }

        Ok(())
    }
}

impl SignCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Determine provider
        let provider_type = self
            .provider
            .as_ref()
            .or(config.signing.provider.as_ref())
            .map(|p| p.parse::<ProviderType>())
            .transpose()?
            .unwrap_or_else(|| {
                // Auto-detect based on file extension
                if let Some(ext) = self.artifact.extension().and_then(|e| e.to_str()) {
                    match ext.to_lowercase().as_str() {
                        "app" | "framework" | "dylib" | "pkg" | "dmg" => ProviderType::MacOS,
                        "exe" | "dll" | "msi" | "msix" => ProviderType::Windows,
                        "apk" | "aab" => ProviderType::Android,
                        _ => ProviderType::Gpg,
                    }
                } else {
                    #[cfg(target_os = "macos")]
                    {
                        ProviderType::MacOS
                    }
                    #[cfg(target_os = "windows")]
                    {
                        ProviderType::Windows
                    }
                    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                    {
                        ProviderType::Gpg
                    }
                }
            });

        let provider = create_provider(provider_type)?;

        if !provider.is_available() {
            anyhow::bail!(
                "Signing provider '{}' is not available on this system",
                provider.name()
            );
        }

        // Get identity
        let identity_query = self
            .identity
            .as_ref()
            .or(config.signing.identity.as_ref())
            .ok_or_else(|| anyhow::anyhow!("No signing identity specified"))?;

        let mut identity = provider.find_identity(identity_query).await?;

        // For Android, set keystore and alias from CLI args
        if provider_type == ProviderType::Android {
            if let Some(ks) = &self.keystore {
                identity.keychain = Some(ks.to_string_lossy().to_string());
            }
            if let Some(alias) = &self.key_alias {
                identity.key_alias = Some(alias.clone());
            }
        }

        // Build sign options
        let options = SignOptions {
            entitlements: self.entitlements.clone().or_else(|| {
                config
                    .signing
                    .macos
                    .entitlements
                    .as_ref()
                    .map(|p| p.to_string_lossy().to_string())
            }),
            hardened_runtime: self.hardened_runtime || config.signing.macos.hardened_runtime,
            timestamp: self.timestamp,
            force: self.force,
            deep: self.deep || config.signing.macos.deep,
            dry_run: self.dry_run,
            verbose: self.verbose || cli.verbose,
            detached: self.detached || config.signing.gpg.detached,
            armor: self.armor || config.signing.gpg.armor,
            keystore_password: std::env::var(
                config
                    .signing
                    .android
                    .keystore_password_env
                    .as_deref()
                    .unwrap_or("ANDROID_KEYSTORE_PASSWORD"),
            )
            .ok(),
            key_password: std::env::var(
                config
                    .signing
                    .android
                    .key_password_env
                    .as_deref()
                    .unwrap_or("ANDROID_KEY_PASSWORD"),
            )
            .ok(),
            passphrase: std::env::var(
                config
                    .signing
                    .gpg
                    .passphrase_env
                    .as_deref()
                    .unwrap_or("GPG_PASSPHRASE"),
            )
            .ok(),
            ..Default::default()
        };

        if !cli.quiet {
            println!(
                "{} {} with {}",
                if self.dry_run {
                    style("Would sign").yellow()
                } else {
                    style("Signing").cyan()
                },
                style(self.artifact.display()).bold(),
                style(&identity.name).green()
            );
        }

        provider.sign(&self.artifact, &identity, &options).await?;

        if !cli.quiet && !self.dry_run {
            println!("{}", style("✓ Signed successfully").green().bold());
        }

        Ok(())
    }
}

impl VerifyCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Determine provider
        let provider_type = self
            .provider
            .as_ref()
            .or(config.signing.provider.as_ref())
            .map(|p| p.parse::<ProviderType>())
            .transpose()?
            .unwrap_or_else(|| {
                // Auto-detect based on file extension
                if let Some(ext) = self.artifact.extension().and_then(|e| e.to_str()) {
                    match ext.to_lowercase().as_str() {
                        "app" | "framework" | "dylib" | "pkg" | "dmg" => ProviderType::MacOS,
                        "exe" | "dll" | "msi" | "msix" => ProviderType::Windows,
                        "apk" | "aab" => ProviderType::Android,
                        _ => ProviderType::Gpg,
                    }
                } else {
                    ProviderType::Gpg
                }
            });

        let provider = create_provider(provider_type)?;

        let options = VerifyOptions {
            deep: self.deep,
            strict: self.strict,
            verbose: self.verbose || cli.verbose,
            check_notarization: self.check_notarization,
        };

        let info = provider.verify(&self.artifact, &options).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&info)?);
            }
            OutputFormat::Text => {
                println!(
                    "{} {}",
                    style("Verifying").cyan(),
                    style(self.artifact.display()).bold()
                );
                println!();

                let status_style = match info.status {
                    canaveral_signing::SignatureStatus::Valid => style("VALID").green().bold(),
                    canaveral_signing::SignatureStatus::Invalid => style("INVALID").red().bold(),
                    canaveral_signing::SignatureStatus::Expired => style("EXPIRED").yellow().bold(),
                    canaveral_signing::SignatureStatus::Revoked => style("REVOKED").red().bold(),
                    canaveral_signing::SignatureStatus::NotSigned => style("NOT SIGNED").dim(),
                    canaveral_signing::SignatureStatus::Unknown => style("UNKNOWN").yellow(),
                };

                println!("  Status: {}", status_style);

                if let Some(signer) = &info.signer {
                    println!("  Signer: {}", style(&signer.common_name).cyan());
                    if let Some(team) = &signer.team_id {
                        println!("  Team:   {}", team);
                    }
                }

                if let Some(signed_at) = info.signed_at {
                    println!("  Signed: {}", signed_at.format("%Y-%m-%d %H:%M:%S UTC"));
                }

                if let Some(notarized) = info.notarized {
                    let notary_status = if notarized {
                        style("Yes").green()
                    } else {
                        style("No").yellow()
                    };
                    println!("  Notarized: {}", notary_status);
                }

                if !info.warnings.is_empty() {
                    println!();
                    println!("  {}", style("Warnings:").yellow());
                    for warning in &info.warnings {
                        println!("    - {}", warning);
                    }
                }

                if self.verbose {
                    if let Some(details) = &info.details {
                        println!();
                        println!("  {}", style("Details:").dim());
                        for line in details.lines().take(20) {
                            println!("    {}", line);
                        }
                    }
                }
            }
        }

        // Return error if not valid
        if info.status != canaveral_signing::SignatureStatus::Valid {
            anyhow::bail!("Signature verification failed: {}", info.status);
        }

        Ok(())
    }
}

impl InfoCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Determine provider
        let provider_type = self
            .provider
            .as_ref()
            .or(config.signing.provider.as_ref())
            .map(|p| p.parse::<ProviderType>())
            .transpose()?
            .unwrap_or({
                #[cfg(target_os = "macos")]
                {
                    ProviderType::MacOS
                }
                #[cfg(target_os = "windows")]
                {
                    ProviderType::Windows
                }
                #[cfg(not(any(target_os = "macos", target_os = "windows")))]
                {
                    ProviderType::Gpg
                }
            });

        let provider = create_provider(provider_type)?;
        let identity = provider.find_identity(&self.identity).await?;

        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&identity)?);
            }
            OutputFormat::Text => {
                println!("{}", style("Signing Identity").bold());
                println!();
                println!("  Name:       {}", style(&identity.name).cyan());
                println!("  Type:       {}", identity.identity_type);
                println!("  ID:         {}", &identity.id);

                if let Some(fp) = &identity.fingerprint {
                    println!("  Fingerprint: {}", fp);
                }

                if let Some(team) = &identity.team_id {
                    println!("  Team ID:    {}", team);
                }

                if let Some(subject) = &identity.subject {
                    println!("  Subject:    {}", subject);
                }

                if let Some(issuer) = &identity.issuer {
                    println!("  Issuer:     {}", issuer);
                }

                if let Some(serial) = &identity.serial_number {
                    println!("  Serial:     {}", serial);
                }

                if let Some(created) = identity.created_at {
                    println!("  Created:    {}", created.format("%Y-%m-%d"));
                }

                if let Some(expires) = identity.expires_at {
                    let days_left = identity.days_until_expiration().unwrap_or(0);
                    let exp_style = if days_left < 0 {
                        style(format!("{} (EXPIRED)", expires.format("%Y-%m-%d"))).red()
                    } else if days_left < 30 {
                        style(format!(
                            "{} ({} days left)",
                            expires.format("%Y-%m-%d"),
                            days_left
                        ))
                        .yellow()
                    } else {
                        style(format!(
                            "{} ({} days left)",
                            expires.format("%Y-%m-%d"),
                            days_left
                        ))
                        .green()
                    };
                    println!("  Expires:    {}", exp_style);
                }

                let valid_style = if identity.is_valid && !identity.is_expired() {
                    style("Yes").green()
                } else {
                    style("No").red()
                };
                println!("  Valid:      {}", valid_style);

                if let Some(keychain) = &identity.keychain {
                    println!("  Keychain:   {}", keychain);
                }

                if let Some(alias) = &identity.key_alias {
                    println!("  Key Alias:  {}", alias);
                }
            }
        }

        Ok(())
    }
}
