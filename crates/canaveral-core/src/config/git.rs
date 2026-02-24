//! Git configuration

use serde::{Deserialize, Serialize};

/// Git configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Remote name
    pub remote: String,

    /// Branch to release from
    pub branch: String,

    /// Whether to require clean working directory
    pub require_clean: bool,

    /// Whether to push tags
    pub push_tags: bool,

    /// Whether to push commits
    pub push_commits: bool,

    /// Commit message template
    pub commit_message: String,

    /// Whether to sign commits
    pub sign_commits: bool,

    /// Whether to sign tags
    pub sign_tags: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            require_clean: true,
            push_tags: true,
            push_commits: true,
            commit_message: "chore(release): {version}".to_string(),
            sign_commits: false,
            sign_tags: false,
        }
    }
}
