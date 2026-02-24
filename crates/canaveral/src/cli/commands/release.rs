//! Release command

use clap::Args;
use console::style;
use std::process::Command;
use tracing::info;

use canaveral_adapters::AdapterRegistry;
use canaveral_changelog::ChangelogGenerator;
use canaveral_changelog::{CommitParser, ConventionalParser};
use canaveral_core::config::load_config_or_default;
use canaveral_core::types::ReleaseType;
use canaveral_core::workflow::{format_tag, ReleaseOptions, ReleaseWorkflow};
use canaveral_git::GitRepo;
use canaveral_strategies::{BumpType, SemVerStrategy, VersionStrategy};

use crate::cli::output::Ui;
use crate::cli::Cli;

/// Create a new release
#[derive(Debug, Args)]
pub struct ReleaseCommand {
    /// Release type (major, minor, patch)
    #[arg(short, long)]
    pub release_type: Option<ReleaseType>,

    /// Explicit version to release
    #[arg(long = "as-version", value_name = "VERSION")]
    pub as_version: Option<String>,

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
            version = ?self.as_version,
            dry_run = self.dry_run,
            no_changelog = self.no_changelog,
            no_publish = self.no_publish,
            no_git = self.no_git,
            package = ?self.package,
            "executing release command"
        );
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let (config, config_path) = load_config_or_default(&cwd);
        let adapter_registry = AdapterRegistry::new();
        let adapter = adapter_registry.detect(&cwd);

        if config_path.is_none() {
            ui.warning(&format!(
                "No configuration found, using defaults. Run {} to create one.",
                style("canaveral init").cyan()
            ));
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
        let tag_version = latest_tag
            .as_ref()
            .and_then(|t| t.version.clone())
            .unwrap_or_else(|| "0.0.0".to_string());
        let current_version = adapter
            .as_ref()
            .and_then(|a| a.get_version(&cwd).ok())
            .unwrap_or(tag_version);

        // Determine next version
        let next_version = if let Some(v) = &self.as_version {
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
                ui.warning("No version bump required - no relevant commits found.");
                return Ok(());
            }

            let strategy = SemVerStrategy::new();
            let current = strategy.parse(&current_version)?;
            let next = strategy.bump(&current, bump_type)?;
            strategy.format(&next)
        };

        let tag = format_tag(&config, &next_version, self.package.as_deref());

        // Show release preview
        ui.header("Release Preview");
        ui.blank();
        ui.key_value("Current version", &ui.fmt_path(&current_version));
        ui.key_value("Next version", &ui.fmt_version(&next_version));
        ui.key_value("Tag", &ui.fmt_tag(&tag));
        ui.blank();

        if self.dry_run {
            ui.warning("[DRY RUN - no changes will be made]");
            ui.blank();
        }

        // Confirm release
        if !self.yes && !self.dry_run {
            let confirmed = ui.confirm("Proceed with release?", true)?;
            if !confirmed {
                ui.warning("Aborted.");
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
        let mut result = workflow.execute()?;
        result.previous_version = Some(current_version.clone());
        result.new_version = next_version.clone();
        result.tag = tag.clone();

        // Update package version via detected adapter
        if let Some(adapter) = &adapter {
            if !self.dry_run {
                adapter.set_version(&cwd, &next_version)?;
                ui.success(&format!(
                    "Updated {} version to {}",
                    style(adapter.name()).cyan(),
                    ui.fmt_version(&next_version)
                ));
            } else {
                ui.info(&format!(
                    "Would update {} version to {}",
                    style(adapter.name()).cyan(),
                    ui.fmt_version(&next_version)
                ));
            }
        }

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

                ui.success(&format!(
                    "Updated changelog at {}",
                    ui.fmt_path(&config.changelog.file.display())
                ));
            }
        }

        // Publish package using detected adapter
        let mut published = false;
        if !self.no_publish {
            if let Some(adapter) = &adapter {
                let validation = adapter.validate_publishable(&cwd)?;
                if !validation.passed {
                    anyhow::bail!(
                        "Publish validation failed:\n{}",
                        validation.errors.join("\n")
                    );
                }

                for warning in &validation.warnings {
                    ui.warning(warning);
                }

                if !self.dry_run {
                    adapter.publish(&cwd, false)?;
                    published = true;
                    ui.success(&format!(
                        "Published package via {}",
                        style(adapter.name()).cyan()
                    ));
                } else {
                    ui.info(&format!(
                        "Would publish package via {}",
                        style(adapter.name()).cyan()
                    ));
                }
            } else {
                ui.warning(&format!(
                    "No publish adapter detected in {}, skipping publish.",
                    style(cwd.display()).dim()
                ));
            }
        }
        result.published = published;

        // Git operations
        if !self.no_git && !self.dry_run {
            if !repo.is_clean()? {
                let commit_message = config
                    .git
                    .commit_message
                    .replace("{version}", &next_version);
                let add_output = Command::new("git")
                    .args(["add", "-A"])
                    .current_dir(&cwd)
                    .output()?;
                if !add_output.status.success() {
                    anyhow::bail!(
                        "Failed to stage release changes: {}",
                        String::from_utf8_lossy(&add_output.stderr)
                    );
                }

                let commit_output = Command::new("git")
                    .args(["commit", "-m", &commit_message])
                    .current_dir(&cwd)
                    .output()?;
                if !commit_output.status.success() {
                    anyhow::bail!(
                        "Failed to commit release changes: {}",
                        String::from_utf8_lossy(&commit_output.stderr)
                    );
                }

                ui.success("Committed release changes");
            }

            // Create tag
            repo.create_tag(&tag, Some(&format!("Release {}", next_version)))?;
            ui.success(&format!("Created tag {}", ui.fmt_tag(&tag)));

            // Push hint
            if config.git.push_tags {
                ui.info(&format!(
                    "To push, run: {}",
                    style(format!("git push {} {}", config.git.remote, tag)).cyan()
                ));
            }
        }

        // Final output
        if ui.is_json() {
            ui.json(&result)?;
        } else {
            ui.blank();
            if self.dry_run {
                ui.success(&format!(
                    "Dry run complete. Version {} would be released.",
                    ui.fmt_version(&next_version)
                ));
            } else {
                ui.success(&format!(
                    "Released version {}",
                    ui.fmt_version(&next_version)
                ));
            }
        }

        Ok(())
    }
}
