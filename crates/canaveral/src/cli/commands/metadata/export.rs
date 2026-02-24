//! metadata export subcommand

use clap::{Args, ValueEnum};
use std::io::Write;
use std::path::PathBuf;

use canaveral_core::config::load_config_or_default;
use canaveral_metadata::{FastlaneStorage, MetadataStorage};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{escape_csv, SinglePlatform};

/// Export format for metadata
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ExportFormat {
    /// JSON format (pretty-printed)
    #[default]
    Json,
    /// TOML format
    Toml,
    /// CSV format (localized text fields only)
    Csv,
}

/// Export metadata to different formats
#[derive(Debug, Args)]
pub struct ExportArgs {
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

pub async fn execute(cmd: &ExportArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let cwd = std::env::current_dir()?;
    let (config, _) = load_config_or_default(&cwd);

    let metadata_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| config.metadata.storage.path.clone());

    let storage = FastlaneStorage::new(&metadata_path);

    ui.step(&format!("Exporting metadata for {}", &cmd.app_id));

    let output_string = match cmd.platform {
        SinglePlatform::Apple => {
            if !storage.exists_apple(&cmd.app_id).await? {
                anyhow::bail!(
                    "Apple metadata not found for '{}'. Run 'canaveral metadata init' first.",
                    &cmd.app_id
                );
            }
            let metadata = storage.load_apple(&cmd.app_id).await?;
            match cmd.format {
                ExportFormat::Json => serde_json::to_string_pretty(&metadata)?,
                ExportFormat::Toml => toml::to_string_pretty(&metadata)?,
                ExportFormat::Csv => export_apple_csv(&metadata)?,
            }
        }
        SinglePlatform::GooglePlay => {
            if !storage.exists_google_play(&cmd.app_id).await? {
                anyhow::bail!(
                    "Google Play metadata not found for '{}'. Run 'canaveral metadata init' first.",
                    &cmd.app_id
                );
            }
            let metadata = storage.load_google_play(&cmd.app_id).await?;
            match cmd.format {
                ExportFormat::Json => serde_json::to_string_pretty(&metadata)?,
                ExportFormat::Toml => toml::to_string_pretty(&metadata)?,
                ExportFormat::Csv => export_google_play_csv(&metadata)?,
            }
        }
    };

    if let Some(ref output_path) = cmd.output {
        let mut file = std::fs::File::create(output_path)?;
        file.write_all(output_string.as_bytes())?;
        ui.success(&format!("Exported to {}", output_path.display()));
    } else {
        println!("{}", output_string);
    }

    Ok(())
}

fn export_apple_csv(metadata: &canaveral_metadata::AppleMetadata) -> anyhow::Result<String> {
    let mut csv_output = String::new();
    csv_output.push_str("locale,name,subtitle,description,keywords,whats_new,promotional_text,privacy_policy_url,support_url,marketing_url\n");

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

fn export_google_play_csv(
    metadata: &canaveral_metadata::GooglePlayMetadata,
) -> anyhow::Result<String> {
    let mut csv_output = String::new();
    csv_output.push_str("locale,title,short_description,full_description,video_url\n");

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
