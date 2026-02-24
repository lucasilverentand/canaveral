//! Team vault CLI commands

use clap::{Args, Subcommand};
use console::style;
use std::path::PathBuf;
use tracing::info;

use canaveral_signing::identity::SigningIdentityType;
use canaveral_signing::team::{generate_keypair, CredentialData, Role, TeamVault};

use crate::cli::output::Ui;
use crate::cli::Cli;

/// Team vault commands
#[derive(Debug, Args)]
pub struct TeamCommand {
    #[command(subcommand)]
    pub command: TeamSubcommand,
}

/// Team vault subcommands
#[derive(Debug, Subcommand)]
pub enum TeamSubcommand {
    /// Initialize a new team vault
    Init(TeamInitCommand),

    /// Generate a new keypair for authentication
    Keygen(KeygenCommand),

    /// Show vault status
    Status(TeamStatusCommand),

    /// Member management
    Member(MemberCommand),

    /// Identity management
    Identity(IdentityCommand),

    /// View audit log
    Audit(AuditCommand),
}

/// Initialize a new team vault
#[derive(Debug, Args)]
pub struct TeamInitCommand {
    /// Team name
    #[arg(required = true)]
    pub team_name: String,

    /// Your email address
    #[arg(short, long, required = true)]
    pub email: String,

    /// Path to vault directory (default: .canaveral/signing)
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Generate a new keypair
#[derive(Debug, Args)]
pub struct KeygenCommand {
    /// Save private key to file
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

/// Show vault status
#[derive(Debug, Args)]
pub struct TeamStatusCommand {
    /// Path to vault directory
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Member management commands
#[derive(Debug, Args)]
pub struct MemberCommand {
    #[command(subcommand)]
    pub command: MemberSubcommand,
}

/// Member subcommands
#[derive(Debug, Subcommand)]
pub enum MemberSubcommand {
    /// List all members
    List(MemberListCommand),
    /// Add a new member
    Add(MemberAddCommand),
    /// Remove a member
    Remove(MemberRemoveCommand),
    /// Change a member's role
    Role(MemberRoleCommand),
}

/// List members
#[derive(Debug, Args)]
pub struct MemberListCommand {
    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Add a member
#[derive(Debug, Args)]
pub struct MemberAddCommand {
    /// Member's email
    #[arg(required = true)]
    pub email: String,

    /// Member's Age public key
    #[arg(required = true)]
    pub public_key: String,

    /// Member's role (admin, signer, viewer)
    #[arg(short, long, default_value = "signer")]
    pub role: String,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Remove a member
#[derive(Debug, Args)]
pub struct MemberRemoveCommand {
    /// Member's email
    #[arg(required = true)]
    pub email: String,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Change member role
#[derive(Debug, Args)]
pub struct MemberRoleCommand {
    /// Member's email
    #[arg(required = true)]
    pub email: String,

    /// New role (admin, signer, viewer)
    #[arg(required = true)]
    pub role: String,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Identity management commands
#[derive(Debug, Args)]
pub struct IdentityCommand {
    #[command(subcommand)]
    pub command: IdentitySubcommand,
}

/// Identity subcommands
#[derive(Debug, Subcommand)]
pub enum IdentitySubcommand {
    /// List all identities
    List(IdentityListCommand),
    /// Import a signing identity
    Import(IdentityImportCommand),
    /// Export a signing identity
    Export(IdentityExportCommand),
    /// Delete a signing identity
    Delete(IdentityDeleteCommand),
}

/// List identities
#[derive(Debug, Args)]
pub struct IdentityListCommand {
    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Import an identity
#[derive(Debug, Args)]
pub struct IdentityImportCommand {
    /// Identity ID (short name)
    #[arg(required = true)]
    pub id: String,

    /// Path to certificate/key file
    #[arg(required = true)]
    pub file: PathBuf,

    /// Display name
    #[arg(short, long)]
    pub name: Option<String>,

    /// Identity type (apple-developer, apple-distribution, android-keystore, gpg, etc.)
    #[arg(short = 't', long, default_value = "generic")]
    pub identity_type: String,

    /// Password for the credential file
    #[arg(long)]
    pub password: Option<String>,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Export an identity
#[derive(Debug, Args)]
pub struct IdentityExportCommand {
    /// Identity ID
    #[arg(required = true)]
    pub id: String,

