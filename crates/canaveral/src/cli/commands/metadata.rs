//! Metadata CLI commands

use clap::{Args, Subcommand, ValueEnum};
use console::style;
use std::io::Write;
use std::path::PathBuf;

use canaveral_core::config::load_config_or_default;
use canaveral_metadata::{
    sync::{
        AppleMetadataSync, AppleSyncConfig, ChangeType, GooglePlayMetadataSync,
        GooglePlaySyncConfig, MetadataDiff, MetadataSync,
    },
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

    /// Export metadata to different formats
    Export(ExportCommand),

    /// Add a new localization
    AddLocale(AddLocaleCommand),

    /// Remove a localization
    RemoveLocale(RemoveLocaleCommand),

    /// List available localizations
    ListLocales(ListLocalesCommand),

    /// Screenshot management commands
    Screenshots(ScreenshotsCommand),

    /// Sync metadata with app stores (pull/push)
    Sync(SyncCommand),

    /// Compare local vs remote metadata
    Diff(DiffCommand),
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

/// Export format for metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ExportFormat {
    /// JSON format (pretty-printed)
    #[default]
    Json,
    /// YAML format
    Yaml,
    /// CSV format (localized text fields only)
    Csv,
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
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Strict mode - fail on warnings (defaults to config value)
    #[arg(long)]
    pub strict: Option<bool>,

    /// Auto-fix common issues
    #[arg(long)]
    pub fix: bool,
}

/// Export metadata to different formats
#[derive(Debug, Args)]
pub struct ExportCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Export format
    #[arg(long, value_enum, default_value = "json")]
    pub format: ExportFormat,

    /// Output file path (defaults to stdout)
    #[arg(long, short = 'o')]
    pub output: Option<PathBuf>,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,
}

/// Target platform for locale operations (single platform only)
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SinglePlatform {
    /// Apple App Store
    Apple,
    /// Google Play Store
    GooglePlay,
}

impl From<SinglePlatform> for Platform {
    fn from(platform: SinglePlatform) -> Self {
        match platform {
            SinglePlatform::Apple => Platform::Apple,
            SinglePlatform::GooglePlay => Platform::GooglePlay,
        }
    }
}

/// Add a new localization
#[derive(Debug, Args)]
pub struct AddLocaleCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (BCP 47 format, e.g., de-DE)
    #[arg(long, required = true)]
    pub locale: String,

    /// Copy content from existing locale
    #[arg(long)]
    pub copy_from: Option<String>,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,
}

/// Remove a localization
#[derive(Debug, Args)]
pub struct RemoveLocaleCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (BCP 47 format, e.g., de-DE)
    #[arg(long, required = true)]
    pub locale: String,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}

/// List available localizations
#[derive(Debug, Args)]
pub struct ListLocalesCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,
}

impl MetadataCommand {
    /// Execute the metadata command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        match &self.command {
            MetadataSubcommand::Init(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Validate(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Export(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::AddLocale(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::RemoveLocale(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::ListLocales(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Screenshots(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Sync(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Diff(cmd) => rt.block_on(cmd.execute(cli)),
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
        // Load config for defaults
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Use config defaults for path and strict mode
        let metadata_path = self
            .path
            .clone()
            .unwrap_or_else(|| config.metadata.storage.path.clone());
        let strict = self
            .strict
            .unwrap_or(config.metadata.validation.strict);

        // Create storage backend
        let storage = FastlaneStorage::new(&metadata_path);

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
                let validator = AppleValidator::new(strict);
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
                let validator = GooglePlayValidator::new(strict);
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
        self.print_results(cli, &result, strict)?;

        // Determine exit status
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

    fn print_results(&self, cli: &Cli, result: &ValidationResult, _strict: bool) -> anyhow::Result<()> {
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

impl ExportCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load config for defaults
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Use config defaults for path
        let metadata_path = self
            .path
            .clone()
            .unwrap_or_else(|| config.metadata.storage.path.clone());

        // Create storage backend
        let storage = FastlaneStorage::new(&metadata_path);

        if !cli.quiet {
            println!(
                "{} metadata for {}",
                style("Exporting").cyan(),
                style(&self.app_id).bold()
            );
        }

        // Load and serialize metadata based on platform
        let output_string = match self.platform {
            SinglePlatform::Apple => {
                // Check if metadata exists
                if !storage.exists_apple(&self.app_id).await? {
                    anyhow::bail!(
                        "Apple metadata not found for '{}'. Run 'canaveral metadata init' first.",
                        &self.app_id
                    );
                }

                // Load metadata
                let metadata = storage.load_apple(&self.app_id).await?;

                // Serialize based on format
                match self.format {
                    ExportFormat::Json => serde_json::to_string_pretty(&metadata)?,
                    ExportFormat::Yaml => serde_yaml::to_string(&metadata)?,
                    ExportFormat::Csv => self.export_apple_csv(&metadata)?,
                }
            }
            SinglePlatform::GooglePlay => {
                // Check if metadata exists
                if !storage.exists_google_play(&self.app_id).await? {
                    anyhow::bail!(
                        "Google Play metadata not found for '{}'. Run 'canaveral metadata init' first.",
                        &self.app_id
                    );
                }

                // Load metadata
                let metadata = storage.load_google_play(&self.app_id).await?;

                // Serialize based on format
                match self.format {
                    ExportFormat::Json => serde_json::to_string_pretty(&metadata)?,
                    ExportFormat::Yaml => serde_yaml::to_string(&metadata)?,
                    ExportFormat::Csv => self.export_google_play_csv(&metadata)?,
                }
            }
        };

        // Write output
        if let Some(ref output_path) = self.output {
            let mut file = std::fs::File::create(output_path)?;
            file.write_all(output_string.as_bytes())?;

            if !cli.quiet {
                println!(
                    "{} Exported to {}",
                    style("Success:").green().bold(),
                    style(output_path.display()).dim()
                );
            }
        } else {
            // Write to stdout
            println!("{}", output_string);
        }

        Ok(())
    }

    /// Export Apple metadata to CSV format (localized text fields)
    fn export_apple_csv(&self, metadata: &canaveral_metadata::AppleMetadata) -> anyhow::Result<String> {
        let mut csv_output = String::new();

        // CSV header
        csv_output.push_str("locale,name,subtitle,description,keywords,whats_new,promotional_text,privacy_policy_url,support_url,marketing_url\n");

        // Sort locales for consistent output
        let mut locales: Vec<_> = metadata.localizations.keys().collect();
        locales.sort();

        for locale in locales {
            if let Some(loc_meta) = metadata.localizations.get(locale) {
                csv_output.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{}\n",
                    escape_csv(locale),
                    escape_csv(&loc_meta.name),
                    escape_csv(loc_meta.subtitle.as_deref().unwrap_or("")),
                    escape_csv(&loc_meta.description),
                    escape_csv(loc_meta.keywords.as_deref().unwrap_or("")),
                    escape_csv(loc_meta.whats_new.as_deref().unwrap_or("")),
                    escape_csv(loc_meta.promotional_text.as_deref().unwrap_or("")),
                    escape_csv(loc_meta.privacy_policy_url.as_deref().unwrap_or("")),
                    escape_csv(loc_meta.support_url.as_deref().unwrap_or("")),
                    escape_csv(loc_meta.marketing_url.as_deref().unwrap_or("")),
                ));
            }
        }

        Ok(csv_output)
    }

    /// Export Google Play metadata to CSV format (localized text fields)
    fn export_google_play_csv(&self, metadata: &canaveral_metadata::GooglePlayMetadata) -> anyhow::Result<String> {
        let mut csv_output = String::new();

        // CSV header
        csv_output.push_str("locale,title,short_description,full_description,video_url\n");

        // Sort locales for consistent output
        let mut locales: Vec<_> = metadata.localizations.keys().collect();
        locales.sort();

        for locale in locales {
            if let Some(loc_meta) = metadata.localizations.get(locale) {
                csv_output.push_str(&format!(
                    "{},{},{},{},{}\n",
                    escape_csv(locale),
                    escape_csv(&loc_meta.title),
                    escape_csv(&loc_meta.short_description),
                    escape_csv(&loc_meta.full_description),
                    escape_csv(loc_meta.video_url.as_deref().unwrap_or("")),
                ));
            }
        }

        Ok(csv_output)
    }
}

/// Escape a string for CSV output
fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

impl AddLocaleCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Parse locale
        let locale = Locale::new(&self.locale)
            .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &self.locale, e))?;

        // Parse copy_from locale if provided
        let copy_from = match &self.copy_from {
            Some(code) => Some(
                Locale::new(code)
                    .map_err(|e| anyhow::anyhow!("Invalid source locale '{}': {}", code, e))?,
            ),
            None => None,
        };

        // Create storage backend
        let storage = FastlaneStorage::new(&self.path);

        if !cli.quiet {
            println!(
                "{} locale {} for {}",
                style("Adding").cyan(),
                style(locale.code()).bold(),
                style(&self.app_id).bold()
            );
            if let Some(ref source) = copy_from {
                println!("  Copying from: {}", style(source.code()).dim());
            }
        }

        // Add the locale
        let platform: Platform = self.platform.into();
        storage
            .add_locale(platform, &self.app_id, &locale, copy_from.as_ref())
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "locale": locale.code(),
                    "copied_from": copy_from.as_ref().map(|l| l.code()),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                println!(
                    "{}",
                    style(format!("Locale '{}' added successfully!", locale.code()))
                        .green()
                        .bold()
                );
            }
        }

        Ok(())
    }
}

