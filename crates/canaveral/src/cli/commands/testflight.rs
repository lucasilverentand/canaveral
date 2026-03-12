//! TestFlight command - Manage TestFlight beta testing

use std::path::PathBuf;

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_stores::apple::{
    BetaGroup, BetaReviewState, BetaTester, BuildProcessingState, TestFlight, TestFlightBuild,
};
use canaveral_stores::types::AppleStoreConfig;

use crate::cli::output::Ui;
use crate::cli::Cli;

/// TestFlight beta testing management
#[derive(Debug, Args)]
pub struct TestFlightCommand {
    #[command(subcommand)]
    pub subcommand: TestFlightSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TestFlightSubcommand {
    /// Upload an IPA to TestFlight
    Upload(UploadArgs),

    /// Get build status
    Status(StatusArgs),

    /// List TestFlight builds
    Builds(BuildsArgs),

    /// Manage beta testers
    Testers(TestersCommand),

    /// Manage beta groups
    Groups(GroupsCommand),

    /// Submit build for beta review
    Submit(SubmitArgs),

    /// Expire a build
    Expire(ExpireArgs),
}

/// Upload arguments
#[derive(Debug, Args)]
pub struct UploadArgs {
    /// Path to IPA file
    pub ipa: PathBuf,

    /// App Store Connect API Key ID
    #[arg(long, env = "APP_STORE_CONNECT_API_KEY_ID")]
    pub api_key_id: Option<String>,

    /// App Store Connect Issuer ID
    #[arg(long, env = "APP_STORE_CONNECT_ISSUER_ID")]
    pub issuer_id: Option<String>,

    /// Path to API key (.p8 file)
    #[arg(long, env = "APP_STORE_CONNECT_API_KEY_PATH")]
    pub api_key: Option<String>,

    /// Skip TestFlight distribution (upload only)
    #[arg(long)]
    pub skip_distribution: bool,

    /// "What's New" text for testers
    #[arg(long)]
    pub changelog: Option<String>,

    /// Locale for changelog
    #[arg(long, default_value = "en-US")]
    pub locale: String,

    /// Dry run - validate but don't upload
    #[arg(long)]
    pub dry_run: bool,
}

/// Status arguments
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Build ID
    pub build_id: Option<String>,

    /// App bundle ID (to find latest build)
    #[arg(long)]
    pub bundle_id: Option<String>,

    /// Build number to find
    #[arg(long)]
    pub build_number: Option<String>,
}

/// Builds list arguments
#[derive(Debug, Args)]
pub struct BuildsArgs {
    /// App bundle ID
    #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
    pub bundle_id: String,

    /// Maximum number of builds to list
    #[arg(long, default_value = "25")]
    pub limit: usize,

    /// Show only processing builds
    #[arg(long)]
    pub processing: bool,
}

/// Testers command
#[derive(Debug, Args)]
pub struct TestersCommand {
    #[command(subcommand)]
    pub subcommand: TestersSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum TestersSubcommand {
    /// List beta testers
    List {
        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,

        /// Filter by group name
        #[arg(long)]
        group: Option<String>,
    },

    /// Add a beta tester
    Add {
        /// Tester email address
        email: String,

        /// Group to add tester to
        #[arg(long)]
        group: String,

        /// Tester first name
        #[arg(long)]
        first_name: Option<String>,

        /// Tester last name
        #[arg(long)]
        last_name: Option<String>,

        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,
    },

    /// Remove a beta tester
    Remove {
        /// Tester email address
        email: String,

        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,
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
    /// List beta groups
    List {
        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,
    },

    /// Create a new beta group
    Create {
        /// Group name
        name: String,

        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,

        /// Create as internal group
        #[arg(long)]
        internal: bool,
    },

    /// Delete a beta group
    Delete {
        /// Group name
        name: String,

        /// App bundle ID
        #[arg(long, env = "CANAVERAL_BUNDLE_ID")]
        bundle_id: String,
    },
}

/// Submit arguments
#[derive(Debug, Args)]
pub struct SubmitArgs {
    /// Build ID
    pub build_id: String,

    /// "What's New" text for review
    #[arg(long)]
    pub changelog: Option<String>,

    /// Locale for changelog
    #[arg(long, default_value = "en-US")]
    pub locale: String,
}

/// Expire arguments
#[derive(Debug, Args)]
pub struct ExpireArgs {
    /// Build ID to expire
    pub build_id: String,

    /// Skip confirmation
    #[arg(long, short = 'y')]
    pub yes: bool,
}

impl TestFlightCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.subcommand {
            TestFlightSubcommand::Upload(_) => "upload",
            TestFlightSubcommand::Status(_) => "status",
            TestFlightSubcommand::Builds(_) => "builds",
            TestFlightSubcommand::Testers(_) => "testers",
            TestFlightSubcommand::Groups(_) => "groups",
            TestFlightSubcommand::Submit(_) => "submit",
            TestFlightSubcommand::Expire(_) => "expire",
        };
        info!(subcommand = subcommand_name, "executing testflight command");
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.subcommand {
            TestFlightSubcommand::Upload(args) => self.upload(args, cli).await,
            TestFlightSubcommand::Status(args) => self.status(args, cli).await,
            TestFlightSubcommand::Builds(args) => self.builds(args, cli).await,
            TestFlightSubcommand::Testers(cmd) => self.testers(cmd, cli).await,
            TestFlightSubcommand::Groups(cmd) => self.groups(cmd, cli).await,
            TestFlightSubcommand::Submit(args) => self.submit(args, cli).await,
            TestFlightSubcommand::Expire(args) => self.expire(args, cli).await,
        }
    }

