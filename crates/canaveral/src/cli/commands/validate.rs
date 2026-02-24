//! Validate command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::detect_packages;
use canaveral_core::config::{load_config_from_dir, validation::validate_config};
use canaveral_git::GitRepo;

use crate::cli::output::Ui;
use crate::cli::Cli;

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
        let ui = Ui::new(cli);
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

        if ui.is_json() {
            let output = serde_json::json!({
                "valid": passed,
                "config_path": config_path.map(|p| p.to_string_lossy().to_string()),
                "errors": errors,
                "warnings": warnings
            });
            ui.json(&output)?;
        } else {
            ui.header("Validation Results");
            ui.blank();

            if let Some(path) = config_path {
                ui.key_value("Config", &style(path.display()).cyan().to_string());
                ui.blank();
            }

            if !errors.is_empty() {
                ui.section("Errors");
                for error in &errors {
                    ui.error(error);
                }
                ui.blank();
            }

            if !warnings.is_empty() {
                ui.section("Warnings");
                for warning in &warnings {
                    ui.warning(warning);
                }
                ui.blank();
            }

            if passed {
                if warnings.is_empty() {
                    ui.success("All checks passed");
                } else {
                    ui.success(&format!(
                        "Validation passed with {} warning(s)",
                        warnings.len()
                    ));
                }
            } else {
                ui.error(&format!("Validation failed with {} error(s)", errors.len()));
            }
        }

        if !passed {
            std::process::exit(1);
        }

        Ok(())
    }
}
