//! Error types for metadata operations.

use thiserror::Error;

/// Errors that can occur during metadata operations.
#[derive(Debug, Error)]
pub enum MetadataError {
    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Resource not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// Invalid format encountered.
    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    /// Validation failed.
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Storage operation failed.
    #[error("Storage error: {0}")]
    StorageError(String),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    SerializationError(String),

    /// Sync operation failed.
    #[error("Sync error: {0}")]
    SyncError(String),

    /// API rate limit exceeded.
    #[error("Rate limited: {0}")]
    RateLimited(String),

    /// Authentication failed.
    #[error("Authentication failed: {0}")]
    AuthenticationError(String),
}

impl From<serde_json::Error> for MetadataError {
    fn from(err: serde_json::Error) -> Self {
        MetadataError::SerializationError(err.to_string())
    }
}

impl From<serde_yaml::Error> for MetadataError {
    fn from(err: serde_yaml::Error) -> Self {
        MetadataError::SerializationError(err.to_string())
    }
}