    async fn upload(&self, args: &UploadArgs, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_stores::apple::AppStoreConnect;
        use canaveral_stores::traits::StoreAdapter;
        use canaveral_stores::types::UploadOptions;

        let ui = Ui::new(cli);

        if !args.ipa.exists() {
            anyhow::bail!("IPA file not found: {}", args.ipa.display());
        }

        let config = self.get_config(
            args.api_key_id.as_deref(),
            args.issuer_id.as_deref(),
            args.api_key.as_deref(),
        )?;

        ui.blank();
        ui.header("Uploading to TestFlight...");
        ui.key_value("File", &style(args.ipa.display()).cyan().to_string());
        if let Ok(meta) = std::fs::metadata(&args.ipa) {
            let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
            ui.key_value("Size", &format!("{:.1} MB", size_mb));
        }
        if let Some(ref changelog) = args.changelog {
            ui.key_value("What's New", &style(changelog).dim().to_string());
        }
        if args.dry_run {
            ui.warning("DRY RUN");
        }
        ui.blank();

        // Dry run: validate only
        if args.dry_run {
            let store = AppStoreConnect::new(config)?;

            ui.info("Validating IPA...");
            let validation = store.validate_artifact(&args.ipa).await?;

            if ui.is_json() {
                ui.json(&validation)?;
            } else if ui.is_text() {
                if validation.valid {
                    ui.success("IPA is valid for TestFlight upload");
                    if let Some(ref app_info) = validation.app_info {
                        ui.key_value("Bundle ID", &style(&app_info.identifier).cyan().to_string());
                        ui.key_value("Version", &style(&app_info.version).cyan().to_string());
                        ui.key_value("Build", &style(&app_info.build_number).cyan().to_string());
                    }
                } else {
                    ui.error("IPA validation failed");
                    for error in &validation.errors {
                        ui.warning(&error.message);
                    }
                }
            }
            return Ok(());
        }

        let store = AppStoreConnect::new(config)?;

        let mut release_notes = std::collections::HashMap::new();
        if let Some(ref changelog) = args.changelog {
            release_notes.insert(args.locale.clone(), changelog.clone());
        }

        let options = UploadOptions {
            track: Some("testflight".to_string()),
            dry_run: false,
            verbose: cli.verbose,
            release_notes,
            ..Default::default()
        };

        let result = store.upload(&args.ipa, &options).await?;

        if ui.is_json() {
            ui.json(&result)?;
        } else if ui.is_text() {
            if result.success {
                ui.success("Upload completed!");
                if let Some(ref build_id) = result.build_id {
                    ui.key_value("Build ID", &style(build_id).cyan().to_string());
                }
                ui.key_value(
                    "Status",
                    &style(result.status.to_string()).yellow().to_string(),
                );
                if let Some(ref url) = result.console_url {
                    ui.key_value("Console", &style(url).dim().to_string());
                }

                if !args.skip_distribution {
                    ui.blank();
                    ui.hint(
                        "Build will be available to TestFlight testers after processing completes.",
                    );
                    ui.hint("Check status with: canaveral testflight status");
                }
            } else {
                ui.error("Upload failed");
                if !result.warnings.is_empty() {
                    for warning in &result.warnings {
                        ui.warning(warning);
                    }
                }
            }
        }

        Ok(())
    }

    async fn status(&self, args: &StatusArgs, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let mut testflight = TestFlight::from_env()?;

        let build = if let Some(ref build_id) = args.build_id {
            testflight.get_build(build_id).await?
        } else if let Some(ref bundle_id) = args.bundle_id {
            let app_id = testflight.get_app_id(bundle_id).await?;
            let builds = testflight.list_builds(&app_id, Some(1)).await?;
            builds
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No builds found for {}", bundle_id))?
        } else {
            anyhow::bail!("Either build_id or bundle_id is required");
        };

        if ui.is_json() {
            ui.json(&build)?;
        } else if ui.is_text() {
            self.print_build(&build);
        }

        Ok(())
    }