    /// Output path
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

/// Delete an identity
#[derive(Debug, Args)]
pub struct IdentityDeleteCommand {
    /// Identity ID
    #[arg(required = true)]
    pub id: String,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,

    /// Skip confirmation
    #[arg(long)]
    pub force: bool,
}

/// View audit log
#[derive(Debug, Args)]
pub struct AuditCommand {
    /// Number of entries to show
    #[arg(short, long, default_value = "20")]
    pub limit: usize,

    /// Filter by actor email
    #[arg(long)]
    pub actor: Option<String>,

    /// Filter by identity ID
    #[arg(long)]
    pub identity: Option<String>,

    /// Path to vault
    #[arg(short, long)]
    pub path: Option<PathBuf>,
}

impl TeamCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.command {
            TeamSubcommand::Init(_) => "init",
            TeamSubcommand::Keygen(_) => "keygen",
            TeamSubcommand::Status(_) => "status",
            TeamSubcommand::Member(_) => "member",
            TeamSubcommand::Identity(_) => "identity",
            TeamSubcommand::Audit(_) => "audit",
        };
        info!(subcommand = subcommand_name, "executing team command");
        match &self.command {
            TeamSubcommand::Init(cmd) => cmd.execute(cli),
            TeamSubcommand::Keygen(cmd) => cmd.execute(cli),
            TeamSubcommand::Status(cmd) => cmd.execute(cli),
            TeamSubcommand::Member(cmd) => cmd.execute(cli),
            TeamSubcommand::Identity(cmd) => cmd.execute(cli),
            TeamSubcommand::Audit(cmd) => cmd.execute(cli),
        }
    }
}

fn get_vault_path(path: Option<&PathBuf>) -> PathBuf {
    path.cloned().unwrap_or_else(|| {
        std::env::current_dir()
            .unwrap_or_default()
            .join(".canaveral")
            .join("signing")
    })
}

impl TeamInitCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());

        let (vault, keypair) = TeamVault::init(&self.team_name, &path, &self.email)?;

        if ui.is_json() {
            let output = serde_json::json!({
                "team_name": vault.team_name(),
                "path": path.to_string_lossy(),
                "public_key": keypair.public_key,
                "private_key": keypair.private_key,
            });
            ui.json(&output)?;
        } else if ui.is_text() {
            ui.success("Team vault initialized!");
            ui.blank();
            ui.key_value("Team", &style(vault.team_name()).cyan().to_string());
            ui.key_value("Path", &path.display().to_string());
            ui.blank();
            ui.section("Your keypair");
            ui.blank();
            println!("  {}", style("Public key (share with team):").underlined());
            println!("  {}", style(&keypair.public_key).green());
            ui.blank();
            println!(
                "  {}",
                style("Private key (KEEP SECRET!):").underlined().red()
            );
            println!("  {}", &keypair.private_key);
            ui.blank();
            ui.warning("Important:");
            ui.hint("1. Save your private key securely (password manager, etc.)");
            ui.hint("2. Set CANAVERAL_SIGNING_KEY env var to authenticate");
            ui.hint("3. Commit the vault files to version control");
            ui.blank();
            ui.hint(
                &style("export CANAVERAL_SIGNING_KEY=\"...\"")
                    .dim()
                    .to_string(),
            );
        }

        Ok(())
    }
}

impl KeygenCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let keypair = generate_keypair();

        if let Some(output) = &self.output {
            std::fs::write(output, &keypair.private_key)?;
            // Set restrictive permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o600))?;
            }
            ui.info(&format!("Private key saved to: {}", output.display()));
            ui.info(&format!("Public key: {}", keypair.public_key));
        } else if ui.is_json() {
            let output = serde_json::json!({
                "public_key": keypair.public_key,
                "private_key": keypair.private_key,
            });
            ui.json(&output)?;
        } else if ui.is_text() {
            ui.success("Generated new keypair");
            ui.blank();
            println!("  {}", style("Public key:").underlined());
            println!("  {}", style(&keypair.public_key).green());
            ui.blank();
            println!(
                "  {}",
                style("Private key (KEEP SECRET!):").underlined().red()
            );
            println!("  {}", &keypair.private_key);
        }

