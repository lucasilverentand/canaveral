//! iOS platform configuration
//!
//! Supports configuring iOS build, signing, and export settings
//! via `canaveral.toml`:
//!
//! ```toml
//! [ios]
//! scheme = "MyApp"
//! team_id = "ABCDE12345"
//! bundle_id = "com.example.app"
//!
//! [ios.signing]
//! style = "automatic"
//! development_team = "ABCDE12345"
//!
//! [ios.export]
//! method = "app-store"
//! upload_symbols = true
//! compile_bitcode = false
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// iOS platform configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IosConfig {
    /// Xcode scheme name (auto-detected if not specified)
    pub scheme: Option<String>,

    /// Apple Developer Team ID
    pub team_id: Option<String>,

    /// App bundle identifier (e.g., "com.example.app")
    pub bundle_id: Option<String>,

    /// Default Xcode build configuration (Debug or Release)
    pub configuration: Option<String>,

    /// Default build destination (e.g., "generic/platform=iOS")
    pub destination: Option<String>,

    /// Custom derived data path
    pub derived_data: Option<PathBuf>,

    /// Default simulator device for testing (e.g., "iPhone 16")
    pub simulator: Option<String>,

    /// Default simulator OS version
    pub simulator_os: Option<String>,

    /// Xcode test plan name
    pub test_plan: Option<String>,

    /// Code signing configuration
    #[serde(default)]
    pub signing: IosSigningConfig,

    /// Export/archive configuration
    #[serde(default)]
    pub export: IosExportConfig,
}

impl Default for IosConfig {
    fn default() -> Self {
        Self {
            scheme: None,
            team_id: None,
            bundle_id: None,
            configuration: None,
            destination: None,
            derived_data: None,
            simulator: Some("iPhone 16".to_string()),
            simulator_os: None,
            test_plan: None,
            signing: IosSigningConfig::default(),
            export: IosExportConfig::default(),
        }
    }
}

/// iOS code signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IosSigningConfig {
    /// Signing style: "automatic" or "manual"
    pub style: String,

    /// Development team ID (often same as team_id)
    pub development_team: Option<String>,

    /// Code sign identity (e.g., "Apple Distribution")
    pub identity: Option<String>,

    /// Provisioning profile name or UUID
    pub provisioning_profile: Option<String>,
}

impl Default for IosSigningConfig {
    fn default() -> Self {
        Self {
            style: "automatic".to_string(),
            development_team: None,
            identity: None,
            provisioning_profile: None,
        }
    }
}

/// iOS export configuration (for archive/IPA export)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct IosExportConfig {
    /// Export method: "app-store", "ad-hoc", "development", "enterprise"
    pub method: String,

    /// Whether to upload dSYM symbols
    pub upload_symbols: bool,

    /// Whether to compile bitcode
    pub compile_bitcode: bool,

    /// Whether to strip Swift symbols
    pub strip_swift_symbols: bool,

    /// Thinning setting (e.g., "none", "<thin-for-all-variants>")
    pub thinning: Option<String>,
}

impl Default for IosExportConfig {
    fn default() -> Self {
        Self {
            method: "app-store".to_string(),
            upload_symbols: true,
            compile_bitcode: false,
            strip_swift_symbols: true,
            thinning: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_ios_config() {
        let config = IosConfig::default();
        assert!(config.scheme.is_none());
        assert_eq!(config.simulator, Some("iPhone 16".to_string()));
        assert_eq!(config.signing.style, "automatic");
        assert_eq!(config.export.method, "app-store");
        assert!(config.export.upload_symbols);
        assert!(!config.export.compile_bitcode);
    }

    #[test]
    fn test_ios_config_deserialization() {
        let toml_str = r#"
scheme = "MyApp"
team_id = "ABCDE12345"
bundle_id = "com.example.app"

[signing]
style = "manual"
development_team = "ABCDE12345"
identity = "Apple Distribution"

[export]
method = "ad-hoc"
upload_symbols = false
compile_bitcode = false
"#;
        let config: IosConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.scheme, Some("MyApp".to_string()));
        assert_eq!(config.team_id, Some("ABCDE12345".to_string()));
        assert_eq!(config.bundle_id, Some("com.example.app".to_string()));
        assert_eq!(config.signing.style, "manual");
        assert_eq!(
            config.signing.development_team,
            Some("ABCDE12345".to_string())
        );
        assert_eq!(config.export.method, "ad-hoc");
        assert!(!config.export.upload_symbols);
    }
}
