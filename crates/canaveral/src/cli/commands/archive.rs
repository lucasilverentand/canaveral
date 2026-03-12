//! Archive command - Archive and export iOS/macOS apps
//!
//! Provides a dedicated command for the Xcode archive/export workflow:
//!   1. xcodebuild archive -> .xcarchive
//!   2. xcodebuild -exportArchive -> .ipa
//!   3. Optionally upload to TestFlight

use std::path::PathBuf;

use clap::{Args, ValueEnum};
use console::style;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_frameworks::{
    context::{BuildContext, BuildProfile, SigningConfig},
    traits::Platform,
    Orchestrator, OrchestratorConfig, OutputFormat as FrameworkOutputFormat,
};

use crate::cli::output::Ui;
use crate::cli::{Cli, OutputFormat};

/// Archive an iOS/macOS app (Xcode archive + export)
#[derive(Debug, Args)]
pub struct ArchiveCommand {
    /// Xcode scheme (auto-detected if not specified)
    #[arg(short, long)]
    pub scheme: Option<String>,

    /// Build configuration (default: Release)
    #[arg(short, long, default_value = "Release")]
    pub configuration: String,

    /// Output directory for archive/export artifacts
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Also export to .ipa after archiving
    #[arg(long)]
    pub export: bool,

    /// Export method (only with --export)
    #[arg(long, value_enum, default_value = "app-store")]
    pub export_method: ExportMethod,

    /// Apple Developer Team ID
    #[arg(long, env = "APPLE_TEAM_ID")]
    pub team_id: Option<String>,

    /// Upload to TestFlight after export
    #[arg(long)]
    pub upload: bool,

    /// Custom derived data path
    #[arg(long)]
    pub derived_data: Option<PathBuf>,

    /// Dry run - show what would happen without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Skip code signing (build with CODE_SIGN_IDENTITY="-")
    ///
    /// Useful for open-source CI where signing credentials aren't available.
    /// The build will succeed but won't produce a distributable artifact.
    #[arg(long)]
    pub skip_signing: bool,

    /// Skip prerequisite checks
    #[arg(long)]
    pub skip_checks: bool,

    /// Extra arguments to pass to xcodebuild
    #[arg(last = true)]
    pub extra_args: Vec<String>,
}

/// IPA export method
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ExportMethod {
    /// App Store distribution
    #[value(name = "app-store")]
    AppStore,
    /// Ad-hoc distribution
    #[value(name = "ad-hoc")]
    AdHoc,
    /// Development distribution
    Development,
    /// Enterprise distribution
    Enterprise,
}

impl ExportMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppStore => "app-store",
            Self::AdHoc => "ad-hoc",
            Self::Development => "development",
            Self::Enterprise => "enterprise",
        }
    }
}