    async fn builds(&self, args: &BuildsArgs, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let mut testflight = TestFlight::from_env()?;

        let app_id = testflight.get_app_id(&args.bundle_id).await?;
        let builds = testflight.list_builds(&app_id, Some(args.limit)).await?;

        let builds: Vec<_> = if args.processing {
            builds
                .into_iter()
                .filter(|b| b.processing_state == BuildProcessingState::Processing)
                .collect()
        } else {
            builds
        };

        if ui.is_json() {
            ui.json(&builds)?;
        } else if ui.is_text() {
            if builds.is_empty() {
                ui.info("No builds found");
            } else {
                ui.header("TestFlight Builds");
                ui.blank();
                for build in &builds {
                    self.print_build(build);
                    ui.blank();
                }
            }
        }

        Ok(())
    }

    async fn testers(&self, cmd: &TestersCommand, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let mut testflight = TestFlight::from_env()?;

        match &cmd.subcommand {
            TestersSubcommand::List { bundle_id, group } => {
                let app_id = testflight.get_app_id(bundle_id).await?;

                let group_id = if let Some(ref group_name) = group {
                    let groups = testflight.list_beta_groups(&app_id).await?;
                    groups
                        .iter()
                        .find(|g| g.name.eq_ignore_ascii_case(group_name))
                        .map(|g| g.id.clone())
                } else {
                    None
                };

                let testers = testflight
                    .list_testers(&app_id, group_id.as_deref())
                    .await?;

                if ui.is_json() {
                    ui.json(&testers)?;
                } else if ui.is_text() {
                    if testers.is_empty() {
                        ui.info("No testers found");
                    } else {
                        ui.header("Beta Testers");
                        ui.blank();
                        for tester in &testers {
                            self.print_tester(tester);
                        }
                    }
                }
            }

            TestersSubcommand::Add {
                email,
                group,
                first_name,
                last_name,
                bundle_id,
            } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                let group_id = groups
                    .iter()
                    .find(|g| g.name.eq_ignore_ascii_case(group))
                    .map(|g| g.id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Group not found: {}", group))?;

                let tester = testflight
                    .invite_tester(
                        email,
                        first_name.as_deref(),
                        last_name.as_deref(),
                        &[group_id],
                    )
                    .await?;

                if ui.is_json() {
                    ui.json(&tester)?;
                } else if ui.is_text() {
                    ui.success(&format!(
                        "Invited {} to group '{}'",
                        style(email).cyan(),
                        group
                    ));
                }
            }

            TestersSubcommand::Remove { email, bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let testers = testflight.list_testers(&app_id, None).await?;

                let tester = testers
                    .iter()
                    .find(|t| t.email.eq_ignore_ascii_case(email))
                    .ok_or_else(|| anyhow::anyhow!("Tester not found: {}", email))?;

                testflight.remove_tester(&tester.id).await?;

                ui.success(&format!("Removed tester {}", style(email).cyan()));
            }
        }

        Ok(())
    }

    async fn groups(&self, cmd: &GroupsCommand, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let mut testflight = TestFlight::from_env()?;

        match &cmd.subcommand {
            GroupsSubcommand::List { bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                if ui.is_json() {
                    ui.json(&groups)?;
                } else if ui.is_text() {
                    if groups.is_empty() {
                        ui.info("No groups found");
                    } else {
                        ui.header("Beta Groups");
                        ui.blank();
                        for group in &groups {
                            self.print_group(group);
                        }
                    }
                }
            }

            GroupsSubcommand::Create {
                name,
                bundle_id,
                internal,
            } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let group = testflight
                    .create_beta_group(&app_id, name, *internal)
                    .await?;

                if ui.is_json() {
                    ui.json(&group)?;
                } else if ui.is_text() {
                    ui.success(&format!("Created group '{}'", style(name).cyan()));
                    ui.key_value("ID", &style(&group.id).dim().to_string());
                    ui.key_value(
                        "Type",
                        &if group.is_internal {
                            style("Internal").yellow().to_string()
                        } else {
                            style("External").green().to_string()
                        },
                    );
                }
            }

            GroupsSubcommand::Delete { name, bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                let group = groups
                    .iter()
                    .find(|g| g.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| anyhow::anyhow!("Group not found: {}", name))?;

                testflight.delete_beta_group(&group.id).await?;

                ui.success(&format!("Deleted group '{}'", style(name).cyan()));
            }
        }

