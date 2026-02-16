//! Screenshot command - Capture and frame app store screenshots

use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};
use console::style;
use tracing::info;

use canaveral_frameworks::{
    screenshots::{
        capture::ScreenshotConfig,
        devices::{presets, DeviceManager},
        framing::{presets as frame_presets, FrameConfig, ScreenshotFramer},
    },
    DeviceConfig, ScreenConfig, ScreenshotSession,
};

use crate::cli::{Cli, OutputFormat};

/// Screenshot capture and framing
#[derive(Debug, Args)]
pub struct ScreenshotsCommand {
    #[command(subcommand)]
    pub command: ScreenshotsSubcommand,
}

/// Screenshot subcommands
#[derive(Debug, Subcommand)]
pub enum ScreenshotsSubcommand {
    /// Capture screenshots
    Capture(CaptureCommand),

    /// Frame screenshots with device mockups
    Frame(FrameCommand),

    /// List available devices/simulators
    Devices(DevicesCommand),

    /// Initialize screenshot configuration
    Init(InitCommand),
}

/// Capture screenshots
#[derive(Debug, Args)]
pub struct CaptureCommand {
    /// Configuration file
    #[arg(short, long, default_value = "screenshots.yaml")]
    pub config: PathBuf,

    /// Output directory
    #[arg(short, long, default_value = "screenshots")]
    pub output: PathBuf,

    /// Devices to capture on (comma-separated or preset name)
    #[arg(short, long)]
    pub devices: Option<String>,

    /// Locales to capture (comma-separated)
    #[arg(short, long, default_value = "en_US")]
    pub locales: String,

    /// App bundle ID or package name
    #[arg(long)]
    pub app_id: Option<String>,

    /// Set clean status bar (iOS only)
    #[arg(long, default_value = "true")]
    pub clean_status_bar: bool,

    /// Perform a dry run
    #[arg(long)]
    pub dry_run: bool,
}

/// Frame screenshots
#[derive(Debug, Args)]
pub struct FrameCommand {
    /// Input screenshot file or directory
    pub input: PathBuf,

    /// Output file or directory
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Frame template
    #[arg(short, long, default_value = "device")]
    pub template: TemplateArg,

    /// Frame preset
    #[arg(long)]
    pub preset: Option<PresetArg>,

    /// Title text
    #[arg(long)]
    pub title: Option<String>,

    /// Subtitle text
    #[arg(long)]
    pub subtitle: Option<String>,

    /// Background color (hex)
    #[arg(long, default_value = "#FFFFFF")]
    pub background: String,

    /// Device type for frame
    #[arg(long, default_value = "iphone")]
    pub device_type: DeviceTypeArg,
}

/// List devices
#[derive(Debug, Args)]
pub struct DevicesCommand {
    /// Platform filter
    #[arg(short, long)]
    pub platform: Option<PlatformArg>,

    /// Show only booted devices
    #[arg(long)]
    pub booted: bool,
}

/// Initialize configuration
#[derive(Debug, Args)]
pub struct InitCommand {
    /// Output file
    #[arg(short, long, default_value = "screenshots.yaml")]
    pub output: PathBuf,

    /// Platform
    #[arg(short, long, default_value = "ios")]
    pub platform: PlatformArg,

    /// Include all required App Store sizes
    #[arg(long)]
    pub app_store: bool,
}

/// Template argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum TemplateArg {
    /// Minimal (no frame)
    Minimal,
    /// Device frame
    Device,
    /// Perspective view
    Perspective,
    /// Floating with shadow
    Floating,
}

/// Preset argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PresetArg {
    /// Apple style
    Apple,
    /// Dark mode
    Dark,
    /// Gradient background
    Gradient,
    /// Minimal
    Minimal,
}

/// Platform argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PlatformArg {
    Ios,
    Android,
}

/// Device type argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DeviceTypeArg {
    Iphone,
    IphoneDynamic,
    Ipad,
    IpadPro,
    AndroidPhone,
    AndroidTablet,
}

