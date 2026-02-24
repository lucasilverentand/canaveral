//! PR validation configuration

use serde::{Deserialize, Serialize};

/// PR validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrConfig {
    /// Branching model (trunk-based, gitflow, custom)
    pub branching_model: String,

    /// Checks to run on PR validation
    #[serde(default)]
    pub checks: Vec<String>,

    /// Whether to require changelog entry
    pub require_changelog: bool,

    /// Whether to require conventional commits
    pub require_conventional_commits: bool,
}

impl Default for PrConfig {
    fn default() -> Self {
        Self {
            branching_model: "trunk-based".to_string(),
            checks: vec![
                "tests".to_string(),
                "lint".to_string(),
                "commit-format".to_string(),
                "version-conflict".to_string(),
            ],
            require_changelog: false,
            require_conventional_commits: true,
        }
    }
}
