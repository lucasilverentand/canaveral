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

    /// Add a new localization
    AddLocale(AddLocaleCommand),

    /// Remove a localization
    RemoveLocale(RemoveLocaleCommand),

    /// List available localizations
    ListLocales(ListLocalesCommand),

    /// Screenshot management commands
    Screenshots(ScreenshotsCommand),
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
            MetadataSubcommand::AddLocale(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::RemoveLocale(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::ListLocales(cmd) => rt.block_on(cmd.execute(cli)),
            MetadataSubcommand::Screenshots(cmd) => rt.block_on(cmd.execute(cli)),
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
