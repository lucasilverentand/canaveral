//! Metadata CLI commands

use clap::{Args, Subcommand, ValueEnum};
use console::style;
use std::path::PathBuf;

use canaveral_metadata::{
    AppleValidator, FastlaneStorage, GooglePlayValidator, Locale, MetadataStorage, Platform,
    StorageFormat, ValidationResult,
};

use crate::cli::{Cli, OutputFormat};

/// Metadata management commands
#[derive(Debug, Args)]
pub struct MetadataCommand {
    #[command(subcommand)]
    pub command: MetadataSubcommand,
}

/// Metadata subcommands
#[derive(Debug, Subcommand)]
pub enum MetadataSubcommand {
    /// Initialize metadata directory structure
    Init(InitCommand),

    /// Validate metadata against store requirements
    Validate(ValidateCommand),
}

/// Target platform for metadata operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum TargetPlatform {
    /// Apple App Store
    Apple,
    /// Google Play Store
    GooglePlay,
    /// Both platforms
    Both,
}

/// Storage format for metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum MetadataFormat {
    /// Fastlane-compatible directory structure
    #[default]
    Fastlane,
    /// Unified format (future)
    Unified,
}

impl From<MetadataFormat> for StorageFormat {
    fn from(format: MetadataFormat) -> Self {
        match format {
            MetadataFormat::Fastlane => StorageFormat::Fastlane,
            MetadataFormat::Unified => StorageFormat::Unified,
        }
    }
}

/// Initialize metadata directory structure
#[derive(Debug, Args)]
pub struct InitCommand {
    /// Target platform
    #[arg(long, value_enum, default_value = "apple")]
    pub platform: TargetPlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Storage format
    #[arg(long, value_enum, default_value = "fastlane")]
    pub format: MetadataFormat,

    /// Locales to initialize (comma-separated)
    #[arg(long, value_delimiter = ',', default_value = "en-US")]
    pub locales: Vec<String>,

    /// Base path for metadata storage
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,
}

/// Validate metadata against store requirements
#[derive(Debug, Args)]
pub struct ValidateCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: TargetPlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,

    /// Strict mode - fail on warnings
    #[arg(long)]
    pub strict: bool,

    /// Auto-fix common issues
    #[arg(long)]
    pub fix: bool,
}

impl MetadataCommand {
    /// Execute the metadata command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        match &self.command {
            MetadataSubcommand::Init(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Validate(cmd) => rt.block_on(cmd.execute(cli)),
        }
    }
}

impl InitCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Parse locales
        let locales: Vec<Locale> = self
            .locales
            .iter()
            .map(|s| Locale::new(s))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| anyhow::anyhow!("Invalid locale: {}", e))?;

        if locales.is_empty() {
            anyhow::bail!("At least one locale must be specified");
        }

        // Create storage backend
        let storage = FastlaneStorage::new(&self.path);

        if !cli.quiet {
            println!(
                "{} metadata directory structure",
                style("Initializing").cyan()
            );
            println!("  App ID:   {}", style(&self.app_id).bold());
            println!("  Path:     {}", style(self.path.display()).dim());
            println!(
                "  Locales:  {}",
                locales
                    .iter()
                    .map(|l| l.code())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        // Initialize for each platform
        match self.platform {
            TargetPlatform::Apple => {
                storage
                    .init(Platform::Apple, &self.app_id, &locales)
                    .await?;
                if !cli.quiet {
                    println!(
                        "  Platform: {}",
                        style("Apple App Store").green()
                    );
                }
            }
            TargetPlatform::GooglePlay => {
                storage
                    .init(Platform::GooglePlay, &self.app_id, &locales)
                    .await?;
                if !cli.quiet {
                    println!(
                        "  Platform: {}",
                        style("Google Play Store").green()
                    );
                }
            }
            TargetPlatform::Both => {
                storage
                    .init(Platform::Apple, &self.app_id, &locales)
                    .await?;
                storage
                    .init(Platform::GooglePlay, &self.app_id, &locales)
                    .await?;
                if !cli.quiet {
                    println!(
                        "  Platform: {}",
                        style("Apple App Store + Google Play Store").green()
                    );
                }
            }
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "app_id": &self.app_id,
                    "path": self.path.display().to_string(),
                    "platform": format!("{:?}", self.platform),
                    "locales": locales.iter().map(|l| l.code()).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                println!(
                    "{}",
                    style("Metadata directory initialized successfully!").green().bold()
                );
                println!();
                println!("Next steps:");
                println!("  1. Fill in the metadata files in {}", self.path.display());
                println!("  2. Add screenshots to the screenshots/ directory");
                println!(
                    "  3. Run `canaveral metadata validate --platform {} --app-id {}`",
                    match self.platform {
                        TargetPlatform::Apple => "apple",
                        TargetPlatform::GooglePlay => "google-play",
                        TargetPlatform::Both => "apple",
                    },
                    &self.app_id
                );
            }
        }

        Ok(())
    }
}

