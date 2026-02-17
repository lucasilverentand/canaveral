//! Signing identity types and management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Type of signing identity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningIdentityType {
    /// macOS/iOS Developer ID or Distribution certificate
    AppleDeveloper,
    /// macOS/iOS App Store Distribution certificate
    AppleDistribution,
    /// macOS Installer certificate (for .pkg files)
    AppleInstaller,
    /// Windows Authenticode certificate
    WindowsAuthenticode,
    /// Windows EV Code Signing certificate
    WindowsEV,
    /// Android keystore key
    AndroidKeystore,
    /// GPG key
    Gpg,
    /// Generic/unknown certificate type
    Generic,
}

impl std::fmt::Display for SigningIdentityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AppleDeveloper => write!(f, "Apple Developer ID"),
            Self::AppleDistribution => write!(f, "Apple Distribution"),
            Self::AppleInstaller => write!(f, "Apple Installer"),
            Self::WindowsAuthenticode => write!(f, "Windows Authenticode"),
            Self::WindowsEV => write!(f, "Windows EV"),
            Self::AndroidKeystore => write!(f, "Android Keystore"),
            Self::Gpg => write!(f, "GPG"),
            Self::Generic => write!(f, "Generic"),
        }
    }
}

/// A signing identity that can be used to sign artifacts
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningIdentity {
    /// Unique identifier for this identity
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Type of signing identity
    #[serde(rename = "type")]
    pub identity_type: SigningIdentityType,

    /// Certificate/key fingerprint (SHA-256)
    pub fingerprint: Option<String>,

    /// Team or organization ID (e.g., Apple Team ID)
    pub team_id: Option<String>,

    /// Certificate subject/common name
    pub subject: Option<String>,

    /// Certificate issuer
    pub issuer: Option<String>,

    /// Certificate serial number
    pub serial_number: Option<String>,

    /// When the certificate/key was created
    pub created_at: Option<DateTime<Utc>>,

    /// When the certificate/key expires
    pub expires_at: Option<DateTime<Utc>>,

    /// Whether this identity is currently valid
    pub is_valid: bool,

    /// Keychain or store where this identity is located (macOS/Windows)
    pub keychain: Option<String>,

    /// Key alias (Android keystore)
    pub key_alias: Option<String>,
}

impl SigningIdentity {
    /// Create a new signing identity
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        identity_type: SigningIdentityType,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            identity_type,
            fingerprint: None,
            team_id: None,
            subject: None,
            issuer: None,
            serial_number: None,
            created_at: None,
            expires_at: None,
            is_valid: true,
            keychain: None,
            key_alias: None,
        }
    }

    /// Check if the identity is expired
    pub fn is_expired(&self) -> bool {
        if let Some(expires_at) = self.expires_at {
            expires_at < Utc::now()
        } else {
            false
        }
    }

    /// Check if the identity is not yet valid
    pub fn is_not_yet_valid(&self) -> bool {
        if let Some(created_at) = self.created_at {
            created_at > Utc::now()
        } else {
            false
        }
    }

    /// Get days until expiration (None if no expiration date)
    pub fn days_until_expiration(&self) -> Option<i64> {
        self.expires_at.map(|exp| {
            let duration = exp.signed_duration_since(Utc::now());
            duration.num_days()
        })
    }

    /// Check if the identity will expire within the given number of days
    pub fn expires_within_days(&self, days: i64) -> bool {
        self.days_until_expiration()
            .map(|d| d <= days)
            .unwrap_or(false)
    }

    /// Get a display string for the identity (useful for UI)
    pub fn display_name(&self) -> String {
        if let Some(team_id) = &self.team_id {
            format!("{} ({})", self.name, team_id)
        } else {
            self.name.clone()
        }
    }
}

impl std::fmt::Display for SigningIdentity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]", self.display_name(), self.identity_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_creation() {
        let identity = SigningIdentity::new(
            "test-id",
            "Test Certificate",
            SigningIdentityType::AppleDeveloper,
        );

        assert_eq!(identity.id, "test-id");
        assert_eq!(identity.name, "Test Certificate");
        assert_eq!(identity.identity_type, SigningIdentityType::AppleDeveloper);
        assert!(identity.is_valid);
    }

    #[test]
    fn test_identity_expiration() {
        let mut identity = SigningIdentity::new("test", "Test", SigningIdentityType::Generic);

        // No expiration set
        assert!(!identity.is_expired());
        assert!(identity.days_until_expiration().is_none());

        // Set expiration in the past
        identity.expires_at = Some(Utc::now() - chrono::Duration::days(1));
        assert!(identity.is_expired());

        // Set expiration in the future
        identity.expires_at = Some(Utc::now() + chrono::Duration::days(30));
        assert!(!identity.is_expired());
        assert!(identity.expires_within_days(60));
        assert!(!identity.expires_within_days(10));
    }

    #[test]
    fn test_display_name() {
        let mut identity = SigningIdentity::new(
            "test",
            "Developer ID Application: Company",
            SigningIdentityType::AppleDeveloper,
        );

        assert_eq!(identity.display_name(), "Developer ID Application: Company");

        identity.team_id = Some("ABCD1234".to_string());
        assert_eq!(
            identity.display_name(),
            "Developer ID Application: Company (ABCD1234)"
        );
    }
}