impl RemoveLocaleCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Parse locale
        let locale = Locale::new(&self.locale)
            .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &self.locale, e))?;

        // Confirmation prompt
        if !self.yes && !cli.quiet {
            println!(
                "{} Are you sure you want to remove locale '{}' for '{}'?",
                style("Warning:").yellow().bold(),
                style(locale.code()).bold(),
                style(&self.app_id).bold()
            );
            println!("This will permanently delete all metadata files for this locale.");
            print!("Type 'yes' to confirm: ");
            std::io::Write::flush(&mut std::io::stdout())?;

            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
            if input.trim().to_lowercase() != "yes" {
                println!("{}", style("Aborted.").red());
                return Ok(());
            }
        }

        // Create storage backend
        let storage = FastlaneStorage::new(&self.path);

        if !cli.quiet {
            println!(
                "{} locale {} for {}",
                style("Removing").cyan(),
                style(locale.code()).bold(),
                style(&self.app_id).bold()
            );
        }

        // Remove the locale
        let platform: Platform = self.platform.into();
        storage
            .remove_locale(platform, &self.app_id, &locale)
            .await
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "locale": locale.code(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                println!(
                    "{}",
                    style(format!("Locale '{}' removed successfully!", locale.code()))
                        .green()
                        .bold()
                );
            }
        }

        Ok(())
    }
}

