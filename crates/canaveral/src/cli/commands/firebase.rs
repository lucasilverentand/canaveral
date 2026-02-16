//! Firebase App Distribution command - Manage Firebase beta testing

use std::path::PathBuf;

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_stores::firebase::{
    Firebase, FirebaseConfig, FirebaseRelease, FirebaseUploadOptions, TesterGroup,
};

use crate::cli::{Cli, OutputFormat};

/// Firebase App Distribution management
#[derive(Debug, Args)]
pub struct FirebaseCommand {
    #[command(subcommand)]
    pub subcommand: FirebaseSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum FirebaseSubcommand {
    /// Upload an artifact to Firebase App Distribution
    Upload(UploadArgs),

    /// List releases
    Releases(ReleasesCommand),

    /// Manage tester groups
    Groups(GroupsCommand),

    /// Manage testers
    Testers(TestersCommand),
}

/// Upload arguments
#[derive(Debug, Args)]
pub struct UploadArgs {
    /// Path to artifact (APK, AAB, or IPA)
    pub artifact: PathBuf,

    /// Firebase project ID
    #[arg(long, env = "FIREBASE_PROJECT_ID")]
    pub project_id: Option<String>,

    /// Firebase app ID (e.g., "1:123456789:ios:abcdef")
    #[arg(long, env = "FIREBASE_APP_ID")]
    pub app_id: Option<String>,

    /// Tester groups to distribute to (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub groups: Vec<String>,

    /// Tester emails to distribute to (comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub testers: Vec<String>,

    /// Release notes
    #[arg(long)]
    pub notes: Option<String>,

    /// Path to file containing release notes
    #[arg(long, conflicts_with = "notes")]
    pub notes_file: Option<PathBuf>,

    /// Perform a dry run (validate but don't upload)
    #[arg(long)]
    pub dry_run: bool,
}

/// Releases command
#[derive(Debug, Args)]
pub struct ReleasesCommand {
    #[command(subcommand)]
    pub subcommand: ReleasesSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ReleasesSubcommand {
    /// List recent releases
    List {
        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,

        /// Maximum number of releases to list
        #[arg(long, default_value = "25")]
        limit: usize,
    },

    /// Get release details
    Get {
        /// Release name (full resource name)
        name: String,

        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,
    },
}

/// Groups command
#[derive(Debug, Args)]
pub struct GroupsCommand {
    #[command(subcommand)]
    pub subcommand: GroupsSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum GroupsSubcommand {
    /// List tester groups
    List {
        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,
    },

    /// Create a new tester group
    Create {
        /// Group alias (identifier)
        alias: String,

        /// Display name for the group
        #[arg(long)]
        display_name: Option<String>,

        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,
    },

    /// Delete a tester group
    Delete {
        /// Group alias
        alias: String,

        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
}

/// Testers command
#[derive(Debug, Args)]
pub struct TestersCommand {
    #[command(subcommand)]
    pub subcommand: TestersSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TestersSubcommand {
    /// Add testers to a group
    Add {
        /// Tester emails (comma-separated)
        #[arg(value_delimiter = ',')]
        emails: Vec<String>,

        /// Group alias to add testers to
        #[arg(long)]
        group: String,

        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,
    },

    /// Remove testers from a group
    Remove {
        /// Tester emails (comma-separated)
        #[arg(value_delimiter = ',')]
        emails: Vec<String>,

        /// Group alias to remove testers from
        #[arg(long)]
        group: String,

        /// Firebase project ID
        #[arg(long, env = "FIREBASE_PROJECT_ID")]
        project_id: Option<String>,

        /// Firebase app ID
        #[arg(long, env = "FIREBASE_APP_ID")]
        app_id: Option<String>,
    },
}

impl FirebaseCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.subcommand {
            FirebaseSubcommand::Upload(_) => "upload",
            FirebaseSubcommand::Releases(_) => "releases",
            FirebaseSubcommand::Groups(_) => "groups",
            FirebaseSubcommand::Testers(_) => "testers",
        };
        info!(subcommand = subcommand_name, "executing firebase command");
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.subcommand {
            FirebaseSubcommand::Upload(args) => self.upload(args, cli).await,
            FirebaseSubcommand::Releases(cmd) => self.releases(cmd, cli).await,
            FirebaseSubcommand::Groups(cmd) => self.groups(cmd, cli).await,
            FirebaseSubcommand::Testers(cmd) => self.testers(cmd, cli).await,
        }
    }

    async fn upload(&self, args: &UploadArgs, cli: &Cli) -> anyhow::Result<()> {
        if !args.artifact.exists() {
            anyhow::bail!("Artifact not found: {}", args.artifact.display());
        }

        let mut firebase = self.get_firebase(
            args.project_id.as_deref(),
            args.app_id.as_deref(),
        )?;

        // Read release notes from file if specified
        let release_notes = if let Some(ref notes_file) = args.notes_file {
            Some(std::fs::read_to_string(notes_file)
                .map_err(|e| anyhow::anyhow!("Failed to read notes file: {}", e))?)
        } else {
            args.notes.clone()
        };

        let options = FirebaseUploadOptions {
            release_notes,
            groups: args.groups.clone(),
            testers: args.testers.clone(),
            dry_run: args.dry_run,
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Uploading to Firebase App Distribution...").bold());
            println!("  File: {}", style(args.artifact.display()).cyan());
            if !args.groups.is_empty() {
                println!("  Groups: {}", style(args.groups.join(", ")).dim());
            }
            if !args.testers.is_empty() {
                println!("  Testers: {}", style(format!("{} recipients", args.testers.len())).dim());
            }
            if args.dry_run {
                println!("  {}", style("(DRY RUN)").yellow().bold());
            }
            println!();
        }

        let release = firebase.upload(&args.artifact, &options).await?;

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&release)?);
        } else if !cli.quiet {
            println!(
                "{} Upload completed!",
                style("✓").green().bold()
            );
            println!("  Version: {}", style(&release.display_version).cyan());
            println!("  Build: {}", style(&release.build_version).dim());
            if let Some(ref uri) = release.firebase_console_uri {
                println!("  Console: {}", style(uri).dim());
            }
        }

