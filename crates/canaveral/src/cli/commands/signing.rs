//! Signing command

use clap::{Args, Subcommand};
use console::style;
use std::path::PathBuf;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_signing::{
    profiles::ProfileManager,
    providers::{create_provider, ProviderType},
    sync::ProfileType,
    SignOptions, VerifyOptions,
};

use crate::cli::output::Ui;
use crate::cli::Cli;

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

    /// Provisioning profile management
    Profiles(ProfilesCommand),

    /// Generate a new Android keystore
    GenerateKeystore(GenerateKeystoreCommand),
}

/// Provisioning profile subcommands
#[derive(Debug, Args)]
pub struct ProfilesCommand {
    #[command(subcommand)]
    pub command: ProfilesSubcommand,
}

/// Profile management subcommands
#[derive(Debug, Subcommand)]
pub enum ProfilesSubcommand {
    /// List installed provisioning profiles
    List(ProfilesListCommand),

    /// Install a provisioning profile
    Install(ProfilesInstallCommand),

    /// Remove expired provisioning profiles
    Cleanup(ProfilesCleanupCommand),

    /// Sync profiles from match repository
    Match(ProfilesMatchCommand),
}

/// List installed profiles
#[derive(Debug, Args)]
pub struct ProfilesListCommand {
    /// Show only valid (non-expired) profiles
    #[arg(long)]
    pub valid_only: bool,

    /// Filter by bundle ID
    #[arg(long)]
    pub bundle_id: Option<String>,

    /// Filter by profile type (development, adhoc, appstore, enterprise)
    #[arg(long)]
    pub profile_type: Option<String>,
}

/// Install a provisioning profile
#[derive(Debug, Args)]
pub struct ProfilesInstallCommand {
    /// Path to the .mobileprovision file
    #[arg(required = true)]
    pub path: PathBuf,
}

/// Remove expired profiles
#[derive(Debug, Args)]
pub struct ProfilesCleanupCommand;

/// Sync profiles from match repo
#[derive(Debug, Args)]
pub struct ProfilesMatchCommand {
    /// Private key file
    #[arg(long, default_value = ".canaveral/match/match.key")]
    pub keyfile: PathBuf,

    /// Storage configuration file
    #[arg(short, long, default_value = ".canaveral/match/config.toml")]
    pub config: PathBuf,

    /// App IDs to sync (comma-separated)
    #[arg(long)]
    pub app_ids: Option<String>,
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

    /// Path to Android keystore for listing keys
    #[arg(long)]
    pub keystore: Option<PathBuf>,
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

    /// Enable V1 (JAR) signing scheme (Android)
    #[arg(long)]
    pub v1_signing: Option<bool>,

    /// Enable V2 (APK Signature Scheme v2) (Android)
    #[arg(long)]
    pub v2_signing: Option<bool>,

    /// Enable V3 (APK Signature Scheme v3) (Android)
    #[arg(long)]
    pub v3_signing: Option<bool>,

    /// Enable V4 (APK Signature Scheme v4) (Android)
    #[arg(long)]
    pub v4_signing: Option<bool>,
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

/// Generate a new Android keystore
#[derive(Debug, Args)]
pub struct GenerateKeystoreCommand {
    /// Output path for the keystore file
    #[arg(required = true)]
    pub output: PathBuf,

    /// Key alias name
    #[arg(long, default_value = "release")]
    pub alias: String,

    /// Keystore and key password (will prompt if not provided)
    #[arg(long)]
    pub password: Option<String>,

    /// Certificate validity in days
    #[arg(long, default_value = "10950")]
    pub validity: u32,

    /// Common Name (CN)
    #[arg(long)]
    pub cn: Option<String>,

    /// Organization (O)
    #[arg(long)]
    pub org: Option<String>,

    /// Country code (C)
    #[arg(long)]
    pub country: Option<String>,

