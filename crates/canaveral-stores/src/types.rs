//! Common types for store adapters

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Store type identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoreType {
    /// Apple App Store / Mac App Store
    Apple,
    /// Google Play Store
    GooglePlay,
    /// Microsoft Store
    Microsoft,
    /// NPM package registry
    Npm,
    /// Crates.io Rust package registry
    Crates,
    /// Python Package Index (PyPI)
    PyPI,
    /// Docker Hub container registry
    DockerHub,
    /// GitHub Releases
    GitHubReleases,
}

impl std::fmt::Display for StoreType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StoreType::Apple => write!(f, "Apple"),
            StoreType::GooglePlay => write!(f, "Google Play"),
            StoreType::Microsoft => write!(f, "Microsoft"),
            StoreType::Npm => write!(f, "NPM"),
            StoreType::Crates => write!(f, "Crates.io"),
            StoreType::PyPI => write!(f, "PyPI"),
            StoreType::DockerHub => write!(f, "Docker Hub"),
            StoreType::GitHubReleases => write!(f, "GitHub Releases"),
        }
    }
}

/// Upload options for store adapters
#[derive(Debug, Clone, Default)]
pub struct UploadOptions {
    /// Release notes/changelog per locale
    pub release_notes: HashMap<String, String>,

    /// Target track/channel (e.g., "internal", "beta", "production")
    pub track: Option<String>,

    /// Percentage of users for staged rollout (0.0 - 1.0)
    pub rollout_percentage: Option<f64>,

    /// Whether to auto-publish after upload
    pub auto_publish: bool,

    /// Additional metadata
    pub metadata: HashMap<String, String>,

    /// Dry run - validate but don't upload
    pub dry_run: bool,

    /// Verbose logging
    pub verbose: bool,

    /// Timeout in seconds
    pub timeout: Option<u64>,

    // --- Metadata integration options ---

    /// Validate metadata before upload
    pub validate_metadata: bool,

    /// Sync metadata after successful upload
    pub sync_metadata: bool,

    /// Path to metadata directory (e.g., fastlane metadata directory)
    pub metadata_path: Option<PathBuf>,

    /// Fail upload if metadata validation has errors
    pub require_valid_metadata: bool,
}

/// Result of artifact validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub valid: bool,

    /// Validation errors
    pub errors: Vec<ValidationError>,

    /// Validation warnings
    pub warnings: Vec<String>,

    /// Extracted app information
    pub app_info: Option<AppInfo>,
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success(app_info: AppInfo) -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            app_info: Some(app_info),
        }
    }

    /// Create a failed validation result
    pub fn failure(errors: Vec<ValidationError>) -> Self {
        Self {
            valid: false,
            errors,
            warnings: Vec::new(),
            app_info: None,
        }
    }
}

/// Validation error details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Error code
    pub code: String,

    /// Human-readable message
    pub message: String,

    /// Severity level
    pub severity: ValidationSeverity,
}

/// Validation error severity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    /// Fatal error - cannot proceed
    Error,
    /// Warning - can proceed but may cause issues
    Warning,
    /// Info - informational message
    Info,
}

/// Extracted app information from artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    /// Universal identifier (bundle ID for mobile, package name for libraries, etc.)
    #[serde(alias = "bundle_id")]
    pub identifier: String,

    /// App version (e.g., "1.2.3")
    pub version: String,

    /// Build number / version code
    pub build_number: String,

    /// App name
    pub name: Option<String>,

    /// Minimum OS version
    pub min_os_version: Option<String>,

    /// Target platforms
    pub platforms: Vec<String>,

    /// File size in bytes
    pub size: u64,

    /// SHA256 hash of the artifact
    pub sha256: Option<String>,
}

impl AppInfo {
    /// Get the bundle identifier (deprecated, use `identifier` field directly)
    #[deprecated(
        since = "1.6.0",
        note = "Use `identifier` field directly instead. This method will be removed in 2.0.0"
    )]
    pub fn bundle_id(&self) -> &str {
        &self.identifier
    }
}

/// Result of an upload operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    /// Whether upload succeeded
    pub success: bool,

    /// Build/submission ID assigned by the store
    pub build_id: Option<String>,

    /// URL to view the build in the store console
    pub console_url: Option<String>,

    /// Current status of the upload
    pub status: UploadStatus,

    /// Any warnings from the upload
    pub warnings: Vec<String>,

    /// Upload timestamp
    pub uploaded_at: DateTime<Utc>,
}

