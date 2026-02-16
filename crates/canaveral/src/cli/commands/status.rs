//! Status command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_git::GitRepo;
use canaveral_adapters::detect_packages;

use crate::cli::{Cli, OutputFormat};

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
        let cwd = std::env::current_dir()?;
        let (config, config_path) = load_config_or_default(&cwd);

        let repo = GitRepo::discover(&cwd)?;

        // Gather status information
        let is_clean = repo.is_clean()?;
        let current_branch = repo.current_branch()?;
        let latest_tag = repo.find_latest_tag(None)?;
        let packages = detect_packages(&cwd)?;

        let commits_since = if let Some(tag) = &latest_tag {
            repo.commits_since_tag(&tag.name)
                .map(|c| c.len())
                .unwrap_or(0)
        } else {
            repo.all_commits().map(|c| c.len()).unwrap_or(0)
        };

        // Output
        match cli.format {
            OutputFormat::Json => {
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
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!("{}", style("Canaveral Status").bold());
                println!();

                // Configuration
                println!("{}", style("Configuration").underlined());
                if let Some(path) = config_path {
                    println!("  Config file: {}", style(path.display()).cyan());
                } else {
                    println!(
                        "  Config file: {} (using defaults)",
                        style("not found").yellow()
                    );
                }
                println!("  Strategy:    {}", config.versioning.strategy);
                println!();

                // Git status
                println!("{}", style("Git").underlined());
                if let Some(branch) = current_branch {
                    let branch_status = if branch == config.git.branch {
                        style(&branch).green()
                    } else {
                        style(&branch).yellow()
                    };
                    println!("  Branch:      {}", branch_status);
                }

                let clean_status = if is_clean {
                    style("clean").green()
                } else {
                    style("dirty").red()
                };
                println!("  Status:      {}", clean_status);

                if let Some(tag) = &latest_tag {
                    println!("  Latest tag:  {}", style(&tag.name).cyan());
                    if let Some(v) = &tag.version {
                        println!("  Version:     {}", style(v).green().bold());
                    }
                } else {
                    println!("  Latest tag:  {}", style("none").dim());
                }

                println!("  Commits since: {}", commits_since);
                println!();

                // Packages
                if !packages.is_empty() {
                    println!("{}", style("Packages").underlined());
                    for pkg in &packages {
                        println!(
                            "  {} {} ({})",
                            style(&pkg.name).cyan(),
                            style(&pkg.version).green(),
                            pkg.package_type
                        );
                    }
                    println!();
                }

                // Readiness
                println!("{}", style("Release Readiness").underlined());
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
                    println!("  {}", style("✓ Ready to release").green().bold());
                } else {
                    for issue in issues {
                        println!("  {} {}", style("✗").red(), issue);
                    }
                }
            }
        }

        Ok(())
    }
}