    /// Key algorithm (RSA, EC)
    #[arg(long, default_value = "RSA")]
    pub key_algorithm: String,

    /// Key size in bits
    #[arg(long, default_value = "2048")]
    pub key_size: u32,
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
            SigningSubcommand::Profiles(_) => "profiles",
            SigningSubcommand::GenerateKeystore(_) => "generate-keystore",
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
            SigningSubcommand::Profiles(cmd) => cmd.execute(cli),
            SigningSubcommand::GenerateKeystore(cmd) => rt.block_on(cmd.execute(cli)),
        }
    }
}

impl ListCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
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

        // For Android with explicit keystore, list keys in that keystore
        if provider_type == ProviderType::Android {
            if let Some(ref keystore_path) = self.keystore {
                let android_provider = canaveral_signing::AndroidProvider::new();
                let password = std::env::var(
                    config
                        .signing
                        .android
                        .keystore_password_env
                        .as_deref()
                        .unwrap_or("ANDROID_KEYSTORE_PASSWORD"),
                )
                .unwrap_or_default();
                let identities = android_provider
                    .list_keystore_keys(&keystore_path.to_string_lossy(), &password)
                    .await?;

                if ui.is_json() {
                    let output = serde_json::json!({
                        "provider": provider.name(),
                        "keystore": keystore_path,
                        "identities": identities
                    });
                    ui.json(&output)?;
                } else if ui.is_text() {
                    ui.header(&format!("Keystore Keys ({})", keystore_path.display()));
                    ui.blank();

                    if identities.is_empty() {
                        ui.hint("No keys found in keystore");
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
                                ui.key_value(
                                    "    Fingerprint",
                                    &format!("{}...", style(short_fp).dim()),
                                );
                            }

                            if let Some(alias) = &id.key_alias {
                                ui.key_value("    Alias", alias);
                            }

                            if let Some(exp) = id.expires_at {
                                ui.key_value("    Expires", &exp.format("%Y-%m-%d").to_string());
                            }

                            ui.blank();
                        }
                    }
                }

                return Ok(());
            }
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

        if ui.is_json() {
            let output = serde_json::json!({
                "provider": provider.name(),
                "identities": identities
            });
            ui.json(&output)?;
        } else if ui.is_text() {
            ui.header(&format!("Signing Identities ({})", provider.name()));
            ui.blank();

            if identities.is_empty() {
                ui.hint("No signing identities found");
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
                        ui.key_value("    Fingerprint", &format!("{}...", style(short_fp).dim()));
                    }

                    if let Some(team) = &id.team_id {
                        ui.key_value("    Team ID", team);
                    }

                    if let Some(exp) = id.expires_at {
                        ui.key_value("    Expires", &exp.format("%Y-%m-%d").to_string());
                    }

                    ui.blank();
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
            v1_signing: self
                .v1_signing
                .or(if provider_type == ProviderType::Android {
                    Some(config.signing.android.v1_signing)
                } else {
                    None
                }),
            v2_signing: self
                .v2_signing
                .or(if provider_type == ProviderType::Android {
                    Some(config.signing.android.v2_signing)
                } else {
                    None
                }),
            v3_signing: self
                .v3_signing
                .or(if provider_type == ProviderType::Android {
                    Some(config.signing.android.v3_signing)
                } else {
                    None
                }),
            v4_signing: self
                .v4_signing
                .or(if provider_type == ProviderType::Android {
                    Some(config.signing.android.v4_signing)
                } else {
                    None
                }),
            ..Default::default()
        };

        let ui = Ui::new(cli);

        if self.dry_run {
            ui.info(&format!(
                "Would sign {} with {}",
                style(self.artifact.display()).bold(),
                style(&identity.name).green()
            ));
        } else {
            ui.info(&format!(
                "Signing {} with {}",
                style(self.artifact.display()).bold(),
                style(&identity.name).green()
            ));
        }

        provider.sign(&self.artifact, &identity, &options).await?;

        if !self.dry_run {
            ui.success("Signed successfully");
        }

        Ok(())
    }
}

