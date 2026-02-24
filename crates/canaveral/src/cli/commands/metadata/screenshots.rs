//! metadata screenshots subcommand

use clap::{Args, Subcommand, ValueEnum};
use console::style;
use std::path::PathBuf;

use canaveral_metadata::{FastlaneStorage, Locale, MetadataStorage};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{list_image_files, list_subdirectories, SinglePlatform};

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
    Add(ScreenshotsAddArgs),
    /// Remove a screenshot
    Remove(ScreenshotsRemoveArgs),
    /// List screenshots
    List(ScreenshotsListArgs),
    /// Validate screenshot dimensions
    Validate(ScreenshotsValidateArgs),
}

pub async fn execute(cmd: &ScreenshotsCommand, cli: &Cli) -> anyhow::Result<()> {
    match &cmd.command {
        ScreenshotsSubcommand::Add(args) => execute_add(args, cli).await,
        ScreenshotsSubcommand::Remove(args) => execute_remove(args, cli).await,
        ScreenshotsSubcommand::List(args) => execute_list(args, cli).await,
        ScreenshotsSubcommand::Validate(args) => execute_validate(args, cli).await,
    }
}

// ── Device types ────────────────────────────────────────────────────

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
enum DeviceType {
    Apple(AppleDeviceType),
    GooglePlay(GooglePlayDeviceType),
}

impl DeviceType {
    fn as_dir_name(&self) -> &'static str {
        match self {
            DeviceType::Apple(d) => d.as_dir_name(),
            DeviceType::GooglePlay(d) => d.as_dir_name(),
        }
    }
}

// ── Args structs ────────────────────────────────────────────────────

/// Add a screenshot
#[derive(Debug, Args)]
pub struct ScreenshotsAddArgs {
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
pub struct ScreenshotsRemoveArgs {
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
pub struct ScreenshotsListArgs {
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
pub struct ScreenshotsValidateArgs {
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

// ── Execute functions ───────────────────────────────────────────────

async fn execute_add(cmd: &ScreenshotsAddArgs, cli: &Cli) -> anyhow::Result<()> {
    use canaveral_metadata::{
        read_image_dimensions, validate_apple_screenshot_file, validate_google_play_screenshot_file,
    };

    let ui = Ui::new(cli);

    let locale = Locale::new(&cmd.locale)
        .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &cmd.locale, e))?;

    let device_type = match cmd.platform {
        SinglePlatform::Apple => {
            let device = cmd
                .apple_device
                .ok_or_else(|| anyhow::anyhow!("--apple-device is required for Apple platform"))?;
            DeviceType::Apple(device)
        }
        SinglePlatform::GooglePlay => {
            let device = cmd.google_device.ok_or_else(|| {
                anyhow::anyhow!("--google-device is required for Google Play platform")
            })?;
            DeviceType::GooglePlay(device)
        }
    };

    if !cmd.file.exists() {
        anyhow::bail!("Screenshot file does not exist: {:?}", cmd.file);
    }
    if !cmd.file.is_file() {
        anyhow::bail!("Path is not a file: {:?}", cmd.file);
    }

    let storage = FastlaneStorage::new(&cmd.path);

    let app_path = match cmd.platform {
        SinglePlatform::Apple => storage.apple_path(&cmd.app_id),
        SinglePlatform::GooglePlay => storage.google_play_path(&cmd.app_id),
    };

    if !app_path.exists() {
        anyhow::bail!(
            "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
            &cmd.app_id
        );
    }

    let screenshots_dir = app_path
        .join("screenshots")
        .join(locale.code())
        .join(device_type.as_dir_name());

    tokio::fs::create_dir_all(&screenshots_dir).await?;

    let dimensions = read_image_dimensions(&cmd.file)
        .map_err(|e| anyhow::anyhow!("Failed to read image dimensions: {}", e))?;

