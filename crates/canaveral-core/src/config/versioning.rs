//! Versioning configuration

use serde::{Deserialize, Serialize};

/// Versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VersioningConfig {
    /// Version strategy (semver, calver, etc.)
    pub strategy: String,

    /// Tag format (e.g., "v{version}")
    pub tag_format: String,

    /// Whether to use independent versioning in monorepos
    pub independent: bool,

    /// Pre-release identifier
    pub prerelease_identifier: Option<String>,

    /// Build metadata
    pub build_metadata: Option<String>,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            strategy: "semver".to_string(),
            tag_format: "v{version}".to_string(),
            independent: false,
            prerelease_identifier: None,
            build_metadata: None,
        }
    }
}