impl VerifyCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
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

        if ui.is_json() {
            ui.json(&info)?;
        } else if ui.is_text() {
            ui.info(&format!(
                "Verifying {}",
                style(self.artifact.display()).bold()
            ));
            ui.blank();

            let status_style = match info.status {
                canaveral_signing::SignatureStatus::Valid => style("VALID").green().bold(),
                canaveral_signing::SignatureStatus::Invalid => style("INVALID").red().bold(),
                canaveral_signing::SignatureStatus::Expired => style("EXPIRED").yellow().bold(),
                canaveral_signing::SignatureStatus::Revoked => style("REVOKED").red().bold(),
                canaveral_signing::SignatureStatus::NotSigned => style("NOT SIGNED").dim(),
                canaveral_signing::SignatureStatus::Unknown => style("UNKNOWN").yellow(),
            };

            ui.key_value_styled("Status", status_style);

            if let Some(signer) = &info.signer {
                ui.key_value("Signer", &style(&signer.common_name).cyan().to_string());
                if let Some(team) = &signer.team_id {
                    ui.key_value("Team", team);
                }
                if let Some(expires) = &signer.expires_at {
                    let exp_str = expires.format("%Y-%m-%d").to_string();
                    if signer.certificate_valid {
                        ui.key_value("Certificate Expires", &style(&exp_str).green().to_string());
                    } else {
                        ui.key_value(
                            "Certificate Expires",
                            &style(format!("{} (EXPIRED)", exp_str)).red().to_string(),
                        );
                    }
                }
            }

            if let Some(signed_at) = info.signed_at {
                ui.key_value(
                    "Signed",
                    &signed_at.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                );
            }

            if let Some(notarized) = info.notarized {
                let notary_status = if notarized {
                    style("Yes").green()
                } else {
                    style("No").yellow()
                };
                ui.key_value_styled("Notarized", notary_status);
            }

            if !info.warnings.is_empty() {
                ui.blank();
                ui.section("Warnings");
                for warning in &info.warnings {
                    ui.warning(warning);
                }
            }

            if self.verbose {
                if let Some(details) = &info.details {
                    ui.blank();
                    ui.section("Details");
                    for line in details.lines().take(20) {
                        println!("    {}", line);
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
        let ui = Ui::new(cli);
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

        if ui.is_json() {
            ui.json(&identity)?;
        } else if ui.is_text() {
            ui.header("Signing Identity");
            ui.blank();
            ui.key_value("Name", &style(&identity.name).cyan().to_string());
            ui.key_value("Type", &identity.identity_type.to_string());
            ui.key_value("ID", &identity.id);

            if let Some(fp) = &identity.fingerprint {
                ui.key_value("Fingerprint", fp);
            }

            if let Some(team) = &identity.team_id {
                ui.key_value("Team ID", team);
            }

            if let Some(subject) = &identity.subject {
                ui.key_value("Subject", subject);
            }

            if let Some(issuer) = &identity.issuer {
                ui.key_value("Issuer", issuer);
            }

            if let Some(serial) = &identity.serial_number {
                ui.key_value("Serial", serial);
            }

            if let Some(created) = identity.created_at {
                ui.key_value("Created", &created.format("%Y-%m-%d").to_string());
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
                ui.key_value_styled("Expires", exp_style);
            }

            let valid_style = if identity.is_valid && !identity.is_expired() {
                style("Yes").green()
            } else {
                style("No").red()
            };
            ui.key_value_styled("Valid", valid_style);

            if let Some(keychain) = &identity.keychain {
                ui.key_value("Keychain", keychain);
            }

            if let Some(alias) = &identity.key_alias {
                ui.key_value("Key Alias", alias);
            }
        }

        Ok(())
    }
}

impl ProfilesCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.command {
            ProfilesSubcommand::List(_) => "list",
            ProfilesSubcommand::Install(_) => "install",
            ProfilesSubcommand::Cleanup(_) => "cleanup",
            ProfilesSubcommand::Match(_) => "match",
        };
        info!(subcommand = subcommand_name, "executing profiles command");

        match &self.command {
            ProfilesSubcommand::List(cmd) => cmd.execute(cli),
            ProfilesSubcommand::Install(cmd) => cmd.execute(cli),
            ProfilesSubcommand::Cleanup(cmd) => cmd.execute(cli),
            ProfilesSubcommand::Match(cmd) => {
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(cmd.execute(cli))
            }
        }
    }
}

