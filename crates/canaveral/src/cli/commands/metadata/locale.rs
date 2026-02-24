//! metadata add-locale, remove-locale, list-locales subcommands

use clap::Args;
use console::style;
use std::path::PathBuf;

use canaveral_metadata::{FastlaneStorage, Locale, MetadataStorage, Platform};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{count_files_in_dir, SinglePlatform};

/// Add a new localization
#[derive(Debug, Args)]
pub struct AddLocaleArgs {
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
pub struct RemoveLocaleArgs {
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
pub struct ListLocalesArgs {
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

pub async fn execute_add(cmd: &AddLocaleArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let locale = Locale::new(&cmd.locale)
        .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &cmd.locale, e))?;

    let copy_from = match &cmd.copy_from {
        Some(code) => Some(
            Locale::new(code)
                .map_err(|e| anyhow::anyhow!("Invalid source locale '{}': {}", code, e))?,
        ),
        None => None,
    };

    let storage = FastlaneStorage::new(&cmd.path);

    ui.step(&format!(
        "Adding locale {} for {}",
        locale.code(),
        &cmd.app_id
    ));
    if let Some(ref source) = copy_from {
        ui.hint(&format!("Copying from: {}", source.code()));
    }

    let platform: Platform = cmd.platform.into();
    storage
        .add_locale(platform, &cmd.app_id, &locale, copy_from.as_ref())
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    ui.json(&serde_json::json!({
        "success": true,
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "locale": locale.code(),
        "copied_from": copy_from.as_ref().map(|l| l.code()),
    }))?;

    ui.blank();
    ui.success(&format!("Locale '{}' added successfully!", locale.code()));

    Ok(())
}

pub async fn execute_remove(cmd: &RemoveLocaleArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let locale = Locale::new(&cmd.locale)
        .map_err(|e| anyhow::anyhow!("Invalid locale '{}': {}", &cmd.locale, e))?;

    if !cmd.yes && ui.is_interactive() {
        let confirmed = ui.confirm(
            &format!(
                "Remove locale '{}' for '{}'? This will permanently delete all metadata files.",
                locale.code(),
                &cmd.app_id
            ),
            false,
        )?;
        if !confirmed {
            ui.warning("Aborted.");
            return Ok(());
        }
    }

    let storage = FastlaneStorage::new(&cmd.path);

    ui.step(&format!(
        "Removing locale {} for {}",
        locale.code(),
        &cmd.app_id
    ));

    let platform: Platform = cmd.platform.into();
    storage
        .remove_locale(platform, &cmd.app_id, &locale)
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    ui.json(&serde_json::json!({
        "success": true,
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "locale": locale.code(),
    }))?;

    ui.blank();
    ui.success(&format!("Locale '{}' removed successfully!", locale.code()));

    Ok(())
}

pub async fn execute_list(cmd: &ListLocalesArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let storage = FastlaneStorage::new(&cmd.path);

    ui.step(&format!("Listing locales for {}", &cmd.app_id));

    let platform: Platform = cmd.platform.into();
    let locales = match platform {
        Platform::Apple => storage.list_locales_apple(&cmd.app_id).await,
        Platform::GooglePlay => storage.list_locales_google_play(&cmd.app_id).await,
        Platform::Npm | Platform::Crates | Platform::PyPI => {
            return Err(anyhow::anyhow!(
                "Locale-based metadata is not applicable to package registries (npm, crates.io, PyPI)"
            ));
        }
    }
    .map_err(|e| anyhow::anyhow!("{}", e))?;

    let app_path = match platform {
        Platform::Apple => storage.apple_path(&cmd.app_id),
        Platform::GooglePlay => storage.google_play_path(&cmd.app_id),
        Platform::Npm | Platform::Crates | Platform::PyPI => {
            unreachable!("Package registries don't support locale-based metadata")
        }
    };

    let mut locale_info: Vec<(String, usize)> = Vec::new();
    for locale in &locales {
        let locale_path = app_path.join(locale.code());
        let file_count = count_files_in_dir(&locale_path).await.unwrap_or(0);
        locale_info.push((locale.code(), file_count));
    }
    locale_info.sort_by(|a, b| a.0.cmp(&b.0));

    ui.json(&serde_json::json!({
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
        "locale_count": locales.len(),
        "locales": locale_info.iter().map(|(code, count)| {
            serde_json::json!({
                "code": code,
                "file_count": count,
            })
        }).collect::<Vec<_>>(),
    }))?;

    if ui.is_text() {
        println!();
        if locale_info.is_empty() {
            println!("{}", style("No locales found.").yellow());
            println!("Run `canaveral metadata init` to create the metadata structure.");
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

    Ok(())
}
