//! metadata diff subcommand

use clap::Args;
use console::style;
use std::path::PathBuf;

use canaveral_core::config::load_config_or_default;
use canaveral_metadata::sync::{
    AppleMetadataSync, ChangeType, GooglePlayMetadataSync, MetadataDiff, MetadataSync,
};

use crate::cli::output::Ui;
use crate::cli::Cli;

use super::{parse_locales, truncate_str, AppleAuthOptions, GooglePlayAuthOptions, SinglePlatform};

/// Compare local vs remote metadata
#[derive(Debug, Args)]
pub struct DiffArgs {
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

pub async fn execute(cmd: &DiffArgs, cli: &Cli) -> anyhow::Result<()> {
    let ui = Ui::new(cli);

    let cwd = std::env::current_dir()?;
    let (config, _) = load_config_or_default(&cwd);

    let metadata_path = cmd
        .path
        .clone()
        .unwrap_or_else(|| config.metadata.storage.path.clone());

    let store_name = match cmd.platform {
        SinglePlatform::Apple => "App Store Connect",
        SinglePlatform::GooglePlay => "Google Play Console",
    };

    ui.step(&format!("Comparing local metadata with {}", store_name));
    ui.key_value("App ID", &cmd.app_id);
    ui.key_value("Path", &metadata_path.display().to_string());

    let diff = match cmd.platform {
        SinglePlatform::Apple => {
            let config = cmd.apple_auth.to_config()?;
            let sync = AppleMetadataSync::new(config, metadata_path).await?;
            sync.diff(&cmd.app_id).await?
        }
        SinglePlatform::GooglePlay => {
            let config = cmd.google_auth.to_config()?;
            let sync = GooglePlayMetadataSync::new(config, metadata_path).await?;
            sync.diff(&cmd.app_id).await?
        }
    };

    let locales = parse_locales(&cmd.locales)?;
    let filtered_diff = if let Some(ref filter_locales) = locales {
        let filter_codes: Vec<String> = filter_locales
            .iter()
            .map(|l| l.code().to_string())
            .collect();
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

    ui.json(&serde_json::json!({
        "app_id": &cmd.app_id,
        "platform": format!("{:?}", cmd.platform),
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
    }))?;

    if ui.is_text() {
        print_diff(&filtered_diff);
    }

    Ok(())
}

fn print_diff(diff: &MetadataDiff) {
    println!();

    if diff.is_empty() {
        println!(
            "{}",
            style("No differences found. Local and remote metadata are in sync.").green()
        );
        return;
    }

    println!(
        "{} ({} change(s))",
        style("Differences found").yellow().bold(),
        diff.len()
    );
    println!();

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

    let added = diff.by_type(ChangeType::Added).len();
    let modified = diff.by_type(ChangeType::Modified).len();
    let removed = diff.by_type(ChangeType::Removed).len();

    println!(
        "Summary: {} added, {} modified, {} removed",
        style(added).green(),
        style(modified).yellow(),
        style(removed).red()
    );
}