impl ProfilesListCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let manager = ProfileManager::new();

        let profiles = manager.list_installed()?;

        let profiles: Vec<_> = profiles
            .into_iter()
            .filter(|p| {
                if self.valid_only && p.is_expired() {
                    return false;
                }
                if let Some(ref bundle_id) = self.bundle_id {
                    if !p.matches_bundle_id(bundle_id) {
                        return false;
                    }
                }
                if let Some(ref pt) = self.profile_type {
                    let expected = match pt.to_lowercase().as_str() {
                        "development" | "dev" => ProfileType::Development,
                        "adhoc" | "ad-hoc" => ProfileType::AdHoc,
                        "appstore" | "app-store" => ProfileType::AppStore,
                        "enterprise" => ProfileType::Enterprise,
                        _ => return true, // Unknown type, don't filter
                    };
                    if p.profile_type != expected {
                        return false;
                    }
                }
                true
            })
            .collect();

        if ui.is_json() {
            ui.json(&profiles)?;
        } else if ui.is_text() {
            ui.header("Installed Provisioning Profiles");
            ui.blank();

            if profiles.is_empty() {
                ui.hint("No matching profiles found");
            } else {
                for profile in &profiles {
                    let status = if profile.is_expired() {
                        style("EXPIRED").red()
                    } else if profile.expires_within_days(30) {
                        style("EXPIRING SOON").yellow()
                    } else {
                        style("VALID").green()
                    };

                    println!(
                        "  {} [{}] [{}]",
                        style(&profile.name).cyan(),
                        style(&profile.profile_type).dim(),
                        status,
                    );
                    ui.key_value("    Bundle ID", &profile.bundle_id);
                    ui.key_value("    UUID", &profile.uuid);
                    ui.key_value("    Team", &profile.team_id);
                    ui.key_value(
                        "    Expires",
                        &profile.expiration_date.format("%Y-%m-%d").to_string(),
                    );
                    if !profile.devices.is_empty() {
                        ui.key_value(
                            "    Devices",
                            &format!("{} registered", profile.devices.len()),
                        );
                    }
                    ui.blank();
                }

                ui.info(&format!("{} profile(s) found", profiles.len()));
            }
        }

        Ok(())
    }
}

impl ProfilesInstallCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let manager = ProfileManager::new();

        if !self.path.exists() {
            anyhow::bail!("File not found: {}", self.path.display());
        }

        ui.info(&format!(
            "Installing profile from {}",
            style(self.path.display()).bold()
        ));

        let profile = manager.install(&self.path)?;

        if ui.is_json() {
            ui.json(&profile)?;
        } else if ui.is_text() {
            ui.success(&format!(
                "Installed profile: {}",
                style(&profile.name).cyan()
            ));
            ui.key_value("UUID", &profile.uuid);
            ui.key_value("Bundle ID", &profile.bundle_id);
            ui.key_value("Type", &profile.profile_type.to_string());
            ui.key_value("Team", &profile.team_id);
            ui.key_value(
                "Expires",
                &profile.expiration_date.format("%Y-%m-%d").to_string(),
            );
        }

        Ok(())
    }
}