impl ListLocalesCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Create storage backend
        let storage = FastlaneStorage::new(&self.path);

        if !cli.quiet {
            println!(
                "{} locales for {}",
                style("Listing").cyan(),
                style(&self.app_id).bold()
            );
        }

        // Get locales based on platform
        let platform: Platform = self.platform.into();
        let locales = match platform {
            Platform::Apple => storage.list_locales_apple(&self.app_id).await,
            Platform::GooglePlay => storage.list_locales_google_play(&self.app_id).await,
        }
        .map_err(|e| anyhow::anyhow!("{}", e))?;

        // Count files in each locale directory
        let mut locale_info: Vec<(String, usize)> = Vec::new();
        let app_path = match platform {
            Platform::Apple => storage.apple_path(&self.app_id),
            Platform::GooglePlay => storage.google_play_path(&self.app_id),
        };

        for locale in &locales {
            let locale_path = app_path.join(locale.code());
            let file_count = count_files_in_dir(&locale_path).await.unwrap_or(0);
            locale_info.push((locale.code(), file_count));
        }

        // Sort by locale code
        locale_info.sort_by(|a, b| a.0.cmp(&b.0));

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "locale_count": locales.len(),
                    "locales": locale_info.iter().map(|(code, count)| {
                        serde_json::json!({
                            "code": code,
                            "file_count": count,
                        })
                    }).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                if locale_info.is_empty() {
                    println!(
                        "{}",
                        style("No locales found.").yellow()
                    );
                    println!(
                        "Run `canaveral metadata init` to create the metadata structure."
                    );
                } else {
                    println!(
                        "{} ({} found)",
                        style("Locales").green().bold(),
                        locale_info.len()
                    );
                    for (code, count) in &locale_info {
                        println!(
                            "  {} {} ({} files)",
                            style("-").dim(),
                            style(code).bold(),
                            count
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

/// Count files in a directory (non-recursive).
async fn count_files_in_dir(path: &std::path::Path) -> std::io::Result<usize> {
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.path().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

// =============================================================================
// Screenshots Commands
// =============================================================================

/// Screenshot management commands
#[derive(Debug, Args)]
pub struct ScreenshotsCommand {
    #[command(subcommand)]
    pub command: ScreenshotsSubcommand,
}

/// Screenshots subcommands
#[derive(Debug, Subcommand)]
pub enum ScreenshotsSubcommand {
    /// Add a screenshot
    Add(ScreenshotsAddCommand),

    /// Remove a screenshot
    Remove(ScreenshotsRemoveCommand),

    /// List screenshots
    List(ScreenshotsListCommand),

    /// Validate screenshot dimensions
    Validate(ScreenshotsValidateCommand),
}

/// Apple device types for screenshots
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum AppleDeviceType {
    /// iPhone 6.5" display (iPhone 14 Pro Max, etc.)
    #[value(name = "iphone_6_5")]
    Iphone65,
    /// iPhone 5.5" display (iPhone 8 Plus, etc.)
    #[value(name = "iphone_5_5")]
    Iphone55,
    /// iPhone 6.7" display (iPhone 14 Plus, etc.)
    #[value(name = "iphone_6_7")]
    Iphone67,
    /// iPhone 6.1" display (iPhone 14, etc.)
    #[value(name = "iphone_6_1")]
    Iphone61,
    /// iPad Pro 12.9"
    #[value(name = "ipad_pro_12_9")]
    IpadPro129,
    /// iPad Pro 11"
    #[value(name = "ipad_pro_11")]
    IpadPro11,
    /// iPad 10.5"
    #[value(name = "ipad_10_5")]
    Ipad105,
    /// Apple Watch Series 9
    #[value(name = "apple_watch")]
    AppleWatch,
    /// Apple TV
    #[value(name = "apple_tv")]
    AppleTv,
}

impl AppleDeviceType {
    /// Returns the directory name for this device type
    fn as_dir_name(&self) -> &'static str {
        match self {
            AppleDeviceType::Iphone65 => "iphone_6_5",
            AppleDeviceType::Iphone55 => "iphone_5_5",
            AppleDeviceType::Iphone67 => "iphone_6_7",
            AppleDeviceType::Iphone61 => "iphone_6_1",
            AppleDeviceType::IpadPro129 => "ipad_pro_12_9",
            AppleDeviceType::IpadPro11 => "ipad_pro_11",
            AppleDeviceType::Ipad105 => "ipad_10_5",
            AppleDeviceType::AppleWatch => "watch_series_9",
            AppleDeviceType::AppleTv => "apple_tv",
        }
    }
}

/// Google Play device types for screenshots
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum GooglePlayDeviceType {
    /// Phone screenshots
    Phone,
    /// 7" tablet screenshots
    #[value(name = "tablet_7")]
    Tablet7,
    /// 10" tablet screenshots
    #[value(name = "tablet_10")]
    Tablet10,
    /// TV screenshots
    Tv,
    /// Wear OS screenshots
    Wear,
}

impl GooglePlayDeviceType {
    /// Returns the directory name for this device type
    fn as_dir_name(&self) -> &'static str {
        match self {
            GooglePlayDeviceType::Phone => "phone",
            GooglePlayDeviceType::Tablet7 => "tablet_7",
            GooglePlayDeviceType::Tablet10 => "tablet_10",
            GooglePlayDeviceType::Tv => "tv",
            GooglePlayDeviceType::Wear => "wear",
        }
    }
}

/// Device type enum that can be either Apple or Google Play
#[derive(Debug, Clone)]
pub enum DeviceType {
    Apple(AppleDeviceType),
    GooglePlay(GooglePlayDeviceType),
}

impl DeviceType {
    /// Returns the directory name for this device type
    fn as_dir_name(&self) -> &'static str {
        match self {
            DeviceType::Apple(d) => d.as_dir_name(),
            DeviceType::GooglePlay(d) => d.as_dir_name(),
        }
    }
}

/// Add a screenshot
#[derive(Debug, Args)]
pub struct ScreenshotsAddCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (BCP 47 format, e.g., en-US)
    #[arg(long, required = true)]
    pub locale: String,

    /// Apple device type (only for Apple platform)
    #[arg(long, value_enum, required_if_eq("platform", "apple"))]
    pub apple_device: Option<AppleDeviceType>,

    /// Google Play device type (only for Google Play platform)
    #[arg(long, value_enum, required_if_eq("platform", "google-play"))]
    pub google_device: Option<GooglePlayDeviceType>,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,

    /// Screenshot file to add
    #[arg(required = true)]
    pub file: PathBuf,
}

/// Remove a screenshot
#[derive(Debug, Args)]
pub struct ScreenshotsRemoveCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (BCP 47 format, e.g., en-US)
    #[arg(long, required = true)]
    pub locale: String,

    /// Apple device type (only for Apple platform)
    #[arg(long, value_enum, required_if_eq("platform", "apple"))]
    pub apple_device: Option<AppleDeviceType>,

    /// Google Play device type (only for Google Play platform)
    #[arg(long, value_enum, required_if_eq("platform", "google-play"))]
    pub google_device: Option<GooglePlayDeviceType>,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,

    /// Screenshot filename to remove
    #[arg(required = true)]
    pub filename: String,
}

/// List screenshots
#[derive(Debug, Args)]
pub struct ScreenshotsListCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (optional, lists all locales if omitted)
    #[arg(long)]
    pub locale: Option<String>,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,
}

/// Validate screenshot dimensions
#[derive(Debug, Args)]
pub struct ScreenshotsValidateCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locale code (optional, validates all locales if omitted)
    #[arg(long)]
    pub locale: Option<String>,

    /// Path to metadata directory
    #[arg(long, default_value = "./metadata")]
    pub path: PathBuf,
}

impl ScreenshotsCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            ScreenshotsSubcommand::Add(cmd) => cmd.execute(cli).await,
            ScreenshotsSubcommand::Remove(cmd) => cmd.execute(cli).await,
            ScreenshotsSubcommand::List(cmd) => cmd.execute(cli).await,
            ScreenshotsSubcommand::Validate(cmd) => cmd.execute(cli).await,
        }
    }
}