        Ok(())
    }
}

impl TeamStatusCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let vault = TeamVault::open(&path)?;

        let members = vault.list_members();
        let identities = vault.list_identities();

        if ui.is_json() {
            let output = serde_json::json!({
                "team_name": vault.team_name(),
                "path": path.to_string_lossy(),
                "member_count": members.len(),
                "identity_count": identities.len(),
                "current_user": vault.current_member().map(|m| &m.email),
            });
            ui.json(&output)?;
        } else if ui.is_text() {
            ui.header("Team Vault Status");
            ui.blank();
            ui.key_value("Team", &style(vault.team_name()).cyan().to_string());
            ui.key_value("Path", &path.display().to_string());
            ui.key_value("Members", &members.len().to_string());
            ui.key_value("Identities", &identities.len().to_string());

            if let Some(member) = vault.current_member() {
                ui.key_value(
                    "Logged in",
                    &format!("{} ({})", style(&member.email).green(), member.role),
                );
            } else {
                ui.key_value_styled("Logged in", style("Not authenticated").yellow());
            }
        }

        Ok(())
    }
}

impl MemberCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            MemberSubcommand::List(cmd) => cmd.execute(cli),
            MemberSubcommand::Add(cmd) => cmd.execute(cli),
            MemberSubcommand::Remove(cmd) => cmd.execute(cli),
            MemberSubcommand::Role(cmd) => cmd.execute(cli),
        }
    }
}

impl MemberListCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let vault = TeamVault::open(&path)?;
        let members = vault.list_members();

        if ui.is_json() {
            ui.json(&members)?;
        } else if ui.is_text() {
            ui.header("Team Members");
            ui.blank();

            for member in members {
                let role_style = match member.role {
                    Role::Admin => style(member.role.to_string()).red().bold(),
                    Role::Signer => style(member.role.to_string()).green(),
                    Role::Viewer => style(member.role.to_string()).dim(),
                };

                let status = if member.active {
                    style("active").green()
                } else {
                    style("inactive").dim()
                };

                println!(
                    "  {} [{}] ({})",
                    style(&member.email).cyan(),
                    role_style,
                    status
                );

                let short_key = if member.public_key.len() > 20 {
                    format!("{}...", &member.public_key[..20])
                } else {
                    member.public_key.clone()
                };
                println!("    Key: {}", style(short_key).dim());
            }
        }

        Ok(())
    }
}

impl MemberAddCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        let role: Role = self.role.parse().map_err(|e: String| anyhow::anyhow!(e))?;
        vault.add_member(&self.email, &self.public_key, role)?;

        ui.success(&format!(
            "Added {} as {}",
            style(&self.email).cyan(),
            style(role.to_string()).green()
        ));

        Ok(())
    }
}

impl MemberRemoveCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        vault.remove_member(&self.email)?;

        ui.success(&format!("Removed {}", style(&self.email).cyan()));

        Ok(())
    }
}

impl MemberRoleCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        let role: Role = self.role.parse().map_err(|e: String| anyhow::anyhow!(e))?;
        vault.change_role(&self.email, role)?;

        ui.success(&format!(
            "Changed {} to {}",
            style(&self.email).cyan(),
            style(role.to_string()).green()
        ));

        Ok(())
    }
}

impl IdentityCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            IdentitySubcommand::List(cmd) => cmd.execute(cli),
            IdentitySubcommand::Import(cmd) => cmd.execute(cli),
            IdentitySubcommand::Export(cmd) => cmd.execute(cli),
            IdentitySubcommand::Delete(cmd) => cmd.execute(cli),
        }
    }
}