impl From<DeviceTypeArg> for canaveral_frameworks::screenshots::devices::DeviceType {
    fn from(d: DeviceTypeArg) -> Self {
        match d {
            DeviceTypeArg::Iphone => Self::IPhone,
            DeviceTypeArg::IphoneDynamic => Self::IPhoneDynamicIsland,
            DeviceTypeArg::Ipad => Self::IPad,
            DeviceTypeArg::IpadPro => Self::IPadPro,
            DeviceTypeArg::AndroidPhone => Self::AndroidPhone,
            DeviceTypeArg::AndroidTablet => Self::AndroidTablet,
        }
    }
}

impl ScreenshotsCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let subcommand_name = match &self.command {
            ScreenshotsSubcommand::Capture(_) => "capture",
            ScreenshotsSubcommand::Frame(_) => "frame",
            ScreenshotsSubcommand::Devices(_) => "devices",
            ScreenshotsSubcommand::Init(_) => "init",
        };
        info!(subcommand = subcommand_name, "executing screenshots command");
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.command {
            ScreenshotsSubcommand::Capture(cmd) => cmd.execute(cli).await,
            ScreenshotsSubcommand::Frame(cmd) => cmd.execute(cli),
            ScreenshotsSubcommand::Devices(cmd) => cmd.execute(cli).await,
            ScreenshotsSubcommand::Init(cmd) => cmd.execute(cli),
        }
    }
}

impl CaptureCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Load or build configuration
        let config = if self.config.exists() {
            ScreenshotConfig::from_yaml(&self.config)?
        } else {
            self.build_config()?
        };

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Capturing screenshots...").bold());
            println!(
                "  Devices: {}",
                style(config.devices.len().to_string()).cyan()
            );
            println!(
                "  Locales: {}",
                style(config.locales.join(", ")).cyan()
            );
            println!(
                "  Screens: {}",
                style(config.screens.len().to_string()).cyan()
            );
            println!(
                "  Output: {}",
                style(config.output_dir.display()).cyan()
            );
            if self.dry_run {
                println!("  {}", style("(DRY RUN)").yellow().bold());
            }
            println!();
        }

        if self.dry_run {
            println!("{} Dry run complete", style("✓").green());
            return Ok(());
        }

        // Run screenshot session
        let mut session = ScreenshotSession::new(config);
        let results = session.run().await?;

        // Output results
        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&results)?);
        } else if !cli.quiet {
            let successful = results.iter().filter(|r| r.success).count();
            let failed = results.iter().filter(|r| !r.success).count();

            println!();
            if failed == 0 {
                println!(
                    "{} Captured {} screenshots successfully",
                    style("✓").green(),
                    successful
                );
            } else {
                println!(
                    "{} Captured {} screenshots, {} failed",
                    style("⚠").yellow(),
                    successful,
                    failed
                );

                for result in results.iter().filter(|r| !r.success) {
                    println!(
                        "  {} {}: {}",
                        style("✗").red(),
                        result.screen_name,
                        result.error.as_deref().unwrap_or("Unknown error")
                    );
                }
            }
        }

        Ok(())
    }

    fn build_config(&self) -> anyhow::Result<ScreenshotConfig> {
        let devices = if let Some(ref device_str) = self.devices {
            self.parse_devices(device_str)?
        } else {
            // Default to iPhone 14 Pro Max
            vec![presets::iphone_14_pro_max()]
        };

        let locales: Vec<String> = self
            .locales
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        let mut config = ScreenshotConfig::new()
            .with_devices(devices)
            .with_locales(locales)
            .with_output_dir(&self.output);

        if let Some(ref app_id) = self.app_id {
            config = config.with_app_id(app_id);
        }

        Ok(config)
    }

    fn parse_devices(&self, device_str: &str) -> anyhow::Result<Vec<DeviceConfig>> {
        match device_str.to_lowercase().as_str() {
            "iphones" | "all-iphones" => Ok(presets::all_iphones()),
            "ipads" | "all-ipads" => Ok(presets::all_ipads()),
            "iphone-16-pro-max" => Ok(vec![presets::iphone_16_pro_max()]),
            "iphone-14-pro-max" => Ok(vec![presets::iphone_14_pro_max()]),
            "iphone-8-plus" => Ok(vec![presets::iphone_8_plus()]),
            "ipad-pro-12.9" => Ok(vec![presets::ipad_pro_129()]),
            "ipad-pro-11" => Ok(vec![presets::ipad_pro_11()]),
            "pixel-7-pro" => Ok(vec![presets::pixel_7_pro()]),
            "pixel-tablet" => Ok(vec![presets::pixel_tablet()]),
            _ => {
                // Custom device spec - try to find matching simulator
                let mut manager = DeviceManager::new();
                let simulators = manager.list_ios_simulators()?;

                let matching: Vec<_> = simulators
                    .iter()
                    .filter(|s| s.name.to_lowercase().contains(&device_str.to_lowercase()))
                    .collect();

                if matching.is_empty() {
                    anyhow::bail!("No device found matching: {}", device_str);
                }

                Ok(matching
                    .into_iter()
                    .map(|s| {
                        DeviceConfig::ios(&s.name, (1290, 2796))
                            .with_simulator(&s.udid)
                    })
                    .collect())
            }
        }
    }
}

