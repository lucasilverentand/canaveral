//! App store and package registry configurations

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// App store and package registry configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct StoresConfig {
    /// Apple App Store / macOS configuration
    #[serde(default)]
    pub apple: Option<AppleStoreConfig>,

    /// Google Play Store configuration
    #[serde(default)]
    pub google_play: Option<GooglePlayStoreConfig>,

    /// Microsoft Store configuration
    #[serde(default)]
    pub microsoft: Option<MicrosoftStoreConfig>,

    /// NPM registry configuration
    #[serde(default)]
    pub npm: Option<NpmRegistryConfig>,

    /// Crates.io registry configuration
    #[serde(default)]
    pub crates_io: Option<CratesIoRegistryConfig>,
}

/// NPM registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmRegistryConfig {
    /// Registry URL (default: https://registry.npmjs.org)
    pub registry_url: String,
    /// NPM authentication token
    pub token: Option<String>,
}

impl Default for NpmRegistryConfig {
    fn default() -> Self {
        Self {
            registry_url: "https://registry.npmjs.org".to_string(),
            token: None,
        }
    }
}

/// Crates.io registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CratesIoRegistryConfig {
    /// Registry URL (default: https://crates.io)
    pub registry_url: String,
    /// API token from CARGO_REGISTRY_TOKEN or ~/.cargo/credentials.toml
    pub token: Option<String>,
}

impl Default for CratesIoRegistryConfig {
    fn default() -> Self {
        Self {
            registry_url: "https://crates.io".to_string(),
            token: None,
        }
    }
}

/// Apple App Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleStoreConfig {
    /// App Store Connect API Key ID
    pub api_key_id: String,

    /// API Key Issuer ID
    pub api_issuer_id: String,

    /// Path to .p8 key file or env var name containing key
    pub api_key: String,

    /// Apple Team ID
    pub team_id: Option<String>,

    /// Bundle identifier
    pub app_id: Option<String>,

    /// Notarize before upload
    #[serde(default)]
    pub notarize: bool,

    /// Staple notarization ticket
    #[serde(default)]
    pub staple: bool,

    /// Primary locale for app metadata
    pub primary_locale: Option<String>,
}

/// Google Play Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePlayStoreConfig {
    /// Android package name
    pub package_name: String,

    /// Path to service account JSON key file
    pub service_account_key: PathBuf,

    /// Default release track
    #[serde(default)]
    pub default_track: Option<String>,
}

/// Microsoft Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftStoreConfig {
    /// Azure AD Tenant ID
    pub tenant_id: String,

    /// Azure AD Application (Client) ID
    pub client_id: String,

    /// Azure AD Client Secret
    pub client_secret: String,

    /// Partner Center Application ID (Store ID)
    pub app_id: String,

    /// Default flight (package flight name) - optional
    pub default_flight: Option<String>,
}
