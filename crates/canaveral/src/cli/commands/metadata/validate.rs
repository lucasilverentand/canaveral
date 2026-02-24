//! metadata validate subcommand

use clap::Args;
use console::style;
use std::path::PathBuf;

use canaveral_core::config::load_config_or_default;
use canaveral_metadata::{
    AppleValidator, FastlaneStorage, GooglePlayValidator, MetadataStorage, ValidationResult,
};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::TargetPlatform;

/// Validate metadata against store requirements
#[derive(Debug, Args)]
pub struct ValidateArgs {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: TargetPlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Strict mode - fail on warnings (defaults to config value)
    #[arg(long)]
    pub strict: Option<bool>,

    /// Auto-fix common issues
    #[arg(long)]
    pub fix: bool,
}

pub async fn execute(cmd: &ValidateArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let cwd = std::env::current_dir()?;
    let (config, _) = load_config_or_default(&cwd);

    let metadata_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| config.metadata.storage.path.clone());
    let strict = cmd.strict.unwrap_or(config.metadata.validation.strict);

    let storage = FastlaneStorage::new(&metadata_path);

    ui.step(&format!("Validating metadata for {}", &cmd.app_id));

    let result = match cmd.platform {
        TargetPlatform::Apple => {
            if !storage.exists_apple(&cmd.app_id).await? {
                anyhow::bail!(
                    "Apple metadata not found for '{}'. Run 'canaveral metadata init' first.",
                    &cmd.app_id
                );
            }
            let metadata = storage.load_apple(&cmd.app_id).await?;
            let validator = AppleValidator::new(strict);
            validator.validate(&metadata)
        }
        TargetPlatform::GooglePlay => {
            if !storage.exists_google_play(&cmd.app_id).await? {
                anyhow::bail!(
                    "Google Play metadata not found for '{}'. Run 'canaveral metadata init' first.",
                    &cmd.app_id
                );
            }
            let metadata = storage.load_google_play(&cmd.app_id).await?;
            let validator = GooglePlayValidator::new(strict);
            validator.validate(&metadata)
        }
        TargetPlatform::Both => {
            anyhow::bail!("Please specify a single platform for validation (apple or google-play)");
        }
    };

    if cmd.fix && !result.is_valid() {
        ui.blank();
        ui.warning("Auto-fix is not yet implemented for metadata issues.");
        ui.hint("Please review the issues below and fix them manually.");
    }

    print_results(&ui, &result, strict)?;

    if !result.is_valid() {
        anyhow::bail!("Validation failed with {} error(s)", result.error_count());
    }

    if strict && result.warning_count() > 0 {
        anyhow::bail!(
            "Validation failed in strict mode with {} warning(s)",
            result.warning_count()
        );
    }

    Ok(())
}

fn print_results(ui: &Ui, result: &ValidationResult, _strict: bool) -> anyhow::Result<()> {
    ui.json(&serde_json::json!({
        "valid": result.is_valid(),
        "clean": result.is_clean(),
        "error_count": result.error_count(),
        "warning_count": result.warning_count(),
        "issues": result.issues.iter().map(|i| {
            serde_json::json!({
                "severity": format!("{}", i.severity),
                "field": &i.field,
                "message": &i.message,
                "suggestion": &i.suggestion,
            })
        }).collect::<Vec<_>>(),
    }))?;

    if !ui.is_text() {
        return Ok(());
    }

    println!();

    if result.is_clean() {
        println!("{}", style("All validations passed!").green().bold());
        return Ok(());
    }

    let errors = result.errors();
    if !errors.is_empty() {
        println!("{} ({} found)", style("Errors").red().bold(), errors.len());
        for issue in errors {
            println!(
                "  {} {}: {}",
                style("x").red(),
                style(&issue.field).dim(),
                issue.message
            );
            if let Some(ref suggestion) = issue.suggestion {
                println!("    {} {}", style("Suggestion:").dim(), suggestion);
            }
        }
        println!();
    }

    let warnings = result.warnings();
    if !warnings.is_empty() {
        println!(
            "{} ({} found)",
            style("Warnings").yellow().bold(),
            warnings.len()
        );
        for issue in warnings {
            println!(
                "  {} {}: {}",
                style("!").yellow(),
                style(&issue.field).dim(),
                issue.message
            );
            if let Some(ref suggestion) = issue.suggestion {
                println!("    {} {}", style("Suggestion:").dim(), suggestion);
            }
        }
        println!();
    }

    let infos = result.infos();
    if !infos.is_empty() && ui.is_verbose() {
        println!("{} ({} found)", style("Info").blue().bold(), infos.len());
        for issue in infos {
            println!(
                "  {} {}: {}",
                style("i").blue(),
                style(&issue.field).dim(),
                issue.message
            );
        }
        println!();
    }

    if result.is_valid() {
        println!("{}", style("Validation passed with warnings.").yellow());
    } else {
        println!(
            "{}",
            style("Validation failed. Please fix the errors above.").red()
        );
    }

    Ok(())
}
