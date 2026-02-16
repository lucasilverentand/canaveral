//! Build command - Build projects for various platforms using framework adapters

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use console::style;
use tracing::info;

use canaveral_frameworks::{
    BuildContext, Orchestrator, OrchestratorConfig,
    OutputFormat as FrameworkOutputFormat,
    context::BuildProfile,
    traits::Platform,
};

use crate::cli::{Cli, OutputFormat};

/// Build a project for specified platform(s)
#[derive(Debug, Args)]
pub struct BuildCommand {
    /// Target platform
    #[arg(short, long, value_enum)]
    pub platform: PlatformArg,

    /// Build profile
    #[arg(long, value_enum, default_value = "release")]
    pub profile: ProfileArg,

    /// Build flavor/variant (e.g., "production", "staging")
    #[arg(long)]
    pub flavor: Option<String>,

    /// Force use of a specific framework adapter
    #[arg(long)]
    pub framework: Option<FrameworkArg>,

    /// Output directory for build artifacts
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Version to embed in the build
    #[arg(long = "build-version", value_name = "VERSION")]
    pub build_version: Option<String>,

    /// Build number to embed
    #[arg(long)]
    pub build_number: Option<u64>,

    /// Dry run - validate but don't actually build
    #[arg(long)]
    pub dry_run: bool,

    /// Skip prerequisite checks
    #[arg(long)]
    pub skip_checks: bool,

    /// Code signing identity (iOS/macOS)
    #[arg(long)]
    pub signing_identity: Option<String>,

    /// Provisioning profile (iOS)
    #[arg(long)]
    pub provisioning_profile: Option<String>,

    /// Team ID (iOS/macOS)
    #[arg(long)]
    pub team_id: Option<String>,

    /// Keystore path (Android)
    #[arg(long)]
    pub keystore: Option<PathBuf>,

    /// Key alias (Android)
    #[arg(long)]
    pub key_alias: Option<String>,

    /// Extra arguments to pass to the underlying build tool
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

/// Target platform argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PlatformArg {
    /// iOS (iPhone, iPad)
    Ios,
    /// Android
    Android,
    /// macOS desktop
    #[value(name = "macos")]
    MacOs,
    /// Windows desktop
    Windows,
    /// Linux desktop
    Linux,
    /// Web (browser)
    Web,
}

impl From<PlatformArg> for Platform {
    fn from(arg: PlatformArg) -> Self {
        match arg {
            PlatformArg::Ios => Platform::Ios,
            PlatformArg::Android => Platform::Android,
            PlatformArg::MacOs => Platform::MacOs,
            PlatformArg::Windows => Platform::Windows,
            PlatformArg::Linux => Platform::Linux,
            PlatformArg::Web => Platform::Web,
        }
    }
}

/// Build profile argument
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum ProfileArg {
    /// Debug build (fast compilation, no optimization)
    Debug,
    /// Release build (optimized, stripped)
    #[default]
    Release,
    /// Profile build (release with debug symbols)
    Profile,
}

impl From<ProfileArg> for BuildProfile {
    fn from(arg: ProfileArg) -> Self {
        match arg {
            ProfileArg::Debug => BuildProfile::Debug,
            ProfileArg::Release => BuildProfile::Release,
            ProfileArg::Profile => BuildProfile::Profile,
        }
    }
}

/// Framework adapter argument
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum FrameworkArg {
    /// Flutter framework
    Flutter,
    /// Expo (React Native)
    Expo,
    /// React Native (bare workflow)
    #[value(name = "react-native")]
    ReactNative,
    /// Tauri (desktop apps)
    Tauri,
    /// Native iOS (Xcode project)
    #[value(name = "native-ios")]
    NativeIos,
    /// Native Android (Gradle project)
    #[value(name = "native-android")]
    NativeAndroid,
}

impl FrameworkArg {
    fn as_adapter_id(&self) -> &'static str {
        match self {
            Self::Flutter => "flutter",
            Self::Expo => "expo",
            Self::ReactNative => "react-native",
            Self::Tauri => "tauri",
            Self::NativeIos => "native-ios",
            Self::NativeAndroid => "native-android",
        }
    }
}

impl BuildCommand {
    /// Execute the build command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(platform = ?self.platform, profile = ?self.profile, dry_run = self.dry_run, flavor = ?self.flavor, "executing build command");
        // Run async operation in tokio runtime
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let platform: Platform = self.platform.into();

        // Create build context
        let mut ctx = BuildContext::new(&cwd, platform)
            .with_profile(self.profile.into())
            .with_dry_run(self.dry_run)
            .with_ci(std::env::var("CI").is_ok());

        // Apply options
        if let Some(ref output_dir) = self.output {
            ctx = ctx.with_output_dir(output_dir);
        }

        if let Some(ref flavor) = self.flavor {
            ctx = ctx.with_flavor(flavor);
        }