    let validation_result = match cmd.platform {
        SinglePlatform::Apple => {
            validate_apple_screenshot_file(&cmd.file, device_type.as_dir_name())
        }
        SinglePlatform::GooglePlay => {
            validate_google_play_screenshot_file(&cmd.file, device_type.as_dir_name())
        }
    };

    if !validation_result.is_valid() {
        for error in validation_result.errors() {
            ui.warning(&error.message);
            if let Some(ref suggestion) = error.suggestion {
                ui.hint(suggestion);
            }
        }
    }

    let next_number = find_next_screenshot_number(&screenshots_dir).await?;

    let extension = cmd
        .file
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");

    let dest_filename = format!("{:02}.{}", next_number, extension);
    let dest_path = screenshots_dir.join(&dest_filename);

    tokio::fs::copy(&cmd.file, &dest_path).await?;

    ui.success(&format!("Added screenshot to {}", dest_path.display()));
    ui.key_value(
        "Dimensions",
        &format!("{}x{}", dimensions.width, dimensions.height),
    );
    ui.key_value("Locale", &locale.code());
    ui.key_value("Device", device_type.as_dir_name());

    ui.json(&serde_json::json!({
        "success": true,
        "source": cmd.file.display().to_string(),
        "destination": dest_path.display().to_string(),
        "filename": dest_filename,
        "dimensions": {
            "width": dimensions.width,
            "height": dimensions.height,
        },
        "locale": locale.code(),
        "device": device_type.as_dir_name(),
        "validation_passed": validation_result.is_valid(),
    }))?;

    Ok(())
}

async fn execute_remove(cmd: &ScreenshotsRemoveArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let locale = Locale::new(&cmd.locale)
        .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &cmd.locale, e))?;

    let device_type = match cmd.platform {
        SinglePlatform::Apple => {
            let device = cmd
                .apple_device
                .ok_or_else(|| anyhow::anyhow!("--apple-device is required for Apple platform"))?;
            DeviceType::Apple(device)
        }
        SinglePlatform::GooglePlay => {
            let device = cmd.google_device.ok_or_else(|| {
                anyhow::anyhow!("--google-device is required for Google Play platform")
            })?;
            DeviceType::GooglePlay(device)
        }
    };

    let storage = FastlaneStorage::new(&cmd.path);

    let app_path = match cmd.platform {
        SinglePlatform::Apple => storage.apple_path(&cmd.app_id),
        SinglePlatform::GooglePlay => storage.google_play_path(&cmd.app_id),
    };

    if !app_path.exists() {
        anyhow::bail!(
            "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
            &cmd.app_id
        );
    }

    let screenshots_dir = app_path
        .join("screenshots")
        .join(locale.code())
        .join(device_type.as_dir_name());

    let file_to_remove = screenshots_dir.join(&cmd.filename);

    if !file_to_remove.exists() {
        anyhow::bail!("Screenshot file not found: {:?}", file_to_remove);
    }

    tokio::fs::remove_file(&file_to_remove).await?;

    ui.warning(&format!("Removed screenshot: {}", &cmd.filename));

    renumber_screenshots(&screenshots_dir).await?;

    ui.hint("Re-numbered remaining screenshots.");

    ui.json(&serde_json::json!({
        "success": true,
        "removed": cmd.filename,
        "locale": locale.code(),
        "device": device_type.as_dir_name(),
    }))?;

    Ok(())
}

