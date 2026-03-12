//! Provisioning profile management
//!
//! Handles parsing, installation, and management of iOS/macOS provisioning profiles.
//! Supports `.mobileprovision` files used for code signing on Apple platforms.

mod manager;
mod parser;
mod portal;

pub use manager::ProfileManager;
pub use parser::parse_mobileprovision;
pub use portal::{PortalClient, PortalConfig, PortalProfile};

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::sync::ProfileType;

/// A parsed provisioning profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisioningProfile {
    /// Profile UUID (unique identifier)
    pub uuid: String,

    /// Human-readable profile name
    pub name: String,

    /// Apple Team ID
    pub team_id: String,

    /// Bundle identifier (e.g., "com.example.app")
    pub bundle_id: String,

    /// Profile type (development, ad-hoc, app store, enterprise)
    pub profile_type: ProfileType,

    /// SHA-1 fingerprints of certificates embedded in the profile
    pub certificates: Vec<String>,

    /// Device UDIDs (for development/ad-hoc profiles)
    pub devices: Vec<String>,

    /// Entitlements dictionary
    pub entitlements: HashMap<String, serde_json::Value>,

    /// When the profile was created
    pub creation_date: DateTime<Utc>,

    /// When the profile expires
    pub expiration_date: DateTime<Utc>,

    /// Local file path (if installed)
    pub path: Option<PathBuf>,
}

impl ProvisioningProfile {
    /// Check if the profile has expired
    pub fn is_expired(&self) -> bool {
        self.expiration_date < Utc::now()
    }

    /// Check if the profile expires within the given number of days
    pub fn expires_within_days(&self, days: i64) -> bool {
        let threshold = Utc::now() + chrono::Duration::days(days);
        self.expiration_date < threshold
    }

    /// Check if this profile matches a given bundle ID
    ///
    /// Supports wildcard matching (e.g., "com.example.*" matches "com.example.app")
    pub fn matches_bundle_id(&self, bundle_id: &str) -> bool {
        if self.bundle_id == bundle_id {
            return true;
        }

        // Handle wildcard profiles (e.g., "com.example.*")
        if self.bundle_id.ends_with(".*") {
            let prefix = &self.bundle_id[..self.bundle_id.len() - 1]; // "com.example."
            return bundle_id.starts_with(prefix);
        }

        // Universal wildcard
        if self.bundle_id == "*" {
            return true;
        }

        false
    }

    /// Infer the profile type from profile properties
    pub fn infer_profile_type(
        has_devices: bool,
        has_get_task_allow: bool,
        is_enterprise: bool,
    ) -> ProfileType {
        if is_enterprise {
            ProfileType::Enterprise
        } else if has_get_task_allow {
            ProfileType::Development
        } else if has_devices {
            ProfileType::AdHoc
        } else {
            ProfileType::AppStore
        }
    }
}

impl std::fmt::Display for ProvisioningProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} ({}) [{}] expires {}",
            self.name,
            self.bundle_id,
            self.profile_type,
            self.expiration_date.format("%Y-%m-%d")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile(bundle_id: &str) -> ProvisioningProfile {
        ProvisioningProfile {
            uuid: "TEST-UUID".to_string(),
            name: "Test Profile".to_string(),
            team_id: "TEAM123".to_string(),
            bundle_id: bundle_id.to_string(),
            profile_type: ProfileType::Development,
            certificates: vec![],
            devices: vec![],
            entitlements: HashMap::new(),
            creation_date: Utc::now(),
            expiration_date: Utc::now() + chrono::Duration::days(365),
            path: None,
        }
    }

    #[test]
    fn test_exact_bundle_id_match() {
        let profile = make_profile("com.example.app");
        assert!(profile.matches_bundle_id("com.example.app"));
        assert!(!profile.matches_bundle_id("com.example.other"));
    }

    #[test]
    fn test_wildcard_bundle_id_match() {
        let profile = make_profile("com.example.*");
        assert!(profile.matches_bundle_id("com.example.app"));
        assert!(profile.matches_bundle_id("com.example.other"));
        assert!(!profile.matches_bundle_id("com.other.app"));
    }

    #[test]
    fn test_universal_wildcard_match() {
        let profile = make_profile("*");
        assert!(profile.matches_bundle_id("com.example.app"));
        assert!(profile.matches_bundle_id("anything"));
    }

    #[test]
    fn test_is_expired() {
        let mut profile = make_profile("com.example.app");
        assert!(!profile.is_expired());

        profile.expiration_date = Utc::now() - chrono::Duration::days(1);
        assert!(profile.is_expired());
    }

    #[test]
    fn test_expires_within_days() {
        let mut profile = make_profile("com.example.app");
        profile.expiration_date = Utc::now() + chrono::Duration::days(15);
        assert!(profile.expires_within_days(30));
        assert!(!profile.expires_within_days(10));
    }

    #[test]
    fn test_infer_profile_type() {
        assert!(matches!(
            ProvisioningProfile::infer_profile_type(false, true, false),
            ProfileType::Development
        ));
        assert!(matches!(
            ProvisioningProfile::infer_profile_type(true, false, false),
            ProfileType::AdHoc
        ));
        assert!(matches!(
            ProvisioningProfile::infer_profile_type(false, false, false),
            ProfileType::AppStore
        ));
        assert!(matches!(
            ProvisioningProfile::infer_profile_type(true, true, true),
            ProfileType::Enterprise
        ));
    }
}