impl ProfilesCleanupCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let manager = ProfileManager::new();

        ui.info("Cleaning up expired provisioning profiles...");

        let removed = manager.cleanup_expired()?;

        if ui.is_json() {
            let output = serde_json::json!({
                "removed_count": removed.len(),
                "removed_uuids": removed,
            });
            ui.json(&output)?;
        } else if ui.is_text() {
            if removed.is_empty() {
                ui.success("No expired profiles found");
            } else {
                ui.success(&format!(
                    "Removed {} expired profile(s)",
                    style(removed.len()).cyan()
                ));
                for uuid in &removed {
                    println!("  {} {}", style("-").red(), style(uuid).dim());
                }
            }
        }

        Ok(())
    }
}

impl ProfilesMatchCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);

        use canaveral_signing::sync::{MatchConfig, MatchSync};

        // Load configuration
        let config_content = std::fs::read_to_string(&self.config)
            .map_err(|_| anyhow::anyhow!("Config not found. Run 'canaveral match init' first."))?;
        let mut config: MatchConfig = toml::from_str(&config_content)?;

        if let Some(ref app_ids) = self.app_ids {
            config.app_ids = app_ids.split(',').map(|s| s.trim().to_string()).collect();
        }

        // Load keypair
        let private_key = std::fs::read_to_string(&self.keyfile)
            .map_err(|_| anyhow::anyhow!("Key file not found: {}", self.keyfile.display()))?;

        let public_key = config
            .encryption_key
            .clone()
            .ok_or_else(|| anyhow::anyhow!("No encryption key in config"))?;

        let keypair = canaveral_signing::team::KeyPair {
            public_key,
            private_key,
        };

        ui.blank();
        ui.header("Syncing profiles from match repository...");
        ui.blank();

        let sync = MatchSync::new(config)?.with_keypair(keypair);
        let manifest = sync.sync().await?;

        let profile_count: usize = manifest.profiles.values().map(|v| v.len()).sum();

        if ui.is_json() {
            ui.json(&manifest)?;
        } else if ui.is_text() {
            ui.success("Profile sync complete");
            ui.info(&format!(
                "{} profile(s) synced",
                style(profile_count).cyan()
            ));

            for (app_id, profiles) in &manifest.profiles {
                for (profile_type, profile) in profiles {
                    println!(
                        "  {} {} ({}) - {} - expires {}",
                        style("*").dim(),
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

impl GenerateKeystoreCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);

        if self.output.exists() {
            anyhow::bail!(
                "Keystore already exists at {}. Remove it first or choose a different path.",
                self.output.display()
            );
        }

        let provider = canaveral_signing::AndroidProvider::new();

        let password = self.password.clone().unwrap_or_else(|| {
            std::env::var("ANDROID_KEYSTORE_PASSWORD").unwrap_or_else(|_| "changeit".to_string())
        });

        let mut dname_parts = Vec::new();
        if let Some(ref cn) = self.cn {
            dname_parts.push(format!("CN={cn}"));
        }
        if let Some(ref org) = self.org {
            dname_parts.push(format!("O={org}"));
        }
        if let Some(ref country) = self.country {
            dname_parts.push(format!("C={country}"));
        }
        let dname = if dname_parts.is_empty() {
            "CN=Unknown, O=Unknown, C=US".to_string()
        } else {
            dname_parts.join(", ")
        };

        ui.info(&format!(
            "Generating keystore at {}",
            style(self.output.display()).bold()
        ));

        provider
            .generate_keystore(
                &self.output,
                &self.alias,
                &password,
                self.validity,
                &dname,
                &self.key_algorithm,
                self.key_size,
            )
            .await?;

        ui.success("Keystore generated successfully");
        ui.key_value("Path", &self.output.display().to_string());
        ui.key_value("Alias", &self.alias);
        ui.key_value("Algorithm", &self.key_algorithm);
        ui.key_value("Key Size", &self.key_size.to_string());
        ui.key_value("Validity", &format!("{} days", self.validity));

        Ok(())
    }
}