impl ScreenshotsAddCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_metadata::{
            read_image_dimensions, validate_apple_screenshot_file,
            validate_google_play_screenshot_file,
        };

        // Parse locale
        let locale = Locale::new(&self.locale)
            .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &self.locale, e))?;

        // Get device type based on platform
        let device_type = match self.platform {
            SinglePlatform::Apple => {
                let device = self
                    .apple_device
                    .ok_or_else(|| anyhow::anyhow!("--apple-device is required for Apple platform"))?;
                DeviceType::Apple(device)
            }
            SinglePlatform::GooglePlay => {
                let device = self
                    .google_device
                    .ok_or_else(|| anyhow::anyhow!("--google-device is required for Google Play platform"))?;
                DeviceType::GooglePlay(device)
            }
        };

        // Verify source file exists
        if !self.file.exists() {
            anyhow::bail!("Screenshot file does not exist: {:?}", self.file);
        }

        if !self.file.is_file() {
            anyhow::bail!("Path is not a file: {:?}", self.file);
        }

        // Get the storage
        let storage = FastlaneStorage::new(&self.path);

        // Check if app metadata exists
        let app_path = match self.platform {
            SinglePlatform::Apple => storage.apple_path(&self.app_id),
            SinglePlatform::GooglePlay => storage.google_play_path(&self.app_id),
        };

        if !app_path.exists() {
            anyhow::bail!(
                "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
                &self.app_id
            );
        }

        // Determine the screenshots directory
        let screenshots_dir = match self.platform {
            SinglePlatform::Apple => {
                // Apple: screenshots/{locale}/{device_type}/
                app_path
                    .join("screenshots")
                    .join(locale.code())
                    .join(device_type.as_dir_name())
            }
            SinglePlatform::GooglePlay => {
                // Google Play: screenshots/{locale}/{device_type}/
                app_path
                    .join("screenshots")
                    .join(locale.code())
                    .join(device_type.as_dir_name())
            }
        };

        // Create directory if it doesn't exist
        tokio::fs::create_dir_all(&screenshots_dir).await?;

        // Read source file dimensions
        let dimensions = read_image_dimensions(&self.file)
            .map_err(|e| anyhow::anyhow!("Failed to read image dimensions: {}", e))?;

        // Validate dimensions and warn if invalid
        let validation_result = match self.platform {
            SinglePlatform::Apple => validate_apple_screenshot_file(&self.file, device_type.as_dir_name()),
            SinglePlatform::GooglePlay => {
                validate_google_play_screenshot_file(&self.file, device_type.as_dir_name())
            }
        };

        if !validation_result.is_valid() {
            for error in validation_result.errors() {
                eprintln!(
                    "{} {}",
                    style("Warning:").yellow().bold(),
                    error.message
                );
                if let Some(ref suggestion) = error.suggestion {
                    eprintln!("  {}", style(suggestion).dim());
                }
            }
        }

        // Find the next available number
        let next_number = find_next_screenshot_number(&screenshots_dir).await?;

        // Determine the extension from the source file
        let extension = self
            .file
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");

        // Create the destination filename
        let dest_filename = format!("{:02}.{}", next_number, extension);
        let dest_path = screenshots_dir.join(&dest_filename);

        // Copy the file
        tokio::fs::copy(&self.file, &dest_path).await?;

        if !cli.quiet {
            println!(
                "{} screenshot to {}",
                style("Added").green().bold(),
                style(dest_path.display()).dim()
            );
            println!(
                "  Dimensions: {}x{}",
                dimensions.width, dimensions.height
            );
            println!(
                "  Locale:     {}",
                locale.code()
            );
            println!(
                "  Device:     {}",
                device_type.as_dir_name()
            );
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "source": self.file.display().to_string(),
                    "destination": dest_path.display().to_string(),
                    "filename": dest_filename,
                    "dimensions": {
                        "width": dimensions.width,
                        "height": dimensions.height,
                    },
                    "locale": locale.code(),
                    "device": device_type.as_dir_name(),
                    "validation_passed": validation_result.is_valid(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {}
        }

        Ok(())
    }
}

impl ScreenshotsRemoveCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Parse locale
        let locale = Locale::new(&self.locale)
            .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &self.locale, e))?;

        // Get device type based on platform
        let device_type = match self.platform {
            SinglePlatform::Apple => {
                let device = self
                    .apple_device
                    .ok_or_else(|| anyhow::anyhow!("--apple-device is required for Apple platform"))?;
                DeviceType::Apple(device)
            }
            SinglePlatform::GooglePlay => {
                let device = self
                    .google_device
                    .ok_or_else(|| anyhow::anyhow!("--google-device is required for Google Play platform"))?;
                DeviceType::GooglePlay(device)
            }
        };

        // Get the storage
        let storage = FastlaneStorage::new(&self.path);

        // Check if app metadata exists
        let app_path = match self.platform {
            SinglePlatform::Apple => storage.apple_path(&self.app_id),
            SinglePlatform::GooglePlay => storage.google_play_path(&self.app_id),
        };

        if !app_path.exists() {
            anyhow::bail!(
                "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
                &self.app_id
            );
        }

        // Determine the screenshots directory
        let screenshots_dir = app_path
            .join("screenshots")
            .join(locale.code())
            .join(device_type.as_dir_name());

        // Find the file to remove
        let file_to_remove = screenshots_dir.join(&self.filename);

        if !file_to_remove.exists() {
            anyhow::bail!("Screenshot file not found: {:?}", file_to_remove);
        }

        // Remove the file
        tokio::fs::remove_file(&file_to_remove).await?;

        if !cli.quiet {
            println!(
                "{} screenshot: {}",
                style("Removed").red().bold(),
                style(&self.filename).dim()
            );
        }

        // Re-number remaining files
        renumber_screenshots(&screenshots_dir).await?;

        if !cli.quiet {
            println!(
                "{}",
                style("Re-numbered remaining screenshots.").dim()
            );
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "removed": self.filename,
                    "locale": locale.code(),
                    "device": device_type.as_dir_name(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {}
        }

        Ok(())
    }
}