        Ok(())
    }

    async fn releases(&self, cmd: &ReleasesCommand, cli: &Cli) -> anyhow::Result<()> {
        match &cmd.subcommand {
            ReleasesSubcommand::List { project_id, app_id, limit } => {
                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let releases = firebase.list_releases(Some(*limit)).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&releases)?);
                } else if !cli.quiet {
                    if releases.is_empty() {
                        println!("No releases found");
                    } else {
                        println!("{}", style("Firebase Releases").bold());
                        println!();
                        for release in &releases {
                            self.print_release(release);
                            println!();
                        }
                    }
                }
            }

            ReleasesSubcommand::Get { name, project_id, app_id } => {
                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let release = firebase.get_release(name).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&release)?);
                } else if !cli.quiet {
                    self.print_release(&release);
                }
            }
        }

        Ok(())
    }

    async fn groups(&self, cmd: &GroupsCommand, cli: &Cli) -> anyhow::Result<()> {
        match &cmd.subcommand {
            GroupsSubcommand::List { project_id, app_id } => {
                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let groups = firebase.list_groups().await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&groups)?);
                } else if !cli.quiet {
                    if groups.is_empty() {
                        println!("No groups found");
                    } else {
                        println!("{}", style("Tester Groups").bold());
                        println!();
                        for group in &groups {
                            self.print_group(group);
                        }
                    }
                }
            }

            GroupsSubcommand::Create { alias, display_name, project_id, app_id } => {
                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let group = firebase.create_group(alias, display_name.as_deref()).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&group)?);
                } else if !cli.quiet {
                    println!(
                        "{} Created group '{}'",
                        style("✓").green().bold(),
                        style(alias).cyan()
                    );
                    if let Some(ref display) = group.display_name {
                        println!("  Display Name: {}", style(display).dim());
                    }
                }
            }

            GroupsSubcommand::Delete { alias, project_id, app_id, yes } => {
                if !yes && cli.format == OutputFormat::Text {
                    use dialoguer::Confirm;

                    let confirmed = Confirm::new()
                        .with_prompt(format!("Are you sure you want to delete group '{}'?", alias))
                        .default(false)
                        .interact()?;

                    if !confirmed {
                        println!("Cancelled");
                        return Ok(());
                    }
                }

                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                firebase.delete_group(alias).await?;

                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "{} Deleted group '{}'",
                        style("✓").green().bold(),
                        style(alias).cyan()
                    );
                }
            }
        }

        Ok(())
    }

    async fn testers(&self, cmd: &TestersCommand, cli: &Cli) -> anyhow::Result<()> {
        match &cmd.subcommand {
            TestersSubcommand::Add { emails, group, project_id, app_id } => {
                if emails.is_empty() {
                    anyhow::bail!("At least one email is required");
                }

                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let email_refs: Vec<&str> = emails.iter().map(|s| s.as_str()).collect();
                firebase.add_testers_to_group(group, &email_refs).await?;

                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "{} Added {} tester(s) to group '{}'",
                        style("✓").green().bold(),
                        emails.len(),
                        style(group).cyan()
                    );
                    for email in emails {
                        println!("  - {}", style(email).dim());
                    }
                }
            }

            TestersSubcommand::Remove { emails, group, project_id, app_id } => {
                if emails.is_empty() {
                    anyhow::bail!("At least one email is required");
                }

                let mut firebase = self.get_firebase(project_id.as_deref(), app_id.as_deref())?;
                let email_refs: Vec<&str> = emails.iter().map(|s| s.as_str()).collect();
                firebase.remove_testers_from_group(group, &email_refs).await?;

                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "{} Removed {} tester(s) from group '{}'",
                        style("✓").green().bold(),
                        emails.len(),
                        style(group).cyan()
                    );
                    for email in emails {
                        println!("  - {}", style(email).dim());
                    }
                }
            }
        }

        Ok(())
    }

    fn get_firebase(
        &self,
        project_id: Option<&str>,
        app_id: Option<&str>,
    ) -> anyhow::Result<Firebase> {
        // Try to get from arguments first, then fall back to env
        let project_id = project_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("FIREBASE_PROJECT_ID").ok())
            .or_else(|| std::env::var("GOOGLE_CLOUD_PROJECT").ok())
            .ok_or_else(|| anyhow::anyhow!("FIREBASE_PROJECT_ID not set"))?;

        let app_id = app_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("FIREBASE_APP_ID").ok())
            .ok_or_else(|| anyhow::anyhow!("FIREBASE_APP_ID not set"))?;

        let service_account = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok()
            .or_else(|| std::env::var("FIREBASE_SERVICE_ACCOUNT").ok());

        let cli_token = std::env::var("FIREBASE_TOKEN").ok();

        if service_account.is_none() && cli_token.is_none() {
            anyhow::bail!("Either GOOGLE_APPLICATION_CREDENTIALS or FIREBASE_TOKEN must be set");
        }

        Ok(Firebase::new(FirebaseConfig {
            project_id,
            app_id,
            service_account,
            cli_token,
        }))
    }

    fn print_release(&self, release: &FirebaseRelease) {
        println!(
            "  {} {}",
            style("Release").bold(),
            style(&release.display_version).cyan()
        );
        println!("    Build: {}", style(&release.build_version).dim());
        println!("    Created: {}", style(release.create_time.format("%Y-%m-%d %H:%M UTC")).dim());
        if let Some(ref notes) = release.release_notes {
            // Truncate long notes for display
            let display_notes = if notes.len() > 100 {
                format!("{}...", &notes[..100])
            } else {
                notes.clone()
            };
            println!("    Notes: {}", style(display_notes).dim());
        }
        if let Some(ref uri) = release.firebase_console_uri {
            println!("    Console: {}", style(uri).dim());
        }
    }

    fn print_group(&self, group: &TesterGroup) {
        println!(
            "  {} - {}",
            style(&group.alias).cyan().bold(),
            group.display_name.as_deref().unwrap_or(&group.alias)
        );
        println!(
            "    Testers: {}",
            style(group.tester_count.to_string()).dim()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_args() {
        let args = UploadArgs {
            artifact: PathBuf::from("app.apk"),
            project_id: Some("my-project".to_string()),
            app_id: Some("1:123:android:abc".to_string()),
            groups: vec!["testers".to_string(), "qa".to_string()],
            testers: vec!["test@example.com".to_string()],
            notes: Some("Release notes".to_string()),
            notes_file: None,
            dry_run: false,
        };

        assert_eq!(args.groups.len(), 2);
        assert_eq!(args.testers.len(), 1);
    }
}
