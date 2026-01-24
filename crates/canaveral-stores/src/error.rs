//! Store error types

use thiserror::Error;

/// Store-related errors
#[derive(Debug, Error)]
pub enum StoreError {
    /// Authentication failed
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Invalid credentials
    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    /// API error from store
    #[error("API error: {status} - {message}")]
    ApiError { status: u16, message: String },

    /// Rate limited by store
    #[error("Rate limited, retry after {retry_after:?} seconds")]
    RateLimited { retry_after: Option<u64> },

    /// Invalid artifact
    #[error("Invalid artifact: {0}")]
    InvalidArtifact(String),

    /// Upload failed
    #[error("Upload failed: {0}")]
    UploadFailed(String),

    /// Validation failed
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Notarization failed
    #[error("Notarization failed: {0}")]
    NotarizationFailed(String),

    /// Build not found
    #[error("Build not found: {0}")]
    BuildNotFound(String),

    /// App not found
    #[error("App not found: {0}")]
    AppNotFound(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigurationError(String),

    /// Tool not found
    #[error("Required tool not found: {0}")]
    ToolNotFound(String),

    /// Command execution failed
    #[error("Command failed: {0}")]
    CommandFailed(String),

    /// Timeout
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// JWT error
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),

    /// Other error
    #[error("{0}")]
    Other(String),
}

/// Result type for store operations
pub type Result<T> = std::result::Result<T, StoreError>;
