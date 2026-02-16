//! Version command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_core::types::ReleaseType;
use canaveral_git::GitRepo;
use canaveral_changelog::{CommitParser, ConventionalParser};
use canaveral_strategies::{BumpType, SemVerStrategy, VersionStrategy};

use crate::cli::{Cli, OutputFormat};

/// Calculate the next version
#[derive(Debug, Args)]
pub struct VersionCommand {
    /// Force a specific release type
    #[arg(short, long)]
    pub release_type: Option<ReleaseType>,

    /// Show current version only
    #[arg(long)]
    pub current: bool,

    /// Package name (for monorepos)
    #[arg(short, long)]
    pub package: Option<String>,
}

impl VersionCommand {
    /// Execute the version command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(release_type = ?self.release_type, current = self.current, package = ?self.package, "executing version command");
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let repo = GitRepo::discover(&cwd)?;

        // Find the latest tag
        let latest_tag = repo.find_latest_tag(None)?;
        let current_version = latest_tag
            .as_ref()
            .and_then(|t| t.version.clone())
            .unwrap_or_else(|| "0.0.0".to_string());

        if self.current {
            self.output_current(&current_version, cli)?;
            return Ok(());
        }

        // Get commits since last tag
        let commits = if let Some(tag) = &latest_tag {
            repo.commits_since_tag(&tag.name)?
        } else {
            repo.all_commits()?
        };

        // Determine bump type from commits
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

        // Calculate next version
        let strategy = SemVerStrategy::new();
        let current = strategy.parse(&current_version)?;
        let next = strategy.bump(&current, bump_type)?;
        let next_version = strategy.format(&next);

        self.output_result(&current_version, &next_version, bump_type, commits.len(), &config, cli)?;

        Ok(())
    }

    fn output_current(&self, version: &str, cli: &Cli) -> anyhow::Result<()> {
        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "current": version
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!("{}", version);
            }
        }
        Ok(())
    }

    fn output_result(
        &self,
        current: &str,
        next: &str,
        bump_type: BumpType,
        commit_count: usize,
        _config: &canaveral_core::config::Config,
        cli: &Cli,
    ) -> anyhow::Result<()> {
        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "current": current,
                    "next": next,
                    "bump_type": bump_type.to_string(),
                    "commits": commit_count
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                if !cli.quiet {
                    println!("{}", style("Version Calculation").bold());
                    println!();
                    println!("  Current version:  {}", style(current).cyan());
                    println!("  Next version:     {}", style(next).green().bold());
                    println!("  Bump type:        {}", style(bump_type.to_string()).yellow());
                    println!("  Commits analyzed: {}", commit_count);
                } else {
                    println!("{}", next);
                }
            }
        }
        Ok(())
    }
}
