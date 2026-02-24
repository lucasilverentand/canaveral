//! Publishing configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Publishing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PublishConfig {
    /// Whether to publish packages
    pub enabled: bool,

    /// Registry configurations
    #[serde(default)]
    pub registries: HashMap<String, RegistryConfig>,

    /// Dry run mode
    pub dry_run: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            registries: HashMap::new(),
            dry_run: false,
        }
    }
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry URL
    pub url: String,

    /// Authentication token environment variable
    pub token_env: Option<String>,
}