impl FrameCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Build frame config
        let config = if let Some(preset) = self.preset {
            match preset {
                PresetArg::Apple => frame_presets::apple_style(),
                PresetArg::Dark => frame_presets::dark(),
                PresetArg::Gradient => frame_presets::gradient(),
                PresetArg::Minimal => frame_presets::minimal(),
            }
        } else {
            let mut config = FrameConfig::new().with_background(&self.background);

            config.template = match self.template {
                TemplateArg::Minimal => canaveral_frameworks::FrameTemplate::Minimal,
                TemplateArg::Device => canaveral_frameworks::FrameTemplate::DeviceFrame,
                TemplateArg::Perspective => canaveral_frameworks::FrameTemplate::Perspective,
                TemplateArg::Floating => canaveral_frameworks::FrameTemplate::Floating,
            };

            if let Some(ref title) = self.title {
                config = config.with_title(title);
            }

            if let Some(ref subtitle) = self.subtitle {
                config = config.with_subtitle(subtitle);
            }

            config
        };

        let framer = ScreenshotFramer::new(config);

        // Process input
        if self.input.is_file() {
            let output = self.output.clone().unwrap_or_else(|| {
                let stem = self.input.file_stem().unwrap_or_default();
                let ext = self.input.extension().unwrap_or_default();
                self.input
                    .with_file_name(format!("{}_framed.{}", stem.to_string_lossy(), ext.to_string_lossy()))
            });

            if !cli.quiet && cli.format == OutputFormat::Text {
                println!(
                    "Framing {} -> {}",
                    style(self.input.display()).cyan(),
                    style(output.display()).cyan()
                );
            }

            framer.frame(&self.input, self.device_type.into(), &output)?;

            if !cli.quiet && cli.format == OutputFormat::Text {
                println!("{} Framed screenshot saved", style("✓").green());
            }
        } else if self.input.is_dir() {
            let output_dir = self.output.clone().unwrap_or_else(|| {
                self.input.join("framed")
            });

            std::fs::create_dir_all(&output_dir)?;

            let mut count = 0;
            for entry in std::fs::read_dir(&self.input)? {
                let entry = entry?;
                let path = entry.path();

                if path.extension().map_or(false, |e| e == "png" || e == "jpg" || e == "jpeg") {
                    let filename = path.file_name().unwrap_or_default();
                    let output = output_dir.join(filename);

                    framer.frame(&path, self.device_type.into(), &output)?;
                    count += 1;
                }
            }

            if !cli.quiet && cli.format == OutputFormat::Text {
                println!(
                    "{} Framed {} screenshots to {}",
                    style("✓").green(),
                    count,
                    style(output_dir.display()).cyan()
                );
            }
        } else {
            anyhow::bail!("Input path not found: {}", self.input.display());
        }

        Ok(())
    }
}

