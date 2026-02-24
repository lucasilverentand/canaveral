//! Metadata CLI commands

mod diff;
mod export;
mod init;
mod locale;
mod screenshots;
mod sync;
mod validate;

use clap::{Args, Subcommand, ValueEnum};
use tracing::info;

use crate::cli::Cli;

// Re-export types used by multiple subcommands
pub(crate) use sync::{AppleAuthOptions, GooglePlayAuthOptions};

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
    Init(init::InitArgs),

    /// Validate metadata against store requirements
    Validate(validate::ValidateArgs),

    /// Export metadata to different formats
    Export(export::ExportArgs),

    /// Add a new localization
    AddLocale(locale::AddLocaleArgs),

    /// Remove a localization
    RemoveLocale(locale::RemoveLocaleArgs),

    /// List available localizations
    ListLocales(locale::ListLocalesArgs),

    /// Screenshot management commands
    Screenshots(screenshots::ScreenshotsCommand),

    /// Sync metadata with app stores (pull/push)
    Sync(sync::SyncCommand),

    /// Compare local vs remote metadata
    Diff(diff::DiffArgs),
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

/// Target platform for single-platform operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SinglePlatform {
    /// Apple App Store
    Apple,
    /// Google Play Store
    GooglePlay,
}

impl From<SinglePlatform> for canaveral_metadata::Platform {
    fn from(platform: SinglePlatform) -> Self {
        match platform {
            SinglePlatform::Apple => canaveral_metadata::Platform::Apple,
            SinglePlatform::GooglePlay => canaveral_metadata::Platform::GooglePlay,
        }
    }
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

impl From<MetadataFormat> for canaveral_metadata::StorageFormat {
    fn from(format: MetadataFormat) -> Self {
        match format {
            MetadataFormat::Fastlane => canaveral_metadata::StorageFormat::Fastlane,
            MetadataFormat::Unified => canaveral_metadata::StorageFormat::Unified,
        }
    }
}

impl MetadataCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!("executing metadata command");
        let rt = tokio::runtime::Runtime::new()?;

        match &self.command {
            MetadataSubcommand::Init(cmd) => rt.block_on(init::execute(cmd, cli)),
            MetadataSubcommand::Validate(cmd) => rt.block_on(validate::execute(cmd, cli)),
            MetadataSubcommand::Export(cmd) => rt.block_on(export::execute(cmd, cli)),
            MetadataSubcommand::AddLocale(cmd) => rt.block_on(locale::execute_add(cmd, cli)),
            MetadataSubcommand::RemoveLocale(cmd) => rt.block_on(locale::execute_remove(cmd, cli)),
            MetadataSubcommand::ListLocales(cmd) => rt.block_on(locale::execute_list(cmd, cli)),
            MetadataSubcommand::Screenshots(cmd) => rt.block_on(screenshots::execute(cmd, cli)),
            MetadataSubcommand::Sync(cmd) => rt.block_on(sync::execute(cmd, cli)),
            MetadataSubcommand::Diff(cmd) => rt.block_on(diff::execute(cmd, cli)),
        }
    }
}

// ── Shared helpers ──────────────────────────────────────────────────

/// Parse a locale string (comma-separated or "all") into Option<Vec<Locale>>
pub(crate) fn parse_locales(
    locales_str: &str,
) -> anyhow::Result<Option<Vec<canaveral_metadata::Locale>>> {
    if locales_str.to_lowercase() == "all" {
        return Ok(None);
    }

    let locales: Result<Vec<_>, _> = locales_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(canaveral_metadata::Locale::new)
        .collect();

    Ok(Some(
        locales.map_err(|e| anyhow::anyhow!("Invalid locale: {}", e))?,
    ))
}

/// Escape a string for CSV output
pub(crate) fn escape_csv(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') || value.contains('\r') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

/// Truncate a string and add ellipsis if too long
pub(crate) fn truncate_str(s: &str, max_len: usize) -> String {
    let s = s.replace('\n', " ").replace('\r', "");
    let s = s.trim();

    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

/// Count files in a directory (non-recursive).
pub(crate) async fn count_files_in_dir(path: &std::path::Path) -> std::io::Result<usize> {
    let mut count = 0;
    let mut entries = tokio::fs::read_dir(path).await?;
    while let Some(entry) = entries.next_entry().await? {
        if entry.path().is_file() {
            count += 1;
        }
    }
    Ok(count)
}

/// List subdirectories in a directory
pub(crate) async fn list_subdirectories(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
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
pub(crate) async fn list_image_files(
    dir: &std::path::Path,
) -> std::io::Result<Vec<std::path::PathBuf>> {
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