async fn execute_list(cmd: &ScreenshotsListArgs, cli: &Cli) -> anyhow::Result<()> {
    use canaveral_metadata::read_image_dimensions;

    let ui = Ui::new(cli);

    let storage = FastlaneStorage::new(&cmd.path);

    let app_path = match cmd.platform {
        SinglePlatform::Apple => storage.apple_path(&cmd.app_id),
        SinglePlatform::GooglePlay => storage.google_play_path(&cmd.app_id),
    };

    if !app_path.exists() {
        anyhow::bail!(
            "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
            &cmd.app_id
        );
    }

    let screenshots_base = app_path.join("screenshots");

    if !screenshots_base.exists() {
        ui.warning("No screenshots directory found.");
        return Ok(());
    }

    let locales: Vec<String> = if let Some(ref locale_code) = cmd.locale {
        vec![locale_code.clone()]
    } else {
        list_subdirectories(&screenshots_base).await?
    };

    let device_types: Vec<&str> = match cmd.platform {
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

    ui.json(&serde_json::json!({
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "locales": all_locales,
    }))?;

    if ui.is_text() {
        if all_locales.is_empty() {
            println!("{}", style("No screenshots found.").yellow());
        } else {
            println!(
                "{} for {}",
                style("Screenshots").green().bold(),
                style(&cmd.app_id).bold()
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

    Ok(())
}

async fn execute_validate(cmd: &ScreenshotsValidateArgs, cli: &Cli) -> anyhow::Result<()> {
    use canaveral_metadata::{
        read_image_dimensions, validate_apple_screenshot_file,
        validate_google_play_screenshot_file, ValidationResult,
    };

    let ui = Ui::new(cli);

    let storage = FastlaneStorage::new(&cmd.path);

    let app_path = match cmd.platform {
        SinglePlatform::Apple => storage.apple_path(&cmd.app_id),
        SinglePlatform::GooglePlay => storage.google_play_path(&cmd.app_id),
    };

    if !app_path.exists() {
        anyhow::bail!(
            "App metadata not found for '{}'. Run 'canaveral metadata init' first.",
            &cmd.app_id
        );
    }

    let screenshots_base = app_path.join("screenshots");

    if !screenshots_base.exists() {
        ui.warning("No screenshots directory found.");
        return Ok(());
    }

    let locales: Vec<String> = if let Some(ref locale_code) = cmd.locale {
        vec![locale_code.clone()]
    } else {
        list_subdirectories(&screenshots_base).await?
    };

    let device_types: Vec<&str> = match cmd.platform {
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

    ui.step(&format!("Validating screenshots for {}", &cmd.app_id));

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

                let validation_result = match cmd.platform {
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

                    if ui.is_text() {
                        let severity_style = match issue.severity {
                            canaveral_metadata::Severity::Error => style("ERROR").red().bold(),
                            canaveral_metadata::Severity::Warning => style("WARN").yellow(),
                            canaveral_metadata::Severity::Info => style("INFO").blue(),
                        };

                        println!(
                            "  {} {}/{}/{}: {}",
                            severity_style, locale_code, device_type, filename, issue.message
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

    ui.json(&serde_json::json!({
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "validated_count": validated_count,
        "valid": overall_result.is_valid(),
        "error_count": overall_result.error_count(),
        "warning_count": overall_result.warning_count(),
        "issues": issues_json,
    }))?;

    if ui.is_text() {
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

    if !overall_result.is_valid() {
        anyhow::bail!(
            "Screenshot validation failed with {} error(s)",
            overall_result.error_count()
        );
    }

    Ok(())
}

// ── Screenshot helpers ──────────────────────────────────────────────

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

async fn renumber_screenshots(dir: &std::path::Path) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }

    let mut files = list_image_files(dir).await?;
    files.sort();

    let temp_dir = dir.join(".temp_renumber");
    tokio::fs::create_dir_all(&temp_dir).await?;

    for (index, file_path) in files.iter().enumerate() {
        let extension = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");
        let new_name = format!("{:02}.{}", index + 1, extension);
        let temp_path = temp_dir.join(&new_name);
        tokio::fs::rename(&file_path, &temp_path).await?;
    }

    let mut temp_entries = tokio::fs::read_dir(&temp_dir).await?;
    while let Some(entry) = temp_entries.next_entry().await? {
        let temp_path = entry.path();
        if temp_path.is_file() {
            let file_name = temp_path.file_name().unwrap();
            let dest_path = dir.join(file_name);
            tokio::fs::rename(&temp_path, &dest_path).await?;
        }
    }

    tokio::fs::remove_dir(&temp_dir).await?;

    Ok(())
}