impl ValidateCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Create storage backend
        let storage = FastlaneStorage::new(&self.path);

        if !cli.quiet {
            println!(
                "{} metadata for {}",
                style("Validating").cyan(),
                style(&self.app_id).bold()
            );
        }

        let result = match self.platform {
            TargetPlatform::Apple => {
                // Check if metadata exists
                if !storage.exists_apple(&self.app_id).await? {
                    anyhow::bail!(
                        "Apple metadata not found for '{}'. Run 'canaveral metadata init' first.",
                        &self.app_id
                    );
                }

                // Load metadata
                let metadata = storage.load_apple(&self.app_id).await?;

                // Validate
                let validator = AppleValidator::new(self.strict);
                validator.validate(&metadata)
            }
            TargetPlatform::GooglePlay => {
                // Check if metadata exists
                if !storage.exists_google_play(&self.app_id).await? {
                    anyhow::bail!(
                        "Google Play metadata not found for '{}'. Run 'canaveral metadata init' first.",
                        &self.app_id
                    );
                }

                // Load metadata
                let metadata = storage.load_google_play(&self.app_id).await?;

                // Validate
                let validator = GooglePlayValidator::new(self.strict);
                validator.validate(&metadata)
            }
            TargetPlatform::Both => {
                anyhow::bail!("Please specify a single platform for validation (apple or google-play)");
            }
        };

        // Handle auto-fix if requested
        if self.fix && !result.is_valid() {
            if !cli.quiet {
                println!();
                println!(
                    "{}",
                    style("Auto-fix is not yet implemented for metadata issues.").yellow()
                );
                println!(
                    "Please review the issues below and fix them manually."
                );
            }
        }

        // Output results
        self.print_results(cli, &result)?;

        // Determine exit status
        if !result.is_valid() {
            anyhow::bail!("Validation failed with {} error(s)", result.error_count());
        }

        if self.strict && result.warning_count() > 0 {
            anyhow::bail!(
                "Validation failed in strict mode with {} warning(s)",
                result.warning_count()
            );
        }

        Ok(())
    }

    fn print_results(&self, cli: &Cli, result: &ValidationResult) -> anyhow::Result<()> {
        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
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
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();

                if result.is_clean() {
                    println!(
                        "{}",
                        style("All validations passed!").green().bold()
                    );
                    return Ok(());
                }

                // Print errors
                let errors = result.errors();
                if !errors.is_empty() {
                    println!(
                        "{} ({} found)",
                        style("Errors").red().bold(),
                        errors.len()
                    );
                    for issue in errors {
                        println!(
                            "  {} {}: {}",
                            style("x").red(),
                            style(&issue.field).dim(),
                            issue.message
                        );
                        if let Some(ref suggestion) = issue.suggestion {
                            println!(
                                "    {} {}",
                                style("Suggestion:").dim(),
                                suggestion
                            );
                        }
                    }
                    println!();
                }

                // Print warnings
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
                            println!(
                                "    {} {}",
                                style("Suggestion:").dim(),
                                suggestion
                            );
                        }
                    }
                    println!();
                }

                // Print info
                let infos = result.infos();
                if !infos.is_empty() && cli.verbose {
                    println!(
                        "{} ({} found)",
                        style("Info").blue().bold(),
                        infos.len()
                    );
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

                // Summary
                if result.is_valid() {
                    println!(
                        "{}",
                        style("Validation passed with warnings.").yellow()
                    );
                } else {
                    println!(
                        "{}",
                        style("Validation failed. Please fix the errors above.").red()
                    );
                }
            }
        }

        Ok(())
    }
}