impl IdentityListCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let vault = TeamVault::open(&path)?;
        let identities = vault.list_identities();

        if ui.is_json() {
            // Don't include encrypted data in JSON output
            let safe_identities: Vec<_> = identities
                .iter()
                .map(|i| {
                    serde_json::json!({
                        "id": i.id,
                        "name": i.name,
                        "type": format!("{:?}", i.identity_type),
                        "expires_at": i.expires_at,
                        "tags": i.tags,
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&safe_identities)?);
        } else if ui.is_text() {
            ui.header("Stored Identities");
            ui.blank();

            if identities.is_empty() {
                ui.hint("No identities stored");
            } else {
                for identity in identities {
                    println!(
                        "  {} ({})",
                        style(&identity.id).cyan().bold(),
                        identity.identity_type
                    );
                    ui.key_value("    Name", &identity.name);

                    if let Some(exp) = identity.expires_at {
                        ui.key_value("    Expires", &exp.format("%Y-%m-%d").to_string());
                    }

                    if !identity.tags.is_empty() {
                        ui.key_value("    Tags", &identity.tags.join(", "));
                    }

                    ui.blank();
                }
            }
        }

        Ok(())
    }
}

impl IdentityImportCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        // Read the credential file
        let data = std::fs::read(&self.file)?;

        // Determine format from extension
        let format = self
            .file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_lowercase();

        let mut credential = CredentialData::new(data, &format);
        if let Some(password) = &self.password {
            credential = credential.with_password(password);
        }

        // Parse identity type
        let identity_type = match self.identity_type.to_lowercase().as_str() {
            "apple-developer" | "developer-id" => SigningIdentityType::AppleDeveloper,
            "apple-distribution" | "distribution" => SigningIdentityType::AppleDistribution,
            "apple-installer" | "installer" => SigningIdentityType::AppleInstaller,
            "windows" | "authenticode" => SigningIdentityType::WindowsAuthenticode,
            "windows-ev" | "ev" => SigningIdentityType::WindowsEV,
            "android" | "android-keystore" | "keystore" => SigningIdentityType::AndroidKeystore,
            "gpg" | "pgp" => SigningIdentityType::Gpg,
            _ => SigningIdentityType::Generic,
        };

        let name = self.name.as_ref().unwrap_or(&self.id);

        vault.import_identity(&self.id, name, identity_type, credential)?;

        ui.success(&format!(
            "Imported {} ({})",
            style(&self.id).cyan(),
            identity_type
        ));

        Ok(())
    }
}

impl IdentityExportCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        let credential = vault.export_identity(&self.id)?;

        if let Some(output) = &self.output {
            std::fs::write(output, &credential.data)?;
            ui.success(&format!(
                "Exported {} to {}",
                style(&self.id).cyan(),
                output.display()
            ));
            if credential.password.is_some() {
                ui.hint("Password is required to use this credential");
            }
        } else if ui.is_json() {
            ui.json(&credential)?;
        } else if ui.is_text() {
            ui.header("Exported credential");
            ui.key_value("Format", &credential.format);
            ui.key_value("Size", &format!("{} bytes", credential.data.len()));
            if credential.password.is_some() {
                ui.key_value("Password", "(set)");
            }
            ui.blank();
            ui.hint(&format!(
                "Use {} to save to file",
                style("--output <path>").cyan()
            ));
        }

        Ok(())
    }
}

impl IdentityDeleteCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let mut vault = TeamVault::open(&path)?;

        if !self.force {
            ui.warning(&format!(
                "Are you sure you want to delete {}? This cannot be undone.",
                style(&self.id).red()
            ));
            ui.hint("Use --force to skip this confirmation.");
            return Ok(());
        }

        vault.delete_identity(&self.id)?;

        ui.success(&format!("Deleted {}", style(&self.id).cyan()));

        Ok(())
    }
}

impl AuditCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let path = get_vault_path(self.path.as_ref());
        let vault = TeamVault::open(&path)?;
        let audit = vault.audit_log();

        let entries: Vec<_> = if let Some(actor) = &self.actor {
            audit.entries_by_actor(actor)
        } else if let Some(identity) = &self.identity {
            audit.entries_for_identity(identity)
        } else {
            audit.last_n(self.limit)
        };

        if ui.is_json() {
            ui.json(&entries)?;
        } else if ui.is_text() {
            ui.header("Audit Log");
            ui.blank();

            if entries.is_empty() {
                ui.hint("No audit entries");
            } else {
                for entry in entries.iter().rev().take(self.limit) {
                    let time = entry.timestamp.format("%Y-%m-%d %H:%M:%S");
                    println!(
                        "  {} {} {}",
                        style(time.to_string()).dim(),
                        style(&entry.actor).cyan(),
                        entry.action
                    );
                }
            }
        }

        Ok(())
    }
}