/// Upload status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    /// Upload completed, processing
    Processing,
    /// Ready for distribution
    Ready,
    /// Failed processing
    Failed,
    /// Waiting for review
    PendingReview,
    /// In review
    InReview,
    /// Approved and live
    Live,
    /// Rejected by review
    Rejected,
}

impl std::fmt::Display for UploadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UploadStatus::Processing => write!(f, "Processing"),
            UploadStatus::Ready => write!(f, "Ready"),
            UploadStatus::Failed => write!(f, "Failed"),
            UploadStatus::PendingReview => write!(f, "Pending Review"),
            UploadStatus::InReview => write!(f, "In Review"),
            UploadStatus::Live => write!(f, "Live"),
            UploadStatus::Rejected => write!(f, "Rejected"),
        }
    }
}

/// Build status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildStatus {
    /// Build/submission ID
    pub build_id: String,

    /// App version
    pub version: String,

    /// Build number
    pub build_number: String,

    /// Current status
    pub status: UploadStatus,

    /// Upload timestamp
    pub uploaded_at: Option<DateTime<Utc>>,

    /// Processing completion timestamp
    pub processed_at: Option<DateTime<Utc>>,

    /// Expiration timestamp (for builds that expire)
    pub expires_at: Option<DateTime<Utc>>,

    /// Distribution track/channel
    pub track: Option<String>,

    /// Rollout percentage if in staged rollout
    pub rollout_percentage: Option<f64>,

    /// Additional status details
    pub details: Option<String>,
}

/// Build summary for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Build {
    /// Build ID
    pub id: String,

    /// App version
    pub version: String,

    /// Build number
    pub build_number: String,

    /// Upload timestamp
    pub uploaded_at: DateTime<Utc>,

    /// Current status
    pub status: UploadStatus,

    /// Distribution track
    pub track: Option<String>,
}

/// Notarization status (Apple-specific but generic enough)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotarizationStatus {
    /// Submitted, waiting for processing
    InProgress,
    /// Successfully notarized
    Accepted,
    /// Notarization failed
    Invalid,
    /// Rejected
    Rejected,
}

impl std::fmt::Display for NotarizationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotarizationStatus::InProgress => write!(f, "In Progress"),
            NotarizationStatus::Accepted => write!(f, "Accepted"),
            NotarizationStatus::Invalid => write!(f, "Invalid"),
            NotarizationStatus::Rejected => write!(f, "Rejected"),
        }
    }
}

/// Notarization result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotarizationResult {
    /// Submission ID
    pub submission_id: String,

    /// Status
    pub status: NotarizationStatus,

    /// URL to notarization log
    pub log_url: Option<String>,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Issues found during notarization
    pub issues: Vec<NotarizationIssue>,
}

/// Issue found during notarization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotarizationIssue {
    /// Severity
    pub severity: String,

    /// Issue code
    pub code: Option<String>,

    /// Path to affected file
    pub path: Option<String>,

    /// Description
    pub message: String,
}

/// Apple-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleStoreConfig {
    /// App Store Connect API Key ID
    pub api_key_id: String,

    /// API Key Issuer ID
    pub api_issuer_id: String,

    /// Path to API key file (.p8) or the key contents
    pub api_key: String,

    /// Apple Team ID
    pub team_id: Option<String>,

    /// App ID (bundle identifier)
    pub app_id: Option<String>,

    /// Whether to notarize before upload
    pub notarize: bool,

    /// Whether to staple notarization ticket
    pub staple: bool,

    /// Primary locale (e.g., "en-US")
    pub primary_locale: Option<String>,
}

/// Google Play configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePlayConfig {
    /// Package name (e.g., "com.example.app")
    pub package_name: String,

    /// Path to service account JSON key file
    pub service_account_key: PathBuf,

    /// Default track for releases
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_store_type_display() {
        assert_eq!(StoreType::Apple.to_string(), "Apple");
        assert_eq!(StoreType::GooglePlay.to_string(), "Google Play");
    }

    #[test]
    fn test_validation_result() {
        let app_info = AppInfo {
            identifier: "com.example.app".to_string(),
            version: "1.0.0".to_string(),
            build_number: "1".to_string(),
            name: Some("Example App".to_string()),
            min_os_version: None,
            platforms: vec!["iOS".to_string()],
            size: 1024,
            sha256: None,
        };

        let result = ValidationResult::success(app_info);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_upload_status_display() {
        assert_eq!(UploadStatus::Processing.to_string(), "Processing");
        assert_eq!(UploadStatus::PendingReview.to_string(), "Pending Review");
    }
}
