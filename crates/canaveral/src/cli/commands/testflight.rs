//! TestFlight command - Manage TestFlight beta testing

use std::path::PathBuf;

use clap::{Args, Subcommand};
use console::style;

use canaveral_stores::apple::{
    TestFlight, TestFlightBuild, BetaGroup, BetaTester,
    BuildProcessingState, BetaReviewState,
};
use canaveral_stores::types::AppleStoreConfig;

use crate::cli::{Cli, OutputFormat};

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

        if !args.ipa.exists() {
            anyhow::bail!("IPA file not found: {}", args.ipa.display());
        }

        let config = self.get_config(
            args.api_key_id.as_deref(),
            args.issuer_id.as_deref(),
            args.api_key.as_deref(),
        )?;

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Uploading to TestFlight...").bold());
            println!("  File: {}", style(args.ipa.display()).cyan());
            println!();
        }

        let store = AppStoreConnect::new(config)?;

        let options = UploadOptions {
            track: Some("testflight".to_string()),
            dry_run: false,
            verbose: cli.verbose,
            ..Default::default()
        };

        let result = store.upload(&args.ipa, &options).await?;

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else if !cli.quiet {
            println!(
                "{} Upload completed!",
                style("✓").green().bold()
            );
            if let Some(ref build_id) = result.build_id {
                println!("  Build ID: {}", style(build_id).cyan());
            }
            println!("  Status: {}", style(result.status.to_string()).yellow());
            if let Some(ref url) = result.console_url {
                println!("  Console: {}", style(url).dim());
            }
        }

        Ok(())
    }

    async fn status(&self, args: &StatusArgs, cli: &Cli) -> anyhow::Result<()> {
        let mut testflight = TestFlight::from_env()?;

        let build = if let Some(ref build_id) = args.build_id {
            testflight.get_build(build_id).await?
        } else if let Some(ref bundle_id) = args.bundle_id {
            let app_id = testflight.get_app_id(bundle_id).await?;
            let builds = testflight.list_builds(&app_id, Some(1)).await?;
            builds.into_iter().next()
                .ok_or_else(|| anyhow::anyhow!("No builds found for {}", bundle_id))?
        } else {
            anyhow::bail!("Either build_id or bundle_id is required");
        };

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&build)?);
        } else if !cli.quiet {
            self.print_build(&build);
        }

        Ok(())
    }

    async fn builds(&self, args: &BuildsArgs, cli: &Cli) -> anyhow::Result<()> {
        let mut testflight = TestFlight::from_env()?;

        let app_id = testflight.get_app_id(&args.bundle_id).await?;
        let builds = testflight.list_builds(&app_id, Some(args.limit)).await?;

        let builds: Vec<_> = if args.processing {
            builds.into_iter()
                .filter(|b| b.processing_state == BuildProcessingState::Processing)
                .collect()
        } else {
            builds
        };

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&builds)?);
        } else if !cli.quiet {
            if builds.is_empty() {
                println!("No builds found");
            } else {
                println!("{}", style("TestFlight Builds").bold());
                println!();
                for build in &builds {
                    self.print_build(build);
                    println!();
                }
            }
        }

        Ok(())
    }

    async fn testers(&self, cmd: &TestersCommand, cli: &Cli) -> anyhow::Result<()> {
        let mut testflight = TestFlight::from_env()?;

        match &cmd.subcommand {
            TestersSubcommand::List { bundle_id, group } => {
                let app_id = testflight.get_app_id(bundle_id).await?;

                let group_id = if let Some(ref group_name) = group {
                    let groups = testflight.list_beta_groups(&app_id).await?;
                    groups.iter()
                        .find(|g| g.name.eq_ignore_ascii_case(group_name))
                        .map(|g| g.id.clone())
                } else {
                    None
                };

                let testers = testflight.list_testers(&app_id, group_id.as_deref()).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&testers)?);
                } else if !cli.quiet {
                    if testers.is_empty() {
                        println!("No testers found");
                    } else {
                        println!("{}", style("Beta Testers").bold());
                        println!();
                        for tester in &testers {
                            self.print_tester(tester);
                        }
                    }
                }
            }

            TestersSubcommand::Add { email, group, first_name, last_name, bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                let group_id = groups.iter()
                    .find(|g| g.name.eq_ignore_ascii_case(group))
                    .map(|g| g.id.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Group not found: {}", group))?;

                let tester = testflight.invite_tester(
                    email,
                    first_name.as_deref(),
                    last_name.as_deref(),
                    &[group_id],
                ).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&tester)?);
                } else if !cli.quiet {
                    println!(
                        "{} Invited {} to group '{}'",
                        style("✓").green().bold(),
                        style(email).cyan(),
                        group
                    );
                }
            }

            TestersSubcommand::Remove { email, bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let testers = testflight.list_testers(&app_id, None).await?;

                let tester = testers.iter()
                    .find(|t| t.email.eq_ignore_ascii_case(email))
                    .ok_or_else(|| anyhow::anyhow!("Tester not found: {}", email))?;

                testflight.remove_tester(&tester.id).await?;

                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "{} Removed tester {}",
                        style("✓").green().bold(),
                        style(email).cyan()
                    );
                }
            }
        }

        Ok(())
    }

    async fn groups(&self, cmd: &GroupsCommand, cli: &Cli) -> anyhow::Result<()> {
        let mut testflight = TestFlight::from_env()?;

        match &cmd.subcommand {
            GroupsSubcommand::List { bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&groups)?);
                } else if !cli.quiet {
                    if groups.is_empty() {
                        println!("No groups found");
                    } else {
                        println!("{}", style("Beta Groups").bold());
                        println!();
                        for group in &groups {
                            self.print_group(group);
                        }
                    }
                }
            }

            GroupsSubcommand::Create { name, bundle_id, internal } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let group = testflight.create_beta_group(&app_id, name, *internal).await?;

                if cli.format == OutputFormat::Json {
                    println!("{}", serde_json::to_string_pretty(&group)?);
                } else if !cli.quiet {
                    println!(
                        "{} Created group '{}'",
                        style("✓").green().bold(),
                        style(name).cyan()
                    );
                    println!("  ID: {}", style(&group.id).dim());
                    println!(
                        "  Type: {}",
                        if group.is_internal {
                            style("Internal").yellow()
                        } else {
                            style("External").green()
                        }
                    );
                }
            }

            GroupsSubcommand::Delete { name, bundle_id } => {
                let app_id = testflight.get_app_id(bundle_id).await?;
                let groups = testflight.list_beta_groups(&app_id).await?;

                let group = groups.iter()
                    .find(|g| g.name.eq_ignore_ascii_case(name))
                    .ok_or_else(|| anyhow::anyhow!("Group not found: {}", name))?;

                testflight.delete_beta_group(&group.id).await?;

                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "{} Deleted group '{}'",
                        style("✓").green().bold(),
                        style(name).cyan()
                    );
                }
            }
        }

        Ok(())
    }

    async fn submit(&self, args: &SubmitArgs, cli: &Cli) -> anyhow::Result<()> {
        let mut testflight = TestFlight::from_env()?;

        // Set changelog if provided
        if let Some(ref changelog) = args.changelog {
            testflight.set_whats_new(&args.build_id, &args.locale, changelog).await?;
        }

        let submission = testflight.submit_for_beta_review(&args.build_id).await?;

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&submission)?);
        } else if !cli.quiet {
            println!(
                "{} Submitted build {} for beta review",
                style("✓").green().bold(),
                style(&args.build_id).cyan()
            );
            println!(
                "  Status: {}",
                match submission.state {
                    BetaReviewState::WaitingForReview => style("Waiting for Review").yellow(),
                    BetaReviewState::InReview => style("In Review").blue(),
                    BetaReviewState::Approved => style("Approved").green(),
                    BetaReviewState::Rejected => style("Rejected").red(),
                }
            );
        }

        Ok(())
    }

    async fn expire(&self, args: &ExpireArgs, cli: &Cli) -> anyhow::Result<()> {
        if !args.yes && cli.format == OutputFormat::Text {
            use dialoguer::Confirm;

            let confirmed = Confirm::new()
                .with_prompt(format!("Are you sure you want to expire build {}?", args.build_id))
                .default(false)
                .interact()?;

            if !confirmed {
                println!("Cancelled");
                return Ok(());
            }
        }

        let mut testflight = TestFlight::from_env()?;
        testflight.expire_build(&args.build_id).await?;

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!(
                "{} Expired build {}",
                style("✓").green().bold(),
                style(&args.build_id).cyan()
            );
        }

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
            println!("    Uploaded: {}", style(uploaded.format("%Y-%m-%d %H:%M UTC")).dim());
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
        println!(
            "  {} - {}",
            style(&tester.email).cyan(),
            style(name).dim()
        );
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
        };
        assert_eq!(args.locale, "en-US");
    }
}