        if let Some(ref version) = self.build_version {
            ctx = ctx.with_version(version);
        }

        if let Some(build_number) = self.build_number {
            ctx = ctx.with_build_number(build_number);
        }

        // Code signing configuration
        if self.signing_identity.is_some()
            || self.provisioning_profile.is_some()
            || self.team_id.is_some()
            || self.keystore.is_some()
            || self.key_alias.is_some()
        {
            use canaveral_frameworks::context::SigningConfig;

            let signing = SigningConfig {
                identity: self.signing_identity.clone(),
                provisioning_profile: self.provisioning_profile.clone(),
                team_id: self.team_id.clone(),
                keystore_path: self.keystore.clone(),
                key_alias: self.key_alias.clone(),
                automatic: false,
            };
            ctx = ctx.with_signing(signing);
        }

        // Pass extra args to framework config
        if !self.extra_args.is_empty() {
            ctx = ctx.with_config(
                "extra_args",
                serde_json::json!(self.extra_args),
            );
        }

        // Create orchestrator with config
        let orchestrator_config = OrchestratorConfig {
            quiet: cli.quiet,
            json_output: cli.format == OutputFormat::Json,
            check_prerequisites: !self.skip_checks,
            ..Default::default()
        };

        let orchestrator = Orchestrator::with_config(orchestrator_config);

        // If framework is specified, validate it
        if let Some(ref framework) = self.framework {
            let adapter_id = framework.as_adapter_id();
            if !cli.quiet && cli.format == OutputFormat::Text {
                println!(
                    "{} Using framework: {}",
                    style("→").cyan(),
                    style(adapter_id).bold()
                );
            }
        }

        // Print build info
        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!("{}", style("Building project...").bold());
            println!("  Platform: {}", style(platform.as_str()).cyan());
            println!("  Profile:  {}", style(self.profile.as_str()).cyan());
            if let Some(ref flavor) = self.flavor {
                println!("  Flavor:   {}", style(flavor).cyan());
            }
            if let Some(ref version) = self.build_version {
                println!("  Version:  {}", style(version).cyan());
            }
            if self.dry_run {
                println!("  {}", style("DRY RUN").yellow().bold());
            }
            println!();
        }

        // Execute build
        let output_format = match cli.format {
            OutputFormat::Text => FrameworkOutputFormat::Text,
            OutputFormat::Json => FrameworkOutputFormat::Json,
        };

        let (output, exit_code) = orchestrator.build_with_output(&ctx, output_format).await;

        // Handle result
        if exit_code != 0 {
            if cli.format == OutputFormat::Text && !cli.quiet {
                println!();
                println!(
                    "{} Build failed",
                    style("✗").red().bold()
                );
            }
            std::process::exit(exit_code);
        }

        // Print success message for text output
        if cli.format == OutputFormat::Text && !cli.quiet {
            println!();
            println!(
                "{} Build completed successfully!",
                style("✓").green().bold()
            );

            // Print artifact paths
            if !output.artifacts.is_empty() {
                println!();
                println!("{}:", style("Artifacts").bold());
                for artifact in &output.artifacts {
                    println!("  • {}", style(&artifact.path).cyan());
                }
            }

            // Print CI outputs
            if std::env::var("GITHUB_OUTPUT").is_ok() {
                println!();
                println!("{}:", style("GitHub Actions outputs set").dim());
                for (key, value) in &output.outputs {
                    println!("  {}={}", style(key).dim(), value);
                }
            }

            // Print GitLab CI variables
            if std::env::var("CI_PROJECT_DIR").is_ok() {
                println!();
                println!("{}:", style("GitLab CI variables").dim());
                for (key, value) in &output.outputs {
                    let env_var = format!("CANAVERAL_{}", key.to_uppercase());
                    println!("  {}={}", style(env_var).dim(), value);
                }
            }
        }

        Ok(())
    }
}

impl ProfileArg {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::Profile => "profile",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_conversion() {
        assert!(matches!(Platform::from(PlatformArg::Ios), Platform::Ios));
        assert!(matches!(Platform::from(PlatformArg::Android), Platform::Android));
        assert!(matches!(Platform::from(PlatformArg::MacOs), Platform::MacOs));
    }

    #[test]
    fn test_profile_conversion() {
        assert!(matches!(BuildProfile::from(ProfileArg::Debug), BuildProfile::Debug));
        assert!(matches!(BuildProfile::from(ProfileArg::Release), BuildProfile::Release));
        assert!(matches!(BuildProfile::from(ProfileArg::Profile), BuildProfile::Profile));
    }

    #[test]
    fn test_framework_adapter_id() {
        assert_eq!(FrameworkArg::Flutter.as_adapter_id(), "flutter");
        assert_eq!(FrameworkArg::Expo.as_adapter_id(), "expo");
        assert_eq!(FrameworkArg::NativeIos.as_adapter_id(), "native-ios");
    }
}