impl ScreenshotsListCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_metadata::read_image_dimensions;

        // Get the storage
        let storage = FastlaneStorage::new(&self.path);

        // Check if app metadata exists
        let app_path = match self.platform {
            SinglePlatform::Apple => storage.apple_path(&self.app_id),
            SinglePlatform::GooglePlay => storage.google_play_path(&self.app_id),
        };

        if !app_path.exists() {
            anyhow::bail!(
                "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
                &self.app_id
            );
        }

        let screenshots_base = app_path.join("screenshots");

        if !screenshots_base.exists() {
            if !cli.quiet {
                println!(
                    "{}",
                    style("No screenshots directory found.").yellow()
                );
            }
            return Ok(());
        }

        // Determine which locales to list
        let locales: Vec<String> = if let Some(ref locale_code) = self.locale {
            vec![locale_code.clone()]
        } else {
            // List all locale directories
            list_subdirectories(&screenshots_base).await?
        };

        // Get device types based on platform
        let device_types: Vec<&str> = match self.platform {
            SinglePlatform::Apple => vec![
                "iphone_6_5",
                "iphone_5_5",
                "iphone_6_7",
                "iphone_6_1",
                "ipad_pro_12_9",
                "ipad_pro_11",
                "ipad_10_5",
                "watch_series_9",
                "apple_tv",
            ],
            SinglePlatform::GooglePlay => vec!["phone", "tablet_7", "tablet_10", "tv", "wear"],
        };

        #[derive(serde::Serialize)]
        struct ScreenshotInfo {
            filename: String,
            width: u32,
            height: u32,
        }

        #[derive(serde::Serialize)]
        struct DeviceScreenshots {
            device: String,
            screenshots: Vec<ScreenshotInfo>,
        }

        #[derive(serde::Serialize)]
        struct LocaleScreenshots {
            locale: String,
            devices: Vec<DeviceScreenshots>,
        }

        let mut all_locales: Vec<LocaleScreenshots> = Vec::new();

        for locale_code in &locales {
            let locale_path = screenshots_base.join(locale_code);
            if !locale_path.exists() {
                continue;
            }

            let mut locale_data = LocaleScreenshots {
                locale: locale_code.clone(),
                devices: Vec::new(),
            };

            for device_type in &device_types {
                let device_path = locale_path.join(device_type);
                if !device_path.exists() {
                    continue;
                }

                let screenshots = list_image_files(&device_path).await?;
                if screenshots.is_empty() {
                    continue;
                }

                let mut device_screenshots = DeviceScreenshots {
                    device: device_type.to_string(),
                    screenshots: Vec::new(),
                };

                for screenshot_path in screenshots {
                    let filename = screenshot_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let (width, height) = match read_image_dimensions(&screenshot_path) {
                        Ok(dims) => (dims.width, dims.height),
                        Err(_) => (0, 0),
                    };

                    device_screenshots.screenshots.push(ScreenshotInfo {
                        filename,
                        width,
                        height,
                    });
                }

                locale_data.devices.push(device_screenshots);
            }

            if !locale_data.devices.is_empty() {
                all_locales.push(locale_data);
            }
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "locales": all_locales,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                if all_locales.is_empty() {
                    println!(
                        "{}",
                        style("No screenshots found.").yellow()
                    );
                } else {
                    println!(
                        "{} for {}",
                        style("Screenshots").green().bold(),
                        style(&self.app_id).bold()
                    );
                    println!();

                    for locale_data in &all_locales {
                        println!(
                            "  {} {}",
                            style("Locale:").cyan(),
                            style(&locale_data.locale).bold()
                        );

                        for device_data in &locale_data.devices {
                            println!(
                                "    {} {} ({} screenshots)",
                                style("-").dim(),
                                style(&device_data.device).yellow(),
                                device_data.screenshots.len()
                            );

                            for screenshot in &device_data.screenshots {
                                println!(
                                    "      {} {} ({}x{})",
                                    style("-").dim(),
                                    screenshot.filename,
                                    screenshot.width,
                                    screenshot.height
                                );
                            }
                        }
                        println!();
                    }
                }
            }
        }

        Ok(())
    }
}

impl ScreenshotsValidateCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        use canaveral_metadata::{
            read_image_dimensions, validate_apple_screenshot_file,
            validate_google_play_screenshot_file, ValidationResult,
        };

        // Get the storage
        let storage = FastlaneStorage::new(&self.path);

        // Check if app metadata exists
        let app_path = match self.platform {
            SinglePlatform::Apple => storage.apple_path(&self.app_id),
            SinglePlatform::GooglePlay => storage.google_play_path(&self.app_id),
        };

        if !app_path.exists() {
            anyhow::bail!(
                "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
                &self.app_id
            );
        }

        let screenshots_base = app_path.join("screenshots");

        if !screenshots_base.exists() {
            if !cli.quiet {
                println!(
                    "{}",
                    style("No screenshots directory found.").yellow()
                );
            }
            return Ok(());
        }

        // Determine which locales to validate
        let locales: Vec<String> = if let Some(ref locale_code) = self.locale {
            vec![locale_code.clone()]
        } else {
            list_subdirectories(&screenshots_base).await?
        };

        // Get device types based on platform
        let device_types: Vec<&str> = match self.platform {
            SinglePlatform::Apple => vec![
                "iphone_6_5",
                "iphone_5_5",
                "iphone_6_7",
                "iphone_6_1",
                "ipad_pro_12_9",
                "ipad_pro_11",
                "ipad_10_5",
                "watch_series_9",
                "apple_tv",
            ],
            SinglePlatform::GooglePlay => vec!["phone", "tablet_7", "tablet_10", "tv", "wear"],
        };

        if !cli.quiet {
            println!(
                "{} screenshots for {}",
                style("Validating").cyan(),
                style(&self.app_id).bold()
            );
        }

        let mut overall_result = ValidationResult::new();
        let mut validated_count = 0;

        #[derive(serde::Serialize)]
        struct ValidationIssueJson {
            file: String,
            locale: String,
            device: String,
            severity: String,
            message: String,
            suggestion: Option<String>,
            dimensions: Option<String>,
        }

        let mut issues_json: Vec<ValidationIssueJson> = Vec::new();

        for locale_code in &locales {
            let locale_path = screenshots_base.join(locale_code);
            if !locale_path.exists() {
                continue;
            }

            for device_type in &device_types {
                let device_path = locale_path.join(device_type);
                if !device_path.exists() {
                    continue;
                }

                let screenshots = list_image_files(&device_path).await?;

                for screenshot_path in screenshots {
                    validated_count += 1;

                    let validation_result = match self.platform {
                        SinglePlatform::Apple => {
                            validate_apple_screenshot_file(&screenshot_path, device_type)
                        }
                        SinglePlatform::GooglePlay => {
                            validate_google_play_screenshot_file(&screenshot_path, device_type)
                        }
                    };

                    let filename = screenshot_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown");

                    let dims_str = match read_image_dimensions(&screenshot_path) {
                        Ok(dims) => Some(format!("{}x{}", dims.width, dims.height)),
                        Err(_) => None,
                    };

                    for issue in &validation_result.issues {
                        issues_json.push(ValidationIssueJson {
                            file: filename.to_string(),
                            locale: locale_code.clone(),
                            device: device_type.to_string(),
                            severity: format!("{}", issue.severity),
                            message: issue.message.clone(),
                            suggestion: issue.suggestion.clone(),
                            dimensions: dims_str.clone(),
                        });

                        if cli.format == OutputFormat::Text && !cli.quiet {
                            let severity_style = match issue.severity {
                                canaveral_metadata::Severity::Error => style("ERROR").red().bold(),
                                canaveral_metadata::Severity::Warning => style("WARN").yellow(),
                                canaveral_metadata::Severity::Info => style("INFO").blue(),
                            };

                            println!(
                                "  {} {}/{}/{}: {}",
                                severity_style,
                                locale_code,
                                device_type,
                                filename,
                                issue.message
                            );

                            if let Some(ref suggestion) = issue.suggestion {
                                println!("    {} {}", style("Suggestion:").dim(), suggestion);
                            }
                        }
                    }

                    overall_result.merge(validation_result);
                }
            }
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "validated_count": validated_count,
                    "valid": overall_result.is_valid(),
                    "error_count": overall_result.error_count(),
                    "warning_count": overall_result.warning_count(),
                    "issues": issues_json,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                if overall_result.is_clean() {
                    println!(
                        "{} All {} screenshots are valid!",
                        style("Success:").green().bold(),
                        validated_count
                    );
                } else if overall_result.is_valid() {
                    println!(
                        "{} {} screenshots validated with {} warning(s).",
                        style("Done:").yellow().bold(),
                        validated_count,
                        overall_result.warning_count()
                    );
                } else {
                    println!(
                        "{} {} screenshots validated with {} error(s) and {} warning(s).",
                        style("Failed:").red().bold(),
                        validated_count,
                        overall_result.error_count(),
                        overall_result.warning_count()
                    );
                }
            }
        }

        if !overall_result.is_valid() {
            anyhow::bail!(
                "Screenshot validation failed with {} error(s)",
                overall_result.error_count()
            );
        }

        Ok(())
    }
}

