//! Release notes generation configuration

use serde::{Deserialize, Serialize};

/// Release notes generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleaseNotesConfig {
    /// Whether to categorize changes
    pub categorize: bool,

    /// Whether to include contributor list
    pub include_contributors: bool,

    /// Whether to include migration guide for breaking changes
    pub include_migration_guide: bool,

    /// Whether to auto-update store metadata with release notes
    pub auto_update_store_metadata: bool,

    /// Locales for release notes
    #[serde(default)]
    pub locales: Vec<String>,
}

impl Default for ReleaseNotesConfig {
    fn default() -> Self {
        Self {
            categorize: true,
            include_contributors: true,
            include_migration_guide: true,
            auto_update_store_metadata: false,
            locales: vec!["en-US".to_string()],
        }
    }
}
