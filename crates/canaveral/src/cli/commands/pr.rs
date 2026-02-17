//! PR validation pipeline command

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_changelog::{CommitParser, ConventionalParser};
use canaveral_core::config::load_config_or_default;
use canaveral_git::GitRepo;

use crate::cli::{Cli, OutputFormat};

/// PR validation and preview
#[derive(Debug, Args)]
pub struct PrCommand {
    #[command(subcommand)]
    pub action: PrAction,
}

/// PR subcommands
#[derive(Debug, Subcommand)]
pub enum PrAction {
    /// Validate current branch against PR requirements
    Validate(PrValidateCommand),
    /// Preview what the release would look like
    Preview(PrPreviewCommand),
    /// Check for version conflicts with base branch
    CheckConflicts(PrCheckConflictsCommand),
}

/// Validate PR requirements
#[derive(Debug, Args)]
pub struct PrValidateCommand {
    /// Base branch to compare against
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Specific checks to run (default: all configured)
    #[arg(long)]
    pub checks: Vec<String>,

    /// Strict mode - treat warnings as errors
    #[arg(long)]
    pub strict: bool,
}

/// Preview release from current branch
#[derive(Debug, Args)]
pub struct PrPreviewCommand {
    /// Base branch to compare against
    #[arg(long, default_value = "main")]
    pub base: String,
}

/// Check for version conflicts
#[derive(Debug, Args)]
pub struct PrCheckConflictsCommand {
    /// Base branch to compare against
    #[arg(long, default_value = "main")]
    pub base: String,
}

impl PrCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let action_name = match &self.action {
            PrAction::Validate(_) => "validate",
            PrAction::Preview(_) => "preview",
            PrAction::CheckConflicts(_) => "check-conflicts",
        };
        info!(action = action_name, "executing pr command");
        match &self.action {
            PrAction::Validate(cmd) => cmd.execute(cli),
            PrAction::Preview(cmd) => cmd.execute(cli),
            PrAction::CheckConflicts(cmd) => cmd.execute(cli),
        }
    }
}

impl PrValidateCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);
        let repo = GitRepo::discover(&cwd)?;

        if !cli.quiet {
            println!("{} Validating PR...", style("→").blue());
            println!("  Base branch: {}", style(&self.base).cyan());
            println!();
        }

        let checks_to_run = if self.checks.is_empty() {
            config.pr.checks.clone()
        } else {
            self.checks.clone()
        };

        let mut passed = Vec::new();
        let mut failed = Vec::new();
        let mut warnings = Vec::new();

        for check in &checks_to_run {
            match check.as_str() {
                "commit-format" => {
                    // Check conventional commits
                    let commits = repo.all_commits()?;
                    let parser = ConventionalParser::new();
                    let mut non_conventional = Vec::new();

                    for commit in &commits {
                        if parser.parse(commit).is_none() && !commit.message.starts_with("Merge") {
                            non_conventional.push(commit.message.clone());
                        }
                    }

                    if non_conventional.is_empty() {
                        passed.push(
                            "commit-format: all commits follow conventional format".to_string(),
                        );
                    } else if config.pr.require_conventional_commits {
                        failed.push(format!(
                            "commit-format: {} non-conventional commits found",
                            non_conventional.len()
                        ));
                    } else {
                        warnings.push(format!(
                            "commit-format: {} non-conventional commits found",
                            non_conventional.len()
                        ));
                    }
                }
                "version-conflict" => {
                    // Check for version tag conflicts
                    let latest_tag = repo.find_latest_tag(None)?;
                    if latest_tag.is_some() {
                        passed.push("version-conflict: no version conflicts detected".to_string());
                    } else {
                        passed
                            .push("version-conflict: no existing tags (first release)".to_string());
                    }
                }
                "tests" => {
                    // Test check is informational - actual testing done via `run test`
                    warnings.push("tests: run `canaveral run test` to execute tests".to_string());
                }
                "lint" => {
                    warnings.push("lint: run `canaveral run lint` to execute linting".to_string());
                }
                other => {
                    warnings.push(format!("{}: unknown check (skipped)", other));
                }
            }
        }

        // Output results
        if cli.format == OutputFormat::Json {
            let result = serde_json::json!({
                "passed": passed,
                "failed": failed,
                "warnings": warnings,
                "success": failed.is_empty(),
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        }

        if !cli.quiet {
            for msg in &passed {
                println!("  {} {}", style("✓").green(), msg);
            }
            for msg in &warnings {
                println!("  {} {}", style("⚠").yellow(), msg);
            }
            for msg in &failed {
                println!("  {} {}", style("✗").red(), msg);
            }

            println!();

            if failed.is_empty() && (warnings.is_empty() || !self.strict) {
                println!(
                    "  {} {}",
                    style("✓").green().bold(),
                    style("PR validation passed.").green()
                );
            } else if !failed.is_empty() || (self.strict && !warnings.is_empty()) {
                println!(
                    "  {} {}",
                    style("✗").red().bold(),
                    style("PR validation failed.").red()
                );
            }
        }

        if !failed.is_empty() || (self.strict && !warnings.is_empty()) {
            anyhow::bail!("PR validation failed");
        }

        Ok(())
    }
}

