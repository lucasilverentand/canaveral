//! Init command

use std::path::PathBuf;

use clap::Args;
use console::style;
use dialoguer::{Confirm, Select};
use tracing::info;

use canaveral_core::config::defaults::{DEFAULT_CONFIG_YAML, DEFAULT_CONFIG_TEMPLATE};

use crate::cli::Cli;

/// Initialize a new Canaveral configuration
#[derive(Debug, Args)]
pub struct InitCommand {
    /// Force overwrite existing configuration
    #[arg(short, long)]
    pub force: bool,

    /// Use defaults without prompting
    #[arg(short = 'y', long)]
    pub yes: bool,

    /// Output file path
    #[arg(short, long)]
    pub output: Option<PathBuf>,
}

impl InitCommand {
    /// Execute the init command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(force = self.force, yes = self.yes, "executing init command");
        let cwd = std::env::current_dir()?;
        let config_path = self
            .output
            .clone()
            .unwrap_or_else(|| cwd.join(DEFAULT_CONFIG_YAML));

        // Check if config already exists
        if config_path.exists() && !self.force {
            if self.yes {
                anyhow::bail!(
                    "Configuration file already exists at {}. Use --force to overwrite.",
                    config_path.display()
                );
            }

            let overwrite = Confirm::new()
                .with_prompt(format!(
                    "Configuration file already exists at {}. Overwrite?",
                    config_path.display()
                ))
                .default(false)
                .interact()?;

            if !overwrite {
                println!("{}", style("Aborted.").yellow());
                return Ok(());
            }
        }

        // Choose format if not specified
        let format = if self.yes {
            "yaml"
        } else {
            let formats = vec!["yaml", "toml"];
            let selection = Select::new()
                .with_prompt("Configuration format")
                .items(&formats)
                .default(0)
                .interact()?;
            formats[selection]
        };

        // Adjust path for format
        let config_path = if format == "toml" && config_path.extension().is_some_and(|e| e == "yaml") {
            config_path.with_extension("toml")
        } else {
            config_path
        };

        // Generate config
        let content = if format == "toml" {
            // Convert YAML to TOML
            let config: canaveral_core::config::Config =
                serde_yaml::from_str(DEFAULT_CONFIG_TEMPLATE)?;
            toml::to_string_pretty(&config)?
        } else {
            DEFAULT_CONFIG_TEMPLATE.to_string()
        };

        // Write config
        std::fs::write(&config_path, &content)?;

        // Auto-install git hooks if configured
        let parsed_config: canaveral_core::config::Config = if format == "toml" {
            toml::from_str(&content)?
        } else {
            serde_yaml::from_str(&content)?
        };
        if parsed_config.git_hooks.auto_install {
            if let Ok(repo_root) = find_git_root(&cwd) {
                match canaveral_git::hooks::install_all(&repo_root) {
                    Ok(()) => {
                        if !cli.quiet {
                            println!(
                                "{} Installed git hooks (commit-msg, pre-commit, pre-push)",
                                style("✓").green().bold()
                            );
                        }
                    }
                    Err(e) => {
                        if !cli.quiet {
                            println!(
                                "{} Could not install git hooks: {e}",
                                style("⚠").yellow().bold()
                            );
                        }
                    }
                }
            }
        }

        if !cli.quiet {
            println!(
                "{} Created configuration at {}",
                style("✓").green().bold(),
                style(config_path.display()).cyan()
            );
            println!();
            println!("Next steps:");
            println!("  1. Edit {} to customize your release workflow", config_path.display());
            println!("  2. Run {} to verify your setup", style("canaveral validate").cyan());
            println!("  3. Run {} to create your first release", style("canaveral release").cyan());
        }

        Ok(())
    }
}

fn find_git_root(start: &std::path::Path) -> anyhow::Result<PathBuf> {
    let mut path = start;
    loop {
        if path.join(".git").exists() {
            return Ok(path.to_path_buf());
        }
        path = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Not inside a git repository"))?;
    }
}
