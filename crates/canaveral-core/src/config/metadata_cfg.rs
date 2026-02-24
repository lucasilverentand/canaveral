//! Metadata management configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Metadata management configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataConfig {
    /// Enable metadata management
    #[serde(default)]
    pub enabled: bool,

    /// Storage configuration
    #[serde(default)]
    pub storage: MetadataStorageConfig,

    /// Default settings
    #[serde(default)]
    pub defaults: MetadataDefaultsConfig,

    /// Validation settings
    #[serde(default)]
    pub validation: MetadataValidationConfig,
}

/// Metadata storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataStorageConfig {
    /// Storage format: "fastlane" or "unified"
    #[serde(default = "default_storage_format")]
    pub format: String,

    /// Base path for metadata files
    #[serde(default = "default_metadata_path")]
    pub path: PathBuf,
}

impl Default for MetadataStorageConfig {
    fn default() -> Self {
        Self {
            format: default_storage_format(),
            path: default_metadata_path(),
        }
    }
}

/// Metadata default settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataDefaultsConfig {
    /// Default locale
    #[serde(default)]
    pub default_locale: Option<String>,

    /// Default support URL
    #[serde(default)]
    pub support_url: Option<String>,

    /// Default privacy policy URL
    #[serde(default)]
    pub privacy_policy_url: Option<String>,
}

/// Metadata validation settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataValidationConfig {
    /// Treat warnings as errors
    #[serde(default)]
    pub strict: bool,

    /// Locales that must be present
    #[serde(default)]
    pub required_locales: Vec<String>,
}

fn default_storage_format() -> String {
    "fastlane".to_string()
}

fn default_metadata_path() -> PathBuf {
    PathBuf::from("./metadata")
}
