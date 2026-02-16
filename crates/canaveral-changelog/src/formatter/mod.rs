//! Changelog formatters

mod markdown;
mod registry;

pub use markdown::MarkdownFormatter;
pub use registry::FormatterRegistry;

use canaveral_core::config::ChangelogConfig;

use crate::types::ChangelogEntry;

/// Trait for changelog formatters
pub trait ChangelogFormatter: Send + Sync {
    /// Format a changelog entry to string
    fn format(&self, entry: &ChangelogEntry, config: &ChangelogConfig) -> String;

    /// Get the file extension for this format
    fn extension(&self) -> &'static str;
}
