//! Error types for signing operations

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias for signing operations
pub type Result<T> = std::result::Result<T, SigningError>;

/// Signing-related errors
#[derive(Debug, Error)]
pub enum SigningError {
    /// Signing identity not found
    #[error("Signing identity not found: {0}")]
    IdentityNotFound(String),

    /// Multiple identities match the criteria
    #[error("Multiple signing identities match '{query}', please be more specific")]
    AmbiguousIdentity { query: String },

    /// Signing failed
    #[error("Failed to sign {path}: {reason}")]
    SigningFailed { path: PathBuf, reason: String },

    /// Verification failed
    #[error("Signature verification failed for {path}: {reason}")]
    VerificationFailed { path: PathBuf, reason: String },

    /// Invalid signature
    #[error("Invalid signature on {0}")]
    InvalidSignature(PathBuf),

    /// No signature found
    #[error("No signature found on {0}")]
    NotSigned(PathBuf),

    /// Certificate expired
    #[error("Certificate has expired: {identity} (expired {expired_at})")]
    CertificateExpired {
        identity: String,
        expired_at: String,
    },

    /// Certificate not yet valid
    #[error("Certificate not yet valid: {identity} (valid from {valid_from})")]
    CertificateNotYetValid {
        identity: String,
        valid_from: String,
    },

    /// Entitlements error
    #[error("Entitlements error: {0}")]
    EntitlementsError(String),

    /// Provisioning profile error
    #[error("Provisioning profile error: {0}")]
    ProvisioningProfileError(String),

    /// Keychain error
    #[error("Keychain error: {0}")]
    KeychainError(String),

    /// Tool not found
    #[error("Signing tool not found: {tool}. {hint}")]
    ToolNotFound { tool: String, hint: String },

    /// Tool execution failed
    #[error("Signing tool failed: {tool} - {reason}")]
    ToolFailed { tool: String, reason: String },

    /// Notarization failed
    #[error("Notarization failed: {0}")]
    NotarizationFailed(String),

    /// Notarization timeout
    #[error("Notarization timed out after {0} seconds")]
    NotarizationTimeout(u64),

    /// Stapling failed
    #[error("Failed to staple notarization ticket: {0}")]
    StaplingFailed(String),

    /// Unsupported platform
    #[error("Signing provider '{provider}' is not supported on this platform")]
    UnsupportedPlatform { provider: String },

    /// Unsupported artifact type
    #[error("Cannot sign {path}: unsupported file type")]
    UnsupportedArtifact { path: PathBuf },

    /// Configuration error
    #[error("Signing configuration error: {0}")]
    ConfigError(String),

    /// Configuration error (alternate)
    #[error("Configuration error: {0}")]
    Configuration(String),

    /// Command execution error
    #[error("Command '{command}' failed with exit code {status}: {stderr}")]
    Command {
        command: String,
        status: i32,
        stderr: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON parsing error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}
