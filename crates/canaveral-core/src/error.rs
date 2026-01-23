//! Error types for Canaveral

use std::path::PathBuf;
use thiserror::Error;

/// Result type alias using CanaveralError
pub type Result<T> = std::result::Result<T, CanaveralError>;

/// Main error type for Canaveral operations
#[derive(Debug, Error)]
pub enum CanaveralError {
    /// Configuration-related errors
    #[error(transparent)]
    Config(#[from] ConfigError),

    /// Git-related errors
    #[error(transparent)]
    Git(#[from] GitError),

    /// Version-related errors
    #[error(transparent)]
    Version(#[from] VersionError),

    /// Changelog-related errors
    #[error(transparent)]
    Changelog(#[from] ChangelogError),

    /// Adapter-related errors
    #[error(transparent)]
    Adapter(#[from] AdapterError),

    /// Workflow-related errors
    #[error(transparent)]
    Workflow(#[from] WorkflowError),

    /// IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error
    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    /// JSON parsing error
    #[error("JSON parsing error: {0}")]
    Json(#[from] serde_json::Error),

    /// Generic errors
    #[error("{0}")]
    Other(String),
}

/// Configuration-related errors
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Configuration file not found
    #[error("Configuration file not found at {0}")]
    NotFound(PathBuf),

    /// Failed to parse configuration
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Invalid configuration value
    #[error("Invalid configuration: {field} - {message}")]
    InvalidValue { field: String, message: String },

    /// Missing required field
    #[error("Missing required configuration field: {0}")]
    MissingField(String),

    /// YAML parsing error
    #[error("YAML parsing error: {0}")]
    YamlError(#[from] serde_yaml::Error),

    /// TOML parsing error
    #[error("TOML parsing error: {0}")]
    TomlError(#[from] toml::de::Error),

    /// IO error
    #[error("IO error reading config: {0}")]
    Io(#[from] std::io::Error),
}

/// Git-related errors
#[derive(Debug, Error)]
pub enum GitError {
    /// Repository not found
    #[error("Git repository not found at {0}")]
    RepositoryNotFound(PathBuf),

    /// Not a git repository
    #[error("Not a git repository: {0}")]
    NotARepository(PathBuf),

    /// Failed to open repository
    #[error("Failed to open repository: {0}")]
    OpenFailed(String),

    /// No commits found
    #[error("No commits found in repository")]
    NoCommits,

    /// No tags found
    #[error("No tags found matching pattern: {0}")]
    NoTags(String),

    /// Tag already exists
    #[error("Tag already exists: {0}")]
    TagExists(String),

    /// Failed to create tag
    #[error("Failed to create tag {name}: {reason}")]
    TagCreationFailed { name: String, reason: String },

    /// Working directory is not clean
    #[error("Working directory has uncommitted changes")]
    DirtyWorkingDirectory,

    /// Failed to push
    #[error("Failed to push to remote: {0}")]
    PushFailed(String),

    /// Remote not found
    #[error("Remote not found: {0}")]
    RemoteNotFound(String),

    /// Git2 library error
    #[error("Git error: {0}")]
    Git2(#[from] git2::Error),
}

/// Version-related errors
#[derive(Debug, Error)]
pub enum VersionError {
    /// Failed to parse version
    #[error("Failed to parse version '{0}': {1}")]
    ParseFailed(String, String),

    /// Invalid version format
    #[error("Invalid version format: {0}")]
    InvalidFormat(String),

    /// No version bump required
    #[error("No version bump required - no relevant commits found")]
    NoBumpRequired,

    /// Invalid bump type
    #[error("Invalid bump type: {0}")]
    InvalidBumpType(String),

    /// Semver error
    #[error("Semver error: {0}")]
    Semver(#[from] semver::Error),
}

/// Changelog-related errors
#[derive(Debug, Error)]
pub enum ChangelogError {
    /// Failed to parse commit
    #[error("Failed to parse commit: {0}")]
    ParseFailed(String),

    /// Failed to generate changelog
    #[error("Failed to generate changelog: {0}")]
    GenerationFailed(String),

    /// Changelog file not found
    #[error("Changelog file not found at {0}")]
    FileNotFound(PathBuf),

    /// Failed to write changelog
    #[error("Failed to write changelog: {0}")]
    WriteFailed(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Adapter-related errors
#[derive(Debug, Error)]
pub enum AdapterError {
    /// Package manifest not found
    #[error("Package manifest not found at {0}")]
    ManifestNotFound(PathBuf),

    /// Failed to parse manifest
    #[error("Failed to parse manifest: {0}")]
    ManifestParseError(String),

    /// Failed to update manifest
    #[error("Failed to update manifest: {0}")]
    ManifestUpdateError(String),

    /// Publish failed
    #[error("Failed to publish package: {0}")]
    PublishFailed(String),

    /// Authentication failed
    #[error("Authentication failed for registry {registry}: {reason}")]
    AuthenticationFailed { registry: String, reason: String },

    /// Unsupported package type
    #[error("Unsupported package type: {0}")]
    UnsupportedType(String),

    /// Command execution failed
    #[error("Command failed: {command} - {reason}")]
    CommandFailed { command: String, reason: String },

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Workflow-related errors
#[derive(Debug, Error)]
pub enum WorkflowError {
    /// Validation failed
    #[error("Validation failed: {0}")]
    ValidationFailed(String),

    /// Pre-condition not met
    #[error("Pre-condition not met: {0}")]
    PreConditionFailed(String),

    /// Step failed
    #[error("Workflow step '{step}' failed: {reason}")]
    StepFailed { step: String, reason: String },

    /// Dry run mode - no changes made
    #[error("Dry run completed - no changes made")]
    DryRun,

    /// User cancelled
    #[error("Operation cancelled by user")]
    Cancelled,
}

impl CanaveralError {
    /// Create a new "other" error with a message
    pub fn other<S: Into<String>>(msg: S) -> Self {
        Self::Other(msg.into())
    }
}
