//! Release command

use clap::Args;
use console::style;
use dialoguer::Confirm;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_core::types::ReleaseType;
use canaveral_core::workflow::{format_tag, ReleaseOptions, ReleaseWorkflow};
use canaveral_changelog::ChangelogGenerator;
use canaveral_git::GitRepo;
use canaveral_strategies::{BumpType, SemVerStrategy, VersionStrategy};
use canaveral_changelog::{CommitParser, ConventionalParser};

use crate::cli::{Cli, OutputFormat};

/// Create a new release
#[derive(Debug, Args)]
pub struct ReleaseCommand {
    /// Release type (major, minor, patch)
    #[arg(short, long)]
    pub release_type: Option<ReleaseType>,

    /// Explicit version to release
    #[arg(long)]
    pub version: Option<String>,

    /// Dry run - don't make any changes
    #[arg(long)]
    pub dry_run: bool,

    /// Skip changelog generation
    #[arg(long)]
    pub no_changelog: bool,

    /// Skip publishing to registry
    #[arg(long)]
    pub no_publish: bool,

    /// Skip git operations (commit, tag, push)
    #[arg(long)]
    pub no_git: bool,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Allow release from non-release branch
    #[arg(long)]
    pub allow_branch: bool,

    /// Package to release (for monorepos)
    #[arg(short, long)]
    pub package: Option<String>,
}