impl DevicesCommand {
    async fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let mut manager = DeviceManager::new();

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Available Devices").bold());
            println!();
        }

        // iOS simulators
        if self.platform.is_none() || matches!(self.platform, Some(PlatformArg::Ios)) {
            let simulators = manager.list_ios_simulators()?;

            if cli.format == OutputFormat::Json {
                println!("{}", serde_json::to_string_pretty(&simulators)?);
            } else if !cli.quiet {
                println!("{}", style("iOS Simulators:").bold().underlined());

                let filtered: Vec<_> = if self.booted {
                    simulators.iter().filter(|s| s.state == "Booted").collect()
                } else {
                    simulators.iter().collect()
                };

                for sim in filtered {
                    let state_style = if sim.state == "Booted" {
                        style(&sim.state).green()
                    } else {
                        style(&sim.state).dim()
                    };

                    println!(
                        "  {} ({}) [{}]",
                        style(&sim.name).cyan(),
                        style(&sim.udid).dim(),
                        state_style
                    );
                }
                println!();
            }
        }

        // Android emulators
        if self.platform.is_none() || matches!(self.platform, Some(PlatformArg::Android)) {
            let emulators = manager.list_android_emulators()?;

            if cli.format == OutputFormat::Json && self.platform.is_some() {
                println!("{}", serde_json::to_string_pretty(&emulators)?);
            } else if !cli.quiet && cli.format == OutputFormat::Text {
                println!("{}", style("Android Emulators:").bold().underlined());

                if emulators.is_empty() {
                    println!("  {}", style("No emulators found").dim());
                } else {
                    for emu in emulators {
                        println!("  {} ({})", style(&emu.name).cyan(), style(&emu.abi).dim());
                    }
                }
                println!();
            }
        }

        // Presets
        if !cli.quiet && cli.format == OutputFormat::Text {
            println!("{}", style("Device Presets:").bold().underlined());
            println!(
                "  {} - All required iPhone sizes",
                style("iphones").cyan()
            );
            println!(
                "  {} - All required iPad sizes",
                style("ipads").cyan()
            );
            println!(
                "  {} - iPhone 16 Pro Max (6.9\")",
                style("iphone-16-pro-max").cyan()
            );
            println!(
                "  {} - iPhone 14 Pro Max (6.7\")",
                style("iphone-14-pro-max").cyan()
            );
            println!(
                "  {} - iPhone 8 Plus (5.5\")",
                style("iphone-8-plus").cyan()
            );
            println!(
                "  {} - iPad Pro 12.9\"",
                style("ipad-pro-12.9").cyan()
            );
            println!(
                "  {} - iPad Pro 11\"",
                style("ipad-pro-11").cyan()
            );
            println!();
        }

        Ok(())
    }
}

impl InitCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        // Build default config
        let devices = match self.platform {
            PlatformArg::Ios => {
                if self.app_store {
                    let mut devices = presets::all_iphones();
                    devices.extend(presets::all_ipads());
                    devices
                } else {
                    vec![presets::iphone_14_pro_max()]
                }
            }
            PlatformArg::Android => {
                vec![presets::pixel_7_pro()]
            }
        };

        let config = ScreenshotConfig::new()
            .with_devices(devices)
            .with_locales(vec!["en_US".to_string()])
            .with_output_dir("screenshots")
            .with_screen(ScreenConfig::new("home", "/").with_wait(2000))
            .with_screen(ScreenConfig::new("feature1", "/feature1").with_wait(1500))
            .with_screen(ScreenConfig::new("feature2", "/feature2").with_wait(1500));

        // Write config
        config.to_yaml(&self.output)?;

        if !cli.quiet && cli.format == OutputFormat::Text {
            println!(
                "{} Created screenshot config at {}",
                style("✓").green(),
                style(self.output.display()).cyan()
            );
            println!();
            println!("Edit the config file to customize:");
            println!("  - Add screen routes to capture");
            println!("  - Add locales for localization");
            println!("  - Configure devices for different sizes");
            println!();
            println!(
                "Then run: {}",
                style("canaveral screenshots capture").cyan()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_type_conversion() {
        let iphone: canaveral_frameworks::screenshots::devices::DeviceType =
            DeviceTypeArg::Iphone.into();
        assert!(matches!(
            iphone,
            canaveral_frameworks::screenshots::devices::DeviceType::IPhone
        ));
    }
}
