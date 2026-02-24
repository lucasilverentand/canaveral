//! CI/CD configuration

use serde::{Deserialize, Serialize};

/// CI/CD configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CIConfig {
    /// CI platform (github, gitlab)
    pub platform: String,

    /// CI mode (native, traditional)
    pub mode: String,

    /// Tasks to run on PR
    #[serde(default)]
    pub on_pr: Vec<String>,

    /// Tasks to run on push to main
    #[serde(default)]
    pub on_push_main: Vec<String>,

    /// Tasks to run on tag
    #[serde(default)]
    pub on_tag: Vec<String>,
}

impl Default for CIConfig {
    fn default() -> Self {
        Self {
            platform: "github".to_string(),
            mode: "native".to_string(),
            on_pr: vec!["test".to_string(), "lint".to_string()],
            on_push_main: vec!["test".to_string(), "release".to_string()],
            on_tag: vec!["publish".to_string()],
        }
    }
}