// =============================================================================
// Sync Commands
// =============================================================================

/// Apple App Store Connect authentication options
#[derive(Args, Debug, Clone)]
pub struct AppleAuthOptions {
    /// App Store Connect API Key ID
    #[arg(long, env = "APP_STORE_CONNECT_KEY_ID")]
    pub api_key_id: Option<String>,

    /// App Store Connect API Issuer ID
    #[arg(long, env = "APP_STORE_CONNECT_ISSUER_ID")]
    pub api_issuer_id: Option<String>,

    /// Path to App Store Connect API private key (.p8 file)
    #[arg(long, env = "APP_STORE_CONNECT_KEY_PATH")]
    pub api_key_path: Option<PathBuf>,
}

impl AppleAuthOptions {
    /// Build AppleSyncConfig from CLI options or environment
    pub fn to_config(&self) -> anyhow::Result<AppleSyncConfig> {
        let api_key_id = self
            .api_key_id
            .clone()
            .or_else(|| std::env::var("APP_STORE_CONNECT_KEY_ID").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Key ID. Provide --api-key-id or set APP_STORE_CONNECT_KEY_ID"
                )
            })?;

        let api_issuer_id = self
            .api_issuer_id
            .clone()
            .or_else(|| std::env::var("APP_STORE_CONNECT_ISSUER_ID").ok())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Issuer ID. Provide --api-issuer-id or set APP_STORE_CONNECT_ISSUER_ID"
                )
            })?;

        let key_path = self
            .api_key_path
            .clone()
            .or_else(|| std::env::var("APP_STORE_CONNECT_KEY_PATH").ok().map(PathBuf::from))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Missing API Key Path. Provide --api-key-path or set APP_STORE_CONNECT_KEY_PATH"
                )
            })?;

        let api_private_key = std::fs::read_to_string(&key_path).map_err(|e| {
            anyhow::anyhow!("Failed to read API key file '{}': {}", key_path.display(), e)
        })?;

        Ok(AppleSyncConfig {
            api_key_id,
            api_issuer_id,
            api_private_key,
            team_id: None,
        })
    }
}

/// Google Play Console authentication options
#[derive(Args, Debug, Clone)]
pub struct GooglePlayAuthOptions {
    /// Path to Google Play service account JSON key
    #[arg(long, env = "GOOGLE_PLAY_SERVICE_ACCOUNT_KEY")]
    pub service_account_key: Option<PathBuf>,
}

impl GooglePlayAuthOptions {
    /// Build GooglePlaySyncConfig from CLI options or environment
    pub fn to_config(&self) -> anyhow::Result<GooglePlaySyncConfig> {
        let key_path = self
            .service_account_key
            .clone()
            .or_else(|| {
                std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT_KEY")
                    .ok()
                    .map(PathBuf::from)
            })
            .or_else(|| {
                std::env::var("GOOGLE_APPLICATION_CREDENTIALS")
                    .ok()
                    .map(PathBuf::from)
            });

        if let Some(path) = key_path {
            Ok(GooglePlaySyncConfig::from_key_file(path))
        } else {
            Err(anyhow::anyhow!(
                "Missing service account key. Provide --service-account-key or set GOOGLE_PLAY_SERVICE_ACCOUNT_KEY"
            ))
        }
    }
}

/// Sync commands container
#[derive(Debug, Args)]
pub struct SyncCommand {
    #[command(subcommand)]
    pub command: SyncSubcommand,
}

/// Sync subcommands
#[derive(Debug, Subcommand)]
pub enum SyncSubcommand {
    /// Download metadata from app store
    Pull(SyncPullCommand),

    /// Upload metadata to app store
    Push(SyncPushCommand),
}

/// Pull metadata from app store
#[derive(Debug, Args)]
pub struct SyncPullCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locales to pull (comma-separated, or "all")
    #[arg(long, default_value = "all")]
    pub locales: String,

    /// Also download screenshots
    #[arg(long)]
    pub include_assets: bool,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apple authentication options
    #[command(flatten)]
    pub apple_auth: AppleAuthOptions,

    /// Google Play authentication options
    #[command(flatten)]
    pub google_auth: GooglePlayAuthOptions,
}

/// Push metadata to app store
#[derive(Debug, Args)]
pub struct SyncPushCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locales to push (comma-separated, or "all")
    #[arg(long, default_value = "all")]
    pub locales: String,

    /// Also upload screenshots
    #[arg(long)]
    pub include_assets: bool,

    /// Only update text, skip screenshots
    #[arg(long)]
    pub skip_screenshots: bool,

    /// Preview changes without actually pushing
    #[arg(long)]
    pub dry_run: bool,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apple authentication options
    #[command(flatten)]
    pub apple_auth: AppleAuthOptions,

    /// Google Play authentication options
    #[command(flatten)]
    pub google_auth: GooglePlayAuthOptions,
}

