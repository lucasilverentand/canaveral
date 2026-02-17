//! Validate command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::detect_packages;
use canaveral_core::config::{load_config_from_dir, validation::validate_config};
use canaveral_git::GitRepo;

use crate::cli::{Cli, OutputFormat};

/// Validate configuration and repository state
#[derive(Debug, Args)]
pub struct ValidateCommand {
    /// Only validate configuration file
    #[arg(long)]
    pub config_only: bool,

    /// Strict mode - treat warnings as errors
    #[arg(long)]
    pub strict: bool,
}

impl ValidateCommand {
    /// Execute the validate command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(
            config_only = self.config_only,
            strict = self.strict,
            "executing validate command"
        );
        let cwd = std::env::current_dir()?;

        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        // Validate configuration
        let config_result = load_config_from_dir(&cwd);
        let (config, config_path) = match config_result {
            Ok((c, p)) => (Some(c), Some(p)),
            Err(e) => {
                errors.push(format!("Configuration: {}", e));
                (None, None)
            }
        };

        if let Some(ref cfg) = config {
            if let Err(e) = validate_config(cfg) {
                errors.push(format!("Configuration validation: {}", e));
            }
        }

        if !self.config_only {
            // Validate git repository
            match GitRepo::discover(&cwd) {
                Ok(repo) => {
                    // Check if clean
                    match repo.is_clean() {
                        Ok(false) => {
                            warnings.push("Working directory has uncommitted changes".to_string());
                        }
                        Err(e) => {
                            errors.push(format!("Git status check: {}", e));
                        }
                        _ => {}
                    }

                    // Check branch
                    if let Some(ref cfg) = config {
                        match repo.current_branch() {
                            Ok(Some(branch)) => {
                                if branch != cfg.git.branch {
                                    warnings.push(format!(
                                        "Not on release branch '{}', currently on '{}'",
                                        cfg.git.branch, branch
                                    ));
                                }
                            }
                            Ok(None) => {
                                warnings.push("HEAD is detached".to_string());
                            }
                            Err(e) => {
                                errors.push(format!("Branch check: {}", e));
                            }
                        }

                        // Check remote
                        match repo.has_remote(&cfg.git.remote) {
                            Ok(true) => {}
                            Ok(false) => {
                                warnings.push(format!("Remote '{}' not found", cfg.git.remote));
                            }
                            Err(e) => {
                                errors.push(format!("Remote check: {}", e));
                            }
                        }
                    }
                }
                Err(e) => {
                    errors.push(format!("Git repository: {}", e));
                }
            }

            // Validate packages
            match detect_packages(&cwd) {
                Ok(packages) => {
                    if packages.is_empty() {
                        warnings.push("No packages detected".to_string());
                    }
                }
                Err(e) => {
                    errors.push(format!("Package detection: {}", e));
                }
            }
        }

        // If strict, promote warnings to errors
        if self.strict {
            errors.append(&mut warnings);
        }

        // Output
        let passed = errors.is_empty();

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "valid": passed,
                    "config_path": config_path.map(|p| p.to_string_lossy().to_string()),
                    "errors": errors,
                    "warnings": warnings
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                if !cli.quiet {
                    println!("{}", style("Validation Results").bold());
                    println!();

                    if let Some(path) = config_path {
                        println!("Config: {}", style(path.display()).cyan());
                        println!();
                    }

                    if !errors.is_empty() {
                        println!("{}", style("Errors:").red().bold());
                        for error in &errors {
                            println!("  {} {}", style("✗").red(), error);
                        }
                        println!();
                    }

                    if !warnings.is_empty() {
                        println!("{}", style("Warnings:").yellow().bold());
                        for warning in &warnings {
                            println!("  {} {}", style("!").yellow(), warning);
                        }
                        println!();
                    }

                    if passed {
                        if warnings.is_empty() {
                            println!("{}", style("✓ All checks passed").green().bold());
                        } else {
                            println!(
                                "{} with {} warning(s)",
                                style("✓ Validation passed").green().bold(),
                                warnings.len()
                            );
                        }
                    } else {
                        println!(
                            "{} with {} error(s)",
                            style("✗ Validation failed").red().bold(),
                            errors.len()
                        );
                    }
                }
            }
        }

        if !passed {
            std::process::exit(1);
        }

        Ok(())
    }
}
