//! Error types for canaveral-tools

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ToolError {
    #[error("tool '{0}' not found")]
    NotFound(String),
    #[error("version '{version}' not available for tool '{tool}'")]
    VersionNotAvailable { tool: String, version: String },
    #[error("installation failed for {tool} {version}: {reason}")]
    InstallFailed {
        tool: String,
        version: String,
        reason: String,
    },
    #[error("detection failed: {0}")]
    DetectionFailed(String),
    #[error("unsupported platform for tool '{0}'")]
    UnsupportedPlatform(String),
    #[error("registry fetch failed for '{tool}': {reason}")]
    RegistryFetchFailed { tool: String, reason: String },
    #[error("checksum mismatch for {tool} {version}: expected {expected}, got {actual}")]
    ChecksumMismatch {
        tool: String,
        version: String,
        expected: String,
        actual: String,
    },
    #[error("extraction failed for {tool} {version}: {reason}")]
    ExtractionFailed {
        tool: String,
        version: String,
        reason: String,
    },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