/// Compare local vs remote metadata
#[derive(Debug, Args)]
pub struct DiffCommand {
    /// Target platform
    #[arg(long, value_enum, required = true)]
    pub platform: SinglePlatform,

    /// App identifier (bundle ID or package name)
    #[arg(long, required = true)]
    pub app_id: String,

    /// Locales to compare (comma-separated, or "all")
    #[arg(long, default_value = "all")]
    pub locales: String,

    /// Path to metadata directory
    #[arg(long)]
    pub path: Option<PathBuf>,

    /// Apple authentication options
    #[command(flatten)]
    pub apple_auth: AppleAuthOptions,

    /// Google Play authentication options
    #[command(flatten)]
    pub google_auth: GooglePlayAuthOptions,
}

impl SyncCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            SyncSubcommand::Pull(cmd) => cmd.execute(cli).await,
            SyncSubcommand::Push(cmd) => cmd.execute(cli).await,
        }
    }
}

impl SyncPullCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load config for defaults
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let metadata_path = self
            .path
            .clone()
            .unwrap_or_else(|| config.metadata.storage.path.clone());

        // Parse locales
        let locales = parse_locales(&self.locales)?;

        if !cli.quiet {
            println!(
                "{} metadata from {}",
                style("Pulling").cyan(),
                match self.platform {
                    SinglePlatform::Apple => "App Store Connect",
                    SinglePlatform::GooglePlay => "Google Play Console",
                }
            );
            println!("  App ID:  {}", style(&self.app_id).bold());
            println!("  Path:    {}", style(metadata_path.display()).dim());
            if let Some(ref locs) = locales {
                println!(
                    "  Locales: {}",
                    locs.iter().map(|l| l.code()).collect::<Vec<_>>().join(", ")
                );
            } else {
                println!("  Locales: {}", style("all").dim());
            }
        }

        match self.platform {
            SinglePlatform::Apple => {
                let config = self.apple_auth.to_config()?;
                let sync = AppleMetadataSync::new(config, metadata_path).await?;
                sync.pull(&self.app_id, locales.as_deref()).await?;
            }
            SinglePlatform::GooglePlay => {
                let config = self.google_auth.to_config()?;
                let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
                sync.pull(&self.app_id, locales.as_deref()).await?;
            }
        }

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "operation": "pull",
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                println!(
                    "{}",
                    style("Metadata pulled successfully!").green().bold()
                );
            }
        }

        Ok(())
    }
}

impl SyncPushCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load config for defaults
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let metadata_path = self
            .path
            .clone()
            .unwrap_or_else(|| config.metadata.storage.path.clone());

        // Parse locales
        let locales = parse_locales(&self.locales)?;

        if !cli.quiet {
            println!(
                "{} metadata to {}{}",
                style("Pushing").cyan(),
                match self.platform {
                    SinglePlatform::Apple => "App Store Connect",
                    SinglePlatform::GooglePlay => "Google Play Console",
                },
                if self.dry_run {
                    style(" (dry run)").yellow().to_string()
                } else {
                    String::new()
                }
            );
            println!("  App ID:  {}", style(&self.app_id).bold());
            println!("  Path:    {}", style(metadata_path.display()).dim());
            if let Some(ref locs) = locales {
                println!(
                    "  Locales: {}",
                    locs.iter().map(|l| l.code()).collect::<Vec<_>>().join(", ")
                );
            } else {
                println!("  Locales: {}", style("all").dim());
            }
        }

        let result = match self.platform {
            SinglePlatform::Apple => {
                let config = self.apple_auth.to_config()?;
                let sync = AppleMetadataSync::new(config, metadata_path).await?;
                sync.push(&self.app_id, locales.as_deref(), self.dry_run)
                    .await?
            }
            SinglePlatform::GooglePlay => {
                let config = self.google_auth.to_config()?;
                let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
                sync.push(&self.app_id, locales.as_deref(), self.dry_run)
                    .await?
            }
        };

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "success": true,
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "operation": "push",
                    "dry_run": self.dry_run,
                    "updated_locales": result.updated_locales,
                    "updated_fields": result.updated_fields,
                    "screenshots_uploaded": result.screenshots_uploaded,
                    "screenshots_removed": result.screenshots_removed,
                    "warnings": result.warnings,
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                println!();
                if result.has_changes() {
                    if self.dry_run {
                        println!(
                            "{} {}",
                            style("Would push:").yellow().bold(),
                            result
                        );
                    } else {
                        println!(
                            "{} {}",
                            style("Pushed:").green().bold(),
                            result
                        );
                    }

                    if !result.updated_locales.is_empty() {
                        println!();
                        println!("  Updated locales:");
                        for locale in &result.updated_locales {
                            println!("    {} {}", style("-").dim(), locale);
                        }
                    }

                    if !result.warnings.is_empty() {
                        println!();
                        println!("  {}:", style("Warnings").yellow());
                        for warning in &result.warnings {
                            println!("    {} {}", style("!").yellow(), warning);
                        }
                    }
                } else {
                    println!(
                        "{}",
                        style("No changes to push.").dim()
                    );
                }
            }
        }

        Ok(())
    }
}

