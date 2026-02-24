//! Code signing configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Code signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SigningConfig {
    /// Whether signing is enabled
    pub enabled: bool,

    /// Signing provider to use (macos, windows, android, gpg)
    pub provider: Option<String>,

    /// Signing identity (certificate name, fingerprint, or key ID)
    pub identity: Option<String>,

    /// macOS-specific signing options
    pub macos: MacOSSigningConfig,

    /// Windows-specific signing options
    pub windows: WindowsSigningConfig,

    /// Android-specific signing options
    pub android: AndroidSigningConfig,

    /// GPG-specific signing options
    pub gpg: GpgSigningConfig,

    /// Artifacts to sign (glob patterns)
    #[serde(default)]
    pub artifacts: Vec<String>,

    /// Whether to verify signatures after signing
    pub verify_after_sign: bool,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: None,
            identity: None,
            macos: MacOSSigningConfig::default(),
            windows: WindowsSigningConfig::default(),
            android: AndroidSigningConfig::default(),
            gpg: GpgSigningConfig::default(),
            artifacts: Vec::new(),
            verify_after_sign: true,
        }
    }
}

/// macOS-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MacOSSigningConfig {
    /// Enable hardened runtime
    pub hardened_runtime: bool,

    /// Path to entitlements file
    pub entitlements: Option<PathBuf>,

    /// Enable timestamping
    pub timestamp: bool,

    /// Deep signing (sign nested code)
    pub deep: bool,

    /// Notarize after signing
    pub notarize: bool,

    /// Apple ID for notarization
    pub apple_id: Option<String>,

    /// App Store Connect API key ID
    pub api_key_id: Option<String>,

    /// App Store Connect API issuer ID
    pub api_issuer_id: Option<String>,

    /// Path to App Store Connect API private key
    pub api_key_path: Option<PathBuf>,

    /// Team ID for notarization
    pub team_id: Option<String>,
}

impl Default for MacOSSigningConfig {
    fn default() -> Self {
        Self {
            hardened_runtime: true,
            entitlements: None,
            timestamp: true,
            deep: true,
            notarize: false,
            apple_id: None,
            api_key_id: None,
            api_issuer_id: None,
            api_key_path: None,
            team_id: None,
        }
    }
}

/// Windows-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowsSigningConfig {
    /// Timestamp server URL
    pub timestamp_url: Option<String>,

    /// Hash algorithm (sha256, sha384, sha512)
    pub algorithm: String,

    /// Description to embed in signature
    pub description: Option<String>,

    /// URL to embed in signature
    pub description_url: Option<String>,

    /// Path to PFX certificate file (alternative to store)
    pub certificate_file: Option<PathBuf>,

    /// Environment variable containing PFX password
    pub certificate_password_env: Option<String>,
}

impl Default for WindowsSigningConfig {
    fn default() -> Self {
        Self {
            timestamp_url: Some("http://timestamp.digicert.com".to_string()),
            algorithm: "sha256".to_string(),
            description: None,
            description_url: None,
            certificate_file: None,
            certificate_password_env: None,
        }
    }
}

/// Android-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AndroidSigningConfig {
    /// Path to keystore file
    pub keystore: Option<PathBuf>,

    /// Key alias in the keystore
    pub key_alias: Option<String>,

    /// Environment variable containing keystore password
    pub keystore_password_env: Option<String>,

    /// Environment variable containing key password
    pub key_password_env: Option<String>,

    /// V1 (JAR) signing scheme
    pub v1_signing: bool,

    /// V2 (APK) signing scheme
    pub v2_signing: bool,

    /// V3 signing scheme
    pub v3_signing: bool,

    /// V4 signing scheme
    pub v4_signing: bool,
}

impl Default for AndroidSigningConfig {
    fn default() -> Self {
        Self {
            keystore: None,
            key_alias: None,
            keystore_password_env: Some("ANDROID_KEYSTORE_PASSWORD".to_string()),
            key_password_env: Some("ANDROID_KEY_PASSWORD".to_string()),
            v1_signing: true,
            v2_signing: true,
            v3_signing: true,
            v4_signing: false,
        }
    }
}

/// GPG-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GpgSigningConfig {
    /// GPG key ID or email
    pub key_id: Option<String>,

    /// Create detached signatures
    pub detached: bool,

    /// ASCII armor output
    pub armor: bool,

    /// Environment variable containing passphrase
    pub passphrase_env: Option<String>,

    /// Path to GPG binary
    pub gpg_path: Option<PathBuf>,
}

impl Default for GpgSigningConfig {
    fn default() -> Self {
        Self {
            key_id: None,
            detached: true,
            armor: true,
            passphrase_env: Some("GPG_PASSPHRASE".to_string()),
            gpg_path: None,
        }
    }
}