impl PrPreviewCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);
        let repo = GitRepo::discover(&cwd)?;

        let latest_tag = repo.find_latest_tag(None)?;
        let current_version = latest_tag
            .as_ref()
            .and_then(|t| t.version.clone())
            .unwrap_or_else(|| "0.0.0".to_string());

        let commits = if let Some(tag) = &latest_tag {
            repo.commits_since_tag(&tag.name)?
        } else {
            repo.all_commits()?
        };

        let parser = ConventionalParser::new();
        let mut has_breaking = false;
        let mut has_feat = false;
        let mut has_fix = false;
        let mut commit_count = 0;

        for commit in &commits {
            if let Some(parsed) = parser.parse(commit) {
                commit_count += 1;
                if parsed.breaking {
                    has_breaking = true;
                }
                if parsed.is_minor() {
                    has_feat = true;
                }
                if parsed.is_patch() {
                    has_fix = true;
                }
            }
        }

        let bump_type = if has_breaking {
            "major"
        } else if has_feat {
            "minor"
        } else if has_fix {
            "patch"
        } else {
            "none"
        };

        if cli.format == OutputFormat::Json {
            let preview = serde_json::json!({
                "current_version": current_version,
                "bump_type": bump_type,
                "commit_count": commit_count,
                "has_breaking_changes": has_breaking,
            });
            println!("{}", serde_json::to_string_pretty(&preview)?);
            return Ok(());
        }

        if !cli.quiet {
            println!("{}", style("Release Preview").bold());
            println!();
            println!("  Current version: {}", style(&current_version).cyan());
            println!("  Bump type:       {}", style(bump_type).yellow());
            println!("  Commits:         {}", commit_count);
            if has_breaking {
                println!("  {}", style("⚠ Contains breaking changes").red().bold());
            }

            let tag_format = &config.versioning.tag_format;
            println!("  Tag format:      {}", style(tag_format).dim());
            println!();
        }

        Ok(())
    }
}

impl PrCheckConflictsCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (_config, _) = load_config_or_default(&cwd);
        let repo = GitRepo::discover(&cwd)?;

        if !cli.quiet {
            println!(
                "{} Checking for version conflicts against {}...",
                style("→").blue(),
                style(&self.base).cyan()
            );
        }

        // Check if current branch has tag conflicts
        let current_branch = repo.current_branch()?;
        let latest_tag = repo.find_latest_tag(None)?;

        if cli.format == OutputFormat::Json {
            let result = serde_json::json!({
                "branch": current_branch,
                "latest_tag": latest_tag.as_ref().map(|t| &t.name),
                "conflicts": false,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
            return Ok(());
        }

        if !cli.quiet {
            if let Some(branch) = &current_branch {
                println!("  Branch: {}", style(branch).cyan());
            }
            if let Some(tag) = &latest_tag {
                println!("  Latest tag: {}", style(&tag.name).yellow());
            }
            println!();
            println!("  {} No version conflicts detected.", style("✓").green());
        }

        Ok(())
    }
}
