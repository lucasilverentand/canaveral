//! Changelog workflow operations

use std::path::Path;

use tracing::{debug, info};

use crate::config::Config;
use crate::error::Result;

/// Options for changelog generation
#[derive(Debug, Clone)]
pub struct ChangelogOptions {
    /// Version to generate changelog for
    pub version: String,
    /// Date string for the release
    pub date: Option<String>,
    /// Whether to prepend to existing changelog
    pub prepend: bool,
}

impl ChangelogOptions {
    /// Create new options with just a version
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            date: None,
            prepend: true,
        }
    }

    /// Set the date
    pub fn with_date(mut self, date: impl Into<String>) -> Self {
        self.date = Some(date.into());
        self
    }
}

/// Write changelog to file
pub fn write_changelog(config: &Config, content: &str, prepend: bool) -> Result<()> {
    let path = &config.changelog.file;
    info!(path = %path.display(), prepend, "writing changelog");

    if prepend && path.exists() {
        let existing = std::fs::read_to_string(path)?;
        let combined = format!("{}\n{}", content, existing);
        std::fs::write(path, combined)?;
    } else {
        std::fs::write(path, content)?;
    }

    Ok(())
}

/// Read existing changelog content
pub fn read_changelog(path: &Path) -> Result<Option<String>> {
    if path.exists() {
        debug!(path = %path.display(), "reading existing changelog");
        Ok(Some(std::fs::read_to_string(path)?))
    } else {
        debug!(path = %path.display(), "no existing changelog found");
        Ok(None)
    }
}
