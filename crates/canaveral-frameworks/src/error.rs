//! Error types for framework adapters

use std::path::PathBuf;
use thiserror::Error;

/// Result type for framework operations
pub type Result<T> = std::result::Result<T, FrameworkError>;

/// Framework adapter errors
#[derive(Error, Debug)]
pub enum FrameworkError {
    /// No framework detected for the project
    #[error("No framework detected at {path}. Supported frameworks: {supported}")]
    NoFrameworkDetected {
        path: PathBuf,
        supported: String,
    },

    /// Multiple frameworks detected, disambiguation required
    #[error("Multiple frameworks detected: {frameworks:?}. Please specify with --framework")]
    AmbiguousFramework {
        frameworks: Vec<String>,
    },

    /// Framework tool not installed
    #[error("Required tool '{tool}' not found. {install_hint}")]
    ToolNotFound {
        tool: String,
        install_hint: String,
    },

    /// Build failed
    #[error("Build failed for {platform}: {message}")]
    BuildFailed {
        platform: String,
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Test failed
    #[error("Tests failed: {summary}")]
    TestFailed {
        summary: String,
        failed_count: usize,
        total_count: usize,
    },

    /// Screenshot capture failed
    #[error("Screenshot capture failed: {message}")]
    ScreenshotFailed {
        message: String,
    },

    /// Artifact not found after build
    #[error("Expected artifact not found at {expected_path}")]
    ArtifactNotFound {
        expected_path: PathBuf,
    },

    /// Invalid configuration
    #[error("Invalid configuration: {message}")]
    InvalidConfig {
        message: String,
    },

    /// Command execution failed
    #[error("Command failed: {command}")]
    CommandFailed {
        command: String,
        exit_code: Option<i32>,
        stdout: String,
        stderr: String,
    },

    /// Capability not supported
    #[error("Capability '{capability}' not supported by {framework}")]
    UnsupportedCapability {
        capability: String,
        framework: String,
    },

    /// Platform not supported
    #[error("Platform '{platform}' not supported by {framework}")]
    UnsupportedPlatform {
        platform: String,
        framework: String,
    },

    /// Version parsing error
    #[error("Failed to parse version: {message}")]
    VersionParseError {
        message: String,
    },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Timeout
    #[error("Operation timed out after {seconds}s")]
    Timeout {
        seconds: u64,
    },

    /// Generic error with context
    #[error("{context}: {message}")]
    Context {
        context: String,
        message: String,
    },
}

impl FrameworkError {
    /// Create a context error
    pub fn context(context: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Context {
            context: context.into(),
            message: message.into(),
        }
    }

    /// Create a build failed error
    pub fn build_failed(platform: impl Into<String>, message: impl Into<String>) -> Self {
        Self::BuildFailed {
            platform: platform.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Create a tool not found error with install hint
    pub fn tool_not_found(tool: impl Into<String>, install_hint: impl Into<String>) -> Self {
        Self::ToolNotFound {
            tool: tool.into(),
            install_hint: install_hint.into(),
        }
    }

    /// Check if this is a retryable error
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Timeout { .. } | Self::Io(_))
    }

    /// Get exit code for CLI
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoFrameworkDetected { .. } => 2,
            Self::AmbiguousFramework { .. } => 2,
            Self::ToolNotFound { .. } => 3,
            Self::BuildFailed { .. } => 10,
            Self::TestFailed { .. } => 11,
            Self::ScreenshotFailed { .. } => 12,
            Self::ArtifactNotFound { .. } => 13,
            Self::InvalidConfig { .. } => 4,
            Self::CommandFailed { exit_code, .. } => exit_code.unwrap_or(1),
            Self::UnsupportedCapability { .. } => 5,
            Self::UnsupportedPlatform { .. } => 5,
            Self::VersionParseError { .. } => 6,
            Self::Io(_) => 7,
            Self::Serialization(_) => 8,
            Self::Timeout { .. } => 9,
            Self::Context { .. } => 1,
        }
    }
}
