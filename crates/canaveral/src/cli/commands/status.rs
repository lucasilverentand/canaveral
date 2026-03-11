//! Status command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::detect_packages_recursive;
use canaveral_core::config::load_config_or_default;
use canaveral_git::GitRepo;

use crate::cli::output::Ui;
use crate::cli::Cli;

/// Show repository status
#[derive(Debug, Args)]
pub struct StatusCommand {
    /// Show verbose status information
    #[arg(short, long)]
    pub verbose: bool,
}

impl StatusCommand {
    /// Execute the status command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!("executing status command");
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let (config, config_path) = load_config_or_default(&cwd);

        let repo = GitRepo::discover(&cwd)?;

        // Gather status information
        let is_clean = repo.is_clean()?;
        let current_branch = repo.current_branch()?;
        let latest_tag = repo.find_latest_tag(None)?;
        let packages = detect_packages_recursive(&cwd, 3)?;

        let commits_since = if let Some(tag) = &latest_tag {
            repo.commits_since_tag(&tag.name)
                .map(|c| c.len())
                .unwrap_or(0)
        } else {
            repo.all_commits().map(|c| c.len()).unwrap_or(0)
        };

        // JSON output
        if ui.is_json() {
            let output = serde_json::json!({
                "config_found": config_path.is_some(),
                "config_path": config_path.map(|p| p.to_string_lossy().to_string()),
                "git": {
                    "clean": is_clean,
                    "branch": current_branch,
                    "latest_tag": latest_tag.as_ref().map(|t| &t.name),
                    "current_version": latest_tag.as_ref().and_then(|t| t.version.clone()),
                    "commits_since_tag": commits_since
                },
                "packages": packages.iter().map(|p| serde_json::json!({
                    "name": p.name,
                    "version": p.version,
                    "type": p.package_type,
                    "path": p.manifest_path.to_string_lossy().to_string()
                })).collect::<Vec<_>>()
            });
            ui.json(&output)?;
            return Ok(());
        }

        // Text output
        ui.header("Canaveral Status");
        ui.blank();

        // Configuration
        ui.section("Configuration");
        if let Some(path) = config_path {
            ui.key_value("Config file", &ui.fmt_path(&path.display()));
        } else {
            ui.key_value_styled("Config file", style("not found").yellow());
            ui.hint("using defaults");
        }
        ui.key_value("Strategy", &config.versioning.strategy.to_string());
        ui.blank();

        // Git status
        ui.section("Git");
        if let Some(branch) = &current_branch {
            let branch_display = if *branch == config.git.branch {
                style(branch).green()
            } else {
                style(branch).yellow()
            };
            ui.key_value_styled("Branch", branch_display);
        }

        let clean_display = if is_clean {
            style("clean").green()
        } else {
            style("dirty").red()
        };
        ui.key_value_styled("Status", clean_display);

        if let Some(tag) = &latest_tag {
            ui.key_value("Latest tag", &ui.fmt_path(&tag.name));
            if let Some(v) = &tag.version {
                ui.key_value("Version", &ui.fmt_version(v));
            }
        } else {
            ui.key_value_styled("Latest tag", style("none").dim());
        }

        ui.key_value("Commits since", &commits_since.to_string());
        ui.blank();

        // Packages (filter out private workspace roots)
        let displayable_packages: Vec<_> = packages.iter().filter(|p| !p.private).collect();
        if !displayable_packages.is_empty() {
            ui.section("Packages");
            for pkg in &displayable_packages {
                ui.step(&format!(
                    "{} {} ({})",
                    style(&pkg.name).cyan(),
                    style(&pkg.version).green(),
                    pkg.package_type
                ));
            }
            ui.blank();
        }

        // Readiness
        ui.section("Release Readiness");
        let mut issues = Vec::new();

        if !is_clean {
            issues.push("Working directory has uncommitted changes");
        }

        if let Some(branch) = repo.current_branch()? {
            if branch != config.git.branch {
                issues.push("Not on release branch");
            }
        }

        if issues.is_empty() {
            ui.success("Ready to release");
        } else {
            for issue in issues {
                ui.error(issue);
            }
        }

        Ok(())
    }
}
