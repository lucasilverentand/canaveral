//! metadata init subcommand

use clap::Args;
use std::path::PathBuf;

use canaveral_metadata::{FastlaneStorage, Locale, MetadataStorage, Platform};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{MetadataFormat, TargetPlatform};

/// Initialize metadata directory structure
#[derive(Debug, Args)]
pub struct InitArgs {
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

pub async fn execute(cmd: &InitArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let locales: Vec<Locale> = cmd
        .locales
        .iter()
        .map(|s| Locale::new(s))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| anyhow::anyhow!("Invalid locale: {}", e))?;

    if locales.is_empty() {
        anyhow::bail!("At least one locale must be specified");
    }

    let storage = FastlaneStorage::new(&cmd.path);

    ui.step("Initializing metadata directory structure");
    ui.key_value("App ID", &cmd.app_id);
    ui.key_value("Path", &cmd.path.display().to_string());
    ui.key_value(
        "Locales",
        &locales
            .iter()
            .map(|l| l.code())
            .collect::<Vec<_>>()
            .join(", "),
    );

    match cmd.platform {
        TargetPlatform::Apple => {
            storage.init(Platform::Apple, &cmd.app_id, &locales).await?;
            ui.key_value("Platform", "Apple App Store");
        }
        TargetPlatform::GooglePlay => {
            storage
                .init(Platform::GooglePlay, &cmd.app_id, &locales)
                .await?;
            ui.key_value("Platform", "Google Play Store");
        }
        TargetPlatform::Both => {
            storage.init(Platform::Apple, &cmd.app_id, &locales).await?;
            storage
                .init(Platform::GooglePlay, &cmd.app_id, &locales)
                .await?;
            ui.key_value("Platform", "Apple App Store + Google Play Store");
        }
    }

    ui.json(&serde_json::json!({
        "success": true,
        "app_id": &cmd.app_id,
        "path": cmd.path.display().to_string(),
        "platform": format!("{:?}", cmd.platform),
        "locales": locales.iter().map(|l| l.code()).collect::<Vec<_>>(),
    }))?;

    ui.blank();
    ui.success("Metadata directory initialized successfully!");
    ui.blank();
    ui.info("Next steps:");
    ui.hint(&format!(
        "1. Fill in the metadata files in {}",
        cmd.path.display()
    ));
    ui.hint("2. Add screenshots to the screenshots/ directory");
    ui.hint(&format!(
        "3. Run `canaveral metadata validate --platform {} --app-id {}`",
        match cmd.platform {
            TargetPlatform::Apple => "apple",
            TargetPlatform::GooglePlay => "google-play",
            TargetPlatform::Both => "apple",
        },
        &cmd.app_id
    ));

    Ok(())
}