impl DiffCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load config for defaults
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let metadata_path = self
            .path
            .clone()
            .unwrap_or_else(|| config.metadata.storage.path.clone());

        if !cli.quiet {
            println!(
                "{} local metadata with {}",
                style("Comparing").cyan(),
                match self.platform {
                    SinglePlatform::Apple => "App Store Connect",
                    SinglePlatform::GooglePlay => "Google Play Console",
                }
            );
            println!("  App ID: {}", style(&self.app_id).bold());
            println!("  Path:   {}", style(metadata_path.display()).dim());
        }

        let diff = match self.platform {
            SinglePlatform::Apple => {
                let config = self.apple_auth.to_config()?;
                let sync = AppleMetadataSync::new(config, metadata_path).await?;
                sync.diff(&self.app_id).await?
            }
            SinglePlatform::GooglePlay => {
                let config = self.google_auth.to_config()?;
                let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
                sync.diff(&self.app_id).await?
            }
        };

        // Filter by locales if specified
        let locales = parse_locales(&self.locales)?;
        let filtered_diff = if let Some(ref filter_locales) = locales {
            let filter_codes: Vec<String> = filter_locales.iter().map(|l| l.code().to_string()).collect();
            MetadataDiff {
                changes: diff
                    .changes
                    .into_iter()
                    .filter(|c| filter_codes.contains(&c.locale))
                    .collect(),
            }
        } else {
            diff
        };

        match cli.format {
            OutputFormat::Json => {
                let output = serde_json::json!({
                    "app_id": &self.app_id,
                    "platform": format!("{:?}", self.platform),
                    "has_changes": filtered_diff.has_changes(),
                    "change_count": filtered_diff.len(),
                    "affected_locales": filtered_diff.affected_locales(),
                    "changes": filtered_diff.changes.iter().map(|c| {
                        serde_json::json!({
                            "locale": &c.locale,
                            "field": &c.field,
                            "change_type": format!("{}", c.change_type),
                            "local_value": &c.local_value,
                            "remote_value": &c.remote_value,
                        })
                    }).collect::<Vec<_>>(),
                });
                println!("{}", serde_json::to_string_pretty(&output)?);
            }
            OutputFormat::Text => {
                print_diff(cli, &filtered_diff)?;
            }
        }

        Ok(())
    }
}

/// Parse a locale string (comma-separated or "all") into Option<Vec<Locale>>
fn parse_locales(locales_str: &str) -> anyhow::Result<Option<Vec<Locale>>> {
    if locales_str.to_lowercase() == "all" {
        return Ok(None);
    }

    let locales: Result<Vec<Locale>, _> = locales_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(Locale::new)
        .collect();

    Ok(Some(locales.map_err(|e| anyhow::anyhow!("Invalid locale: {}", e))?))
}

/// Print a diff in a nice format with colors
fn print_diff(_cli: &Cli, diff: &MetadataDiff) -> anyhow::Result<()> {
    println!();

    if diff.is_empty() {
        println!(
            "{}",
            style("No differences found. Local and remote metadata are in sync.").green()
        );
        return Ok(());
    }

    println!(
        "{} ({} change(s))",
        style("Differences found").yellow().bold(),
        diff.len()
    );
    println!();

    // Group changes by locale
    let affected_locales = diff.affected_locales();

    for locale in &affected_locales {
        let locale_changes = diff.for_locale(locale);
        if locale_changes.is_empty() {
            continue;
        }

        println!("  {} {}", style("Locale:").cyan(), style(locale).bold());

        for change in locale_changes {
            let (symbol, color) = match change.change_type {
                ChangeType::Added => ("+", console::Color::Green),
                ChangeType::Modified => ("~", console::Color::Yellow),
                ChangeType::Removed => ("-", console::Color::Red),
            };

            println!(
                "    {} {} {}",
                style(symbol).fg(color).bold(),
                style(&change.field).bold(),
                style(format!("({})", change.change_type)).dim()
            );

            // Show truncated preview of values
            match change.change_type {
                ChangeType::Added => {
                    if let Some(ref value) = change.local_value {
                        let preview = truncate_str(value, 60);
                        println!(
                            "      {} {}",
                            style("local:").fg(console::Color::Green),
                            preview
                        );
                    }
                }
                ChangeType::Removed => {
                    if let Some(ref value) = change.remote_value {
                        let preview = truncate_str(value, 60);
                        println!(
                            "      {} {}",
                            style("remote:").fg(console::Color::Red),
                            preview
                        );
                    }
                }
                ChangeType::Modified => {
                    if let Some(ref remote) = change.remote_value {
                        let preview = truncate_str(remote, 60);
                        println!(
                            "      {} {}",
                            style("remote:").fg(console::Color::Red),
                            preview
                        );
                    }
                    if let Some(ref local) = change.local_value {
                        let preview = truncate_str(local, 60);
                        println!(
                            "      {} {}",
                            style("local:").fg(console::Color::Green),
                            preview
                        );
                    }
                }
            }
        }
        println!();
    }

    // Summary
    let added = diff.by_type(ChangeType::Added).len();
    let modified = diff.by_type(ChangeType::Modified).len();
    let removed = diff.by_type(ChangeType::Removed).len();

    println!(
        "Summary: {} added, {} modified, {} removed",
        style(added).green(),
        style(modified).yellow(),
        style(removed).red()
    );

    Ok(())
}

/// Truncate a string and add ellipsis if too long
fn truncate_str(s: &str, max_len: usize) -> String {
    // Replace newlines with spaces for preview
    let s = s.replace('\n', " ").replace('\r', "");
    let s = s.trim();

    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Find the next available screenshot number in a directory
async fn find_next_screenshot_number(dir: &std::path::Path) -> std::io::Result<u32> {
    if !dir.exists() {
        return Ok(1);
    }

    let mut max_number = 0u32;
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(num) = stem.parse::<u32>() {
                    max_number = max_number.max(num);
                }
            }
        }
    }

    Ok(max_number + 1)
}

/// Re-number screenshots in a directory to maintain sequential order
async fn renumber_screenshots(dir: &std::path::Path) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    // Collect all image files
    let mut files = list_image_files(dir).await?;
    files.sort();

    // Create a temporary mapping
    let temp_dir = dir.join(".temp_renumber");
    tokio::fs::create_dir_all(&temp_dir).await?;

    // Move files to temp with new names
    for (index, file_path) in files.iter().enumerate() {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let new_name = format!("{:02}.{}", index + 1, extension);
        let temp_path = temp_dir.join(&new_name);
        tokio::fs::rename(&file_path, &temp_path).await?;
    }

    // Move files back from temp
    let mut temp_entries = tokio::fs::read_dir(&temp_dir).await?;
    while let Some(entry) = temp_entries.next_entry().await? {
        let temp_path = entry.path();
        if temp_path.is_file() {
            let file_name = temp_path.file_name().unwrap();
            let dest_path = dir.join(file_name);
            tokio::fs::rename(&temp_path, &dest_path).await?;
        }
    }

    // Remove temp directory
    tokio::fs::remove_dir(&temp_dir).await?;

    Ok(())
}

/// List subdirectories in a directory
async fn list_subdirectories(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut dirs = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_dir() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                if !name.starts_with('.') {
                    dirs.push(name.to_string());
                }
            }
        }
    }

    dirs.sort();
    Ok(dirs)
}

/// List image files in a directory
async fn list_image_files(dir: &std::path::Path) -> std::io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let mut entries = tokio::fs::read_dir(dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg" {
                    files.push(path);
                }
            }
        }
    }

    files.sort();
    Ok(files)
}
