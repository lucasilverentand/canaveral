//! Changelog configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Changelog configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangelogConfig {
    /// Whether to generate changelog
    pub enabled: bool,

    /// Changelog file path
    pub file: PathBuf,

    /// Changelog format (markdown, etc.)
    pub format: String,

    /// Commit types to include
    #[serde(default)]
    pub types: HashMap<String, CommitTypeConfig>,

    /// Header template
    pub header: Option<String>,

    /// Whether to include commit hashes
    pub include_hashes: bool,

    /// Whether to include authors
    pub include_authors: bool,

    /// Whether to include dates
    pub include_dates: bool,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        let mut types = HashMap::new();
        types.insert(
            "feat".to_string(),
            CommitTypeConfig {
                section: "Features".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "fix".to_string(),
            CommitTypeConfig {
                section: "Bug Fixes".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "docs".to_string(),
            CommitTypeConfig {
                section: "Documentation".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "perf".to_string(),
            CommitTypeConfig {
                section: "Performance".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "refactor".to_string(),
            CommitTypeConfig {
                section: "Refactoring".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "test".to_string(),
            CommitTypeConfig {
                section: "Tests".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "chore".to_string(),
            CommitTypeConfig {
                section: "Chores".to_string(),
                hidden: true,
            },
        );

        Self {
            enabled: true,
            file: PathBuf::from("CHANGELOG.md"),
            format: "markdown".to_string(),
            types,
            header: None,
            include_hashes: true,
            include_authors: false,
            include_dates: true,
        }
    }
}

/// Configuration for a commit type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitTypeConfig {
    /// Section header in changelog
    pub section: String,
    /// Whether to hide this type from changelog
    pub hidden: bool,
}