        Ok(())
    }

    async fn submit(&self, args: &SubmitArgs, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let mut testflight = TestFlight::from_env()?;

        // Set changelog if provided
        if let Some(ref changelog) = args.changelog {
            testflight
                .set_whats_new(&args.build_id, &args.locale, changelog)
                .await?;
        }

        let submission = testflight.submit_for_beta_review(&args.build_id).await?;

        if ui.is_json() {
            ui.json(&submission)?;
        } else if ui.is_text() {
            ui.success(&format!(
                "Submitted build {} for beta review",
                style(&args.build_id).cyan()
            ));
            ui.key_value(
                "Status",
                &match submission.state {
                    BetaReviewState::WaitingForReview => {
                        style("Waiting for Review").yellow().to_string()
                    }
                    BetaReviewState::InReview => style("In Review").blue().to_string(),
                    BetaReviewState::Approved => style("Approved").green().to_string(),
                    BetaReviewState::Rejected => style("Rejected").red().to_string(),
                },
            );
        }

        Ok(())
    }

    async fn expire(&self, args: &ExpireArgs, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);

        if !args.yes {
            let confirmed = ui.confirm(
                &format!("Are you sure you want to expire build {}?", args.build_id),
                false,
            )?;

            if !confirmed {
                ui.info("Cancelled");
                return Ok(());
            }
        }

        let mut testflight = TestFlight::from_env()?;
        testflight.expire_build(&args.build_id).await?;

        ui.success(&format!("Expired build {}", style(&args.build_id).cyan()));

        Ok(())
    }

    fn get_config(
        &self,
        api_key_id: Option<&str>,
        issuer_id: Option<&str>,
        api_key: Option<&str>,
    ) -> anyhow::Result<AppleStoreConfig> {
        let api_key_id = api_key_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("APP_STORE_CONNECT_API_KEY_ID").ok())
            .ok_or_else(|| anyhow::anyhow!("APP_STORE_CONNECT_API_KEY_ID not set"))?;

        let api_issuer_id = issuer_id
            .map(|s| s.to_string())
            .or_else(|| std::env::var("APP_STORE_CONNECT_ISSUER_ID").ok())
            .ok_or_else(|| anyhow::anyhow!("APP_STORE_CONNECT_ISSUER_ID not set"))?;

        let api_key = api_key
            .map(|s| s.to_string())
            .or_else(|| std::env::var("APP_STORE_CONNECT_API_KEY_PATH").ok())
            .or_else(|| std::env::var("APP_STORE_CONNECT_API_KEY").ok())
            .ok_or_else(|| anyhow::anyhow!("APP_STORE_CONNECT_API_KEY not set"))?;

        Ok(AppleStoreConfig {
            api_key_id,
            api_issuer_id,
            api_key,
            team_id: std::env::var("APP_STORE_CONNECT_TEAM_ID").ok(),
            app_id: None,
            notarize: false,
            staple: false,
            primary_locale: None,
        })
    }

    fn print_build(&self, build: &TestFlightBuild) {
        println!(
            "  {} {}",
            style("Build").bold(),
            style(&build.version).cyan()
        );
        println!("    ID: {}", style(&build.id).dim());
        println!(
            "    Status: {}",
            match build.processing_state {
                BuildProcessingState::Processing => style("Processing").yellow(),
                BuildProcessingState::Valid => style("Valid").green(),
                BuildProcessingState::Invalid => style("Invalid").red(),
                BuildProcessingState::Failed => style("Failed").red(),
            }
        );
        if let Some(ref uploaded) = build.uploaded_at {
            println!(
                "    Uploaded: {}",
                style(uploaded.format("%Y-%m-%d %H:%M UTC")).dim()
            );
        }
        if build.expired {
            println!("    {}", style("EXPIRED").red().bold());
        } else if let Some(ref expires) = build.expires_at {
            println!("    Expires: {}", style(expires.format("%Y-%m-%d")).dim());
        }
    }

    fn print_tester(&self, tester: &BetaTester) {
        let name = match (&tester.first_name, &tester.last_name) {
            (Some(f), Some(l)) => format!("{} {}", f, l),
            (Some(f), None) => f.clone(),
            (None, Some(l)) => l.clone(),
            (None, None) => "(no name)".to_string(),
        };
        println!("  {} - {}", style(&tester.email).cyan(), style(name).dim());
    }

    fn print_group(&self, group: &BetaGroup) {
        println!(
            "  {} {}",
            if group.is_internal {
                style("[Internal]").yellow()
            } else {
                style("[External]").green()
            },
            style(&group.name).bold()
        );
        println!("    ID: {}", style(&group.id).dim());
        if group.public_link_enabled {
            if let Some(ref link) = group.public_link {
                println!("    Public Link: {}", style(link).dim());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_locale() {
        let args = UploadArgs {
            ipa: PathBuf::from("test.ipa"),
            api_key_id: None,
            issuer_id: None,
            api_key: None,
            skip_distribution: false,
            changelog: None,
            locale: "en-US".to_string(),
            dry_run: false,
        };
        assert_eq!(args.locale, "en-US");
    }
}
