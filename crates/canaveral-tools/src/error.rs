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
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
