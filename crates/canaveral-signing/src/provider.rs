//! Signing provider trait and common types

use crate::error::Result;
use crate::identity::SigningIdentity;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Options for signing an artifact
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignOptions {
    /// Path to entitlements file (macOS)
    pub entitlements: Option<String>,

    /// Enable hardened runtime (macOS)
    pub hardened_runtime: bool,

    /// Timestamp the signature
    pub timestamp: bool,

    /// Timestamp server URL
    pub timestamp_url: Option<String>,

    /// Signature algorithm (e.g., "sha256", "sha384")
    pub algorithm: Option<String>,

    /// Additional flags to pass to the signing tool
    pub extra_flags: Vec<String>,

    /// Description/comment to include in signature
    pub description: Option<String>,

    /// URL to include in signature (Windows)
    pub description_url: Option<String>,

    /// Force re-signing even if already signed
    pub force: bool,

    /// Deep signing (macOS - sign nested code)
    pub deep: bool,

    /// Preserve metadata/extended attributes
    pub preserve_metadata: bool,

    /// Dry run mode - don't actually sign
    pub dry_run: bool,

    /// Verbose output
    pub verbose: bool,

    /// Keystore password (Android)
    pub keystore_password: Option<String>,

    /// Key password (Android)
    pub key_password: Option<String>,

    /// GPG passphrase
    pub passphrase: Option<String>,

    /// Create detached signature (GPG)
    pub detached: bool,

    /// Armor output (GPG)
    pub armor: bool,
}

/// Options for verifying a signature
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifyOptions {
    /// Verify deep signatures (macOS)
    pub deep: bool,

    /// Strict verification
    pub strict: bool,

    /// Verbose output
    pub verbose: bool,

    /// Check notarization status (macOS)
    pub check_notarization: bool,
}

/// Status of a signature verification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureStatus {
    /// Signature is valid
    Valid,
    /// Signature is invalid
    Invalid,
    /// Signature has expired
    Expired,
    /// Certificate has been revoked
    Revoked,
    /// Not signed
    NotSigned,
    /// Unknown/unable to verify
    Unknown,
}

impl std::fmt::Display for SignatureStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Valid => write!(f, "Valid"),
            Self::Invalid => write!(f, "Invalid"),
            Self::Expired => write!(f, "Expired"),
            Self::Revoked => write!(f, "Revoked"),
            Self::NotSigned => write!(f, "Not Signed"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Information about a signature on an artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    /// Path to the signed artifact
    pub path: String,

    /// Status of the signature
    pub status: SignatureStatus,

    /// Signer identity information
    pub signer: Option<SignerInfo>,

    /// Timestamp of when the artifact was signed
    pub signed_at: Option<DateTime<Utc>>,

    /// Timestamp authority used
    pub timestamp_authority: Option<String>,

    /// Whether the signature is notarized (macOS)
    pub notarized: Option<bool>,

    /// Whether the notarization ticket is stapled (macOS)
    pub stapled: Option<bool>,

    /// Hash algorithm used
    pub algorithm: Option<String>,

    /// Any warnings from verification
    pub warnings: Vec<String>,

    /// Detailed verification output
    pub details: Option<String>,
}

/// Information about the signer of an artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignerInfo {
    /// Common name of the signer
    pub common_name: String,

    /// Organization name
    pub organization: Option<String>,

    /// Team ID (Apple)
    pub team_id: Option<String>,

    /// Certificate fingerprint
    pub fingerprint: Option<String>,

    /// Certificate serial number
    pub serial_number: Option<String>,

    /// Certificate expiration date
    pub expires_at: Option<DateTime<Utc>>,

    /// Whether the certificate is still valid
    pub certificate_valid: bool,
}

/// Trait for signing providers
///
/// Each platform/tool has its own implementation of this trait.
#[async_trait::async_trait]
pub trait SigningProvider: Send + Sync {
    /// Get the name of this signing provider
    fn name(&self) -> &str;

    /// Check if this provider is available on the current system
    fn is_available(&self) -> bool;

    /// List available signing identities
    async fn list_identities(&self) -> Result<Vec<SigningIdentity>>;

    /// Find a signing identity by name, fingerprint, or other criteria
    async fn find_identity(&self, query: &str) -> Result<SigningIdentity>;

    /// Sign an artifact with the given identity
    async fn sign(
        &self,
        artifact: &Path,
        identity: &SigningIdentity,
        options: &SignOptions,
    ) -> Result<()>;

    /// Verify the signature on an artifact
    async fn verify(&self, artifact: &Path, options: &VerifyOptions) -> Result<SignatureInfo>;

    /// Check if an artifact is signed
    async fn is_signed(&self, artifact: &Path) -> Result<bool> {
        let info = self.verify(artifact, &VerifyOptions::default()).await?;
        Ok(info.status != SignatureStatus::NotSigned)
    }

    /// Get supported file extensions for this provider
    fn supported_extensions(&self) -> &[&str];

    /// Check if a file type is supported
    fn supports_file(&self, path: &Path) -> bool {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            self.supported_extensions()
                .iter()
                .any(|e| e.eq_ignore_ascii_case(ext))
        } else {
            // Some providers support extensionless files (e.g., macOS binaries)
            self.supported_extensions().contains(&"")
        }
    }
}

// We need async_trait for the trait definition
// Add this to Cargo.toml: async-trait = "0.1"