impl ArchiveCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(
            scheme = ?self.scheme,
            configuration = %self.configuration,
            export = self.export,
            upload = self.upload,
            dry_run = self.dry_run,
            "executing archive command"
        );
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Resolve scheme: CLI flag > config > auto-detect later
        let scheme = self.scheme.clone().or_else(|| config.ios.scheme.clone());

        // Resolve team ID: CLI flag > config
        let team_id = self
            .team_id
            .clone()
            .or_else(|| config.ios.team_id.clone())
            .or_else(|| config.ios.signing.development_team.clone());

        // Resolve export method: CLI flag is always used (has a default)
        let export_method = self.export_method.as_str();

        // Show header
        if ui.is_text() {
            ui.blank();
            ui.header("Archiving iOS app...");
            if let Some(ref s) = scheme {
                ui.key_value("Scheme", &style(s).cyan().to_string());
            }
            ui.key_value(
                "Configuration",
                &style(&self.configuration).cyan().to_string(),
            );
            if let Some(ref t) = team_id {
                ui.key_value("Team ID", &style(t).cyan().to_string());
            }
            if self.export {
                ui.key_value("Export", &style(export_method).cyan().to_string());
            }
            if self.upload {
                ui.key_value("Upload", &style("TestFlight").cyan().to_string());
            }
            if self.dry_run {
                ui.warning("DRY RUN");
            }
            ui.blank();
        }

        // Build context for the framework adapter
        let profile = if self.configuration.eq_ignore_ascii_case("debug") {
            BuildProfile::Debug
        } else {
            BuildProfile::Release
        };

        let mut ctx = BuildContext::new(&cwd, Platform::Ios)
            .with_profile(profile)
            .with_dry_run(self.dry_run)
            .with_ci(std::env::var("CI").is_ok());

        // Pass scheme to framework config
        if let Some(ref s) = scheme {
            ctx = ctx.with_config("scheme", serde_json::json!(s));
        }

        // Pass archive-specific config
        ctx = ctx.with_config("archive", serde_json::json!(true));
        ctx = ctx.with_config("export_method", serde_json::json!(export_method));

        if self.export {
            ctx = ctx.with_config("export", serde_json::json!(true));
        }

        // Output directory
        if let Some(ref output) = self.output {
            ctx = ctx.with_output_dir(output);
        } else if let Some(ref output) = config.ios.derived_data {
            ctx = ctx.with_output_dir(output);
        }

        // Derived data
        if let Some(ref dd) = self.derived_data {
            ctx = ctx.with_config("derived_data", serde_json::json!(dd.to_string_lossy()));
        }

        // Signing configuration
        if self.skip_signing {
            // Open-source / CI mode: disable code signing entirely
            ctx = ctx.with_config("CODE_SIGN_IDENTITY", serde_json::json!("-"));
            ctx = ctx.with_config("CODE_SIGNING_REQUIRED", serde_json::json!("NO"));
            ctx = ctx.with_config("CODE_SIGNING_ALLOWED", serde_json::json!("NO"));
            if ui.is_text() {
                ui.info("Code signing disabled (--skip-signing)");
            }
        } else {
            let signing_style = config.ios.signing.style.as_str();
            let signing = SigningConfig {
                identity: config.ios.signing.identity.clone(),
                provisioning_profile: config.ios.signing.provisioning_profile.clone(),
                team_id: team_id.clone(),
                keystore_path: None,
                key_alias: None,
                automatic: signing_style == "automatic",
            };
            ctx = ctx.with_signing(signing);
        }

        // Extra args
        if !self.extra_args.is_empty() {
            ctx = ctx.with_config("extra_args", serde_json::json!(self.extra_args));
        }

        // Export options from config
        ctx = ctx.with_config(
            "upload_symbols",
            serde_json::json!(config.ios.export.upload_symbols),
        );
        ctx = ctx.with_config(
            "compile_bitcode",
            serde_json::json!(config.ios.export.compile_bitcode),
        );

        // Bundle ID from config (needed for export options plist)
        if let Some(ref bundle_id) = config.ios.bundle_id {
            ctx = ctx.with_config("bundle_id", serde_json::json!(bundle_id));
        }

        // Dry run: show what would happen
        if self.dry_run {
            if ui.is_text() {
                ui.step("Would run: xcodebuild archive");
                if let Some(ref s) = scheme {
                    ui.step(&format!("  -scheme {}", s));
                }
                ui.step(&format!("  -configuration {}", self.configuration));
                ui.step("  -destination generic/platform=iOS");
                if self.export {
                    ui.blank();
                    ui.step("Would run: xcodebuild -exportArchive");
                    ui.step(&format!(
                        "  -exportOptionsPlist (method: {})",
                        export_method
                    ));
                }
                if self.upload {
                    ui.blank();
                    ui.step("Would upload .ipa to TestFlight via App Store Connect");
                }
            }

            if ui.is_json() {
                let output = serde_json::json!({
                    "dry_run": true,
                    "scheme": scheme,
                    "configuration": self.configuration,
                    "export": self.export,
                    "export_method": export_method,
                    "upload": self.upload,
                    "team_id": team_id,
                });
                ui.json(&output)?;
            }

            return Ok(());
        }

        // Create orchestrator and run build (which handles archive)
        let orchestrator_config = OrchestratorConfig {
            quiet: ui.is_quiet() || ui.is_json(),
            json_output: ui.is_json(),
            check_prerequisites: !self.skip_checks,
            ..Default::default()
        };

        let orchestrator = Orchestrator::with_config(orchestrator_config);

        let output_format = match cli.format {
            OutputFormat::Text => FrameworkOutputFormat::Text,
            OutputFormat::Json => FrameworkOutputFormat::Json,
        };

        let (output, exit_code) = orchestrator.build_with_output(&ctx, output_format).await;

        if exit_code != 0 {
            if ui.is_text() {
                ui.blank();
                ui.error("Archive failed");
            }
            std::process::exit(exit_code);
        }

        if ui.is_text() {
            ui.blank();
            ui.success("Archive completed successfully!");

            if !output.artifacts.is_empty() {
                ui.blank();
                ui.section("Artifacts");
                for artifact in &output.artifacts {
                    ui.step(&style(&artifact.path).cyan().to_string());
                }
            }
        }

        // Upload to TestFlight if requested
        if self.upload {
            if ui.is_text() {
                ui.blank();
                ui.header("Uploading to TestFlight...");
            }

            // Find the IPA artifact
            let ipa_path = output
                .artifacts
                .iter()
                .find(|a| a.path.ends_with(".ipa"))
                .map(|a| PathBuf::from(&a.path));

            match ipa_path {
                Some(ipa) => {
                    if ui.is_text() {
                        ui.key_value("IPA", &style(ipa.display()).cyan().to_string());
                    }

                    // The actual upload would go through canaveral-stores
                    // For now, show what would happen
                    ui.info("TestFlight upload requires App Store Connect credentials.");
                    ui.hint("Set APP_STORE_CONNECT_KEY_ID, APP_STORE_CONNECT_ISSUER_ID, and APP_STORE_CONNECT_KEY environment variables.");
                    ui.hint("Or use: canaveral testflight upload <ipa-path>");
                }
                None => {
                    ui.warning(
                        "No .ipa artifact found. Run with --export to generate an IPA for upload.",
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_method_as_str() {
        assert_eq!(ExportMethod::AppStore.as_str(), "app-store");
        assert_eq!(ExportMethod::AdHoc.as_str(), "ad-hoc");
        assert_eq!(ExportMethod::Development.as_str(), "development");
        assert_eq!(ExportMethod::Enterprise.as_str(), "enterprise");
    }

    #[test]
    fn test_archive_command_defaults() {
        use clap::Parser;

        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            archive: ArchiveCommand,
        }

        let cli = TestCli::try_parse_from(["test"]).unwrap();
        assert_eq!(cli.archive.configuration, "Release");
        assert!(!cli.archive.export);
        assert!(!cli.archive.upload);
        assert!(!cli.archive.dry_run);
        assert!(cli.archive.scheme.is_none());
    }
}
