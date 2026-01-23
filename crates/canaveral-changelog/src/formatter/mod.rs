//! Changelog formatters

mod markdown;

pub use markdown::MarkdownFormatter;

use canaveral_core::config::ChangelogConfig;

use crate::types::ChangelogEntry;

/// Trait for changelog formatters
pub trait ChangelogFormatter: Send + Sync {
    /// Format a changelog entry to string
    fn format(&self, entry: &ChangelogEntry, config: &ChangelogConfig) -> String;

    /// Get the file extension for this format
    fn extension(&self) -> &'static str;
}