impl ReleaseCommand {
    /// Execute the release command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(
            release_type = ?self.release_type,
            version = ?self.version,
            dry_run = self.dry_run,
            no_changelog = self.no_changelog,
            no_publish = self.no_publish,
            no_git = self.no_git,
            package = ?self.package,
            "executing release command"
        );
        let cwd = std::env::current_dir()?;
        let (config, config_path) = load_config_or_default(&cwd);

        if config_path.is_none() && !cli.quiet {
            println!(
                "{} No configuration found, using defaults. Run {} to create one.",
                style("!").yellow().bold(),
                style("canaveral init").cyan()
            );
        }

        let repo = GitRepo::discover(&cwd)?;

        // Check repository state
        if config.git.require_clean && !repo.is_clean()? && !self.dry_run {
            anyhow::bail!("Working directory has uncommitted changes. Commit or stash them first.");
        }

        // Check branch
        let current_branch = repo.current_branch()?;
        if let Some(branch) = &current_branch {
            if branch != &config.git.branch && !self.allow_branch && !self.dry_run {
                anyhow::bail!(
                    "Not on release branch '{}'. Current branch: '{}'. Use --allow-branch to override.",
                    config.git.branch,
                    branch
                );
            }
        }

        // Find current version
        let latest_tag = repo.find_latest_tag(None)?;
        let current_version = latest_tag
            .as_ref()
            .and_then(|t| t.version.clone())
            .unwrap_or_else(|| "0.0.0".to_string());

        // Determine next version
        let next_version = if let Some(v) = &self.version {
            v.clone()
        } else {
            // Get commits and determine bump type
            let commits = if let Some(tag) = &latest_tag {
                repo.commits_since_tag(&tag.name)?
            } else {
                repo.all_commits()?
            };

            let parser = ConventionalParser::new();
            let mut bump_type = BumpType::None;

            for commit in &commits {
                if let Some(parsed) = parser.parse(commit) {
                    let commit_bump = if parsed.breaking {
                        BumpType::Major
                    } else if parsed.is_minor() {
                        BumpType::Minor
                    } else if parsed.is_patch() {
                        BumpType::Patch
                    } else {
                        BumpType::None
                    };
                    bump_type = bump_type.max(commit_bump);
                }
            }

            // Override with explicit release type
            if let Some(rt) = self.release_type {
                bump_type = match rt {
                    ReleaseType::Major => BumpType::Major,
                    ReleaseType::Minor => BumpType::Minor,
                    ReleaseType::Patch => BumpType::Patch,
                    ReleaseType::Prerelease => BumpType::Prerelease,
                    ReleaseType::Custom => bump_type,
                };
            }

            if bump_type == BumpType::None {
                if !cli.quiet {
                    println!("{}", style("No version bump required - no relevant commits found.").yellow());
                }
                return Ok(());
            }

            let strategy = SemVerStrategy::new();
            let current = strategy.parse(&current_version)?;
            let next = strategy.bump(&current, bump_type)?;
            strategy.format(&next)
        };

        let tag = format_tag(&config, &next_version, self.package.as_deref());

        // Show release preview
        if !cli.quiet {
            println!("{}", style("Release Preview").bold());
            println!();
            println!("  Current version: {}", style(&current_version).cyan());
            println!("  Next version:    {}", style(&next_version).green().bold());
            println!("  Tag:             {}", style(&tag).yellow());
            println!();

            if self.dry_run {
                println!("  {}", style("[DRY RUN - no changes will be made]").yellow().bold());
                println!();
            }
        }

        // Confirm release
        if !self.yes && !self.dry_run {
            let confirmed = Confirm::new()
                .with_prompt("Proceed with release?")
                .default(true)
                .interact()?;

            if !confirmed {
                println!("{}", style("Aborted.").yellow());
                return Ok(());
            }
        }

        // Execute release workflow
        let options = ReleaseOptions {
            release_type: self.release_type,
            version: Some(next_version.clone()),
            prerelease: None,
            dry_run: self.dry_run,
            skip_changelog: self.no_changelog,
            skip_publish: self.no_publish,
            skip_git: self.no_git,
            allow_branch: self.allow_branch,
            package: self.package.clone(),
        };

        let workflow = ReleaseWorkflow::new(&config, options);
        let result = workflow.execute()?;

        // Generate changelog if not skipped
        if !self.no_changelog && config.changelog.enabled {
            let commits = if let Some(tag_info) = &latest_tag {
                repo.commits_since_tag(&tag_info.name)?
            } else {
                repo.all_commits()?
            };

            let generator = ChangelogGenerator::new(config.changelog.clone());
            let changelog = generator.generate_formatted(&next_version, &commits);

            if !self.dry_run {
                let changelog_path = cwd.join(&config.changelog.file);
                if changelog_path.exists() {
                    let existing = std::fs::read_to_string(&changelog_path)?;
                    let combined = format!("{}\n{}", changelog, existing);
                    std::fs::write(&changelog_path, combined)?;
                } else {
                    std::fs::write(&changelog_path, &changelog)?;
                }

                if !cli.quiet {
                    println!(
                        "{} Updated changelog at {}",
                        style("✓").green(),
                        style(config.changelog.file.display()).cyan()
                    );
                }
            }
        }

        // Git operations
        if !self.no_git && !self.dry_run {
            // Create tag
            repo.create_tag(&tag, Some(&format!("Release {}", next_version)))?;

            if !cli.quiet {
                println!("{} Created tag {}", style("✓").green(), style(&tag).yellow());
            }

            // Note: Push is typically done via git CLI for proper auth handling
            if config.git.push_tags {
                if !cli.quiet {
                    println!(
                        "{} To push, run: {}",
                        style("→").blue(),
                        style(format!("git push {} {}", config.git.remote, tag)).cyan()
                    );
                }
            }
        }

        // Output result
        match cli.format {
            OutputFormat::Json => {
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
            OutputFormat::Text => {
                if !cli.quiet {
                    println!();
                    if self.dry_run {
                        println!(
                            "{} Dry run complete. Version {} would be released.",
                            style("✓").green().bold(),
                            style(&next_version).green().bold()
                        );
                    } else {
                        println!(
                            "{} Released version {}",
                            style("✓").green().bold(),
                            style(&next_version).green().bold()
                        );
                    }
                }
            }
        }

        Ok(())
    }
}
