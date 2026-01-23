//! Migration Tools - Migrate from other release tools to Canaveral
//!
//! Supports migration from:
//! - semantic-release
//! - release-please
//! - standard-version
//! - lerna

use std::path::Path;

use crate::config::Config;
use crate::error::{CanaveralError, Result};

mod release_please;
mod semantic_release;

pub use release_please::ReleasePleaseMigrator;
pub use semantic_release::SemanticReleaseMigrator;

/// Migration source type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationSource {
    /// semantic-release configuration
    SemanticRelease,
    /// release-please configuration
    ReleasePlease,
    /// standard-version configuration
    StandardVersion,
    /// Lerna configuration
    Lerna,
}

impl MigrationSource {
    /// Get display name
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SemanticRelease => "semantic-release",
            Self::ReleasePlease => "release-please",
            Self::StandardVersion => "standard-version",
            Self::Lerna => "lerna",
        }
    }

    /// Get config file names to look for
    pub fn config_files(&self) -> &'static [&'static str] {
        match self {
            Self::SemanticRelease => &[
                ".releaserc",
                ".releaserc.json",
                ".releaserc.yaml",
                ".releaserc.yml",
                ".releaserc.js",
                "release.config.js",
            ],
            Self::ReleasePlease => &[
                "release-please-config.json",
                ".release-please-manifest.json",
            ],
            Self::StandardVersion => &[".versionrc", ".versionrc.json", ".versionrc.js"],
            Self::Lerna => &["lerna.json"],
        }
    }
}

/// Migration result
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Source tool that was migrated from
    pub source: MigrationSource,
    /// Generated Canaveral configuration
    pub config: Config,
    /// Migration warnings
    pub warnings: Vec<String>,
    /// Unsupported features that couldn't be migrated
    pub unsupported: Vec<String>,
    /// Suggested manual steps
    pub manual_steps: Vec<String>,
}

impl MigrationResult {
    /// Create a new migration result
    pub fn new(source: MigrationSource, config: Config) -> Self {
        Self {
            source,
            config,
            warnings: Vec::new(),
            unsupported: Vec::new(),
            manual_steps: Vec::new(),
        }
    }

    /// Add a warning
    pub fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    /// Add an unsupported feature
    pub fn unsupported(&mut self, msg: impl Into<String>) {
        self.unsupported.push(msg.into());
    }

    /// Add a manual step
    pub fn manual_step(&mut self, msg: impl Into<String>) {
        self.manual_steps.push(msg.into());
    }

    /// Check if migration has any issues
    pub fn has_issues(&self) -> bool {
        !self.warnings.is_empty() || !self.unsupported.is_empty()
    }
}

/// Trait for migrating from a specific tool
pub trait Migrator {
    /// Get the source tool type
    fn source(&self) -> MigrationSource;

    /// Check if this migrator can handle the given path
    fn can_migrate(&self, path: &Path) -> bool;

    /// Detect the configuration file
    fn detect_config(&self, path: &Path) -> Option<std::path::PathBuf>;

    /// Perform the migration
    fn migrate(&self, path: &Path) -> Result<MigrationResult>;
}

/// Auto-detect and migrate from any supported tool
pub fn auto_migrate(path: &Path) -> Result<MigrationResult> {
    let migrators: Vec<Box<dyn Migrator>> = vec![
        Box::new(SemanticReleaseMigrator::new()),
        Box::new(ReleasePleaseMigrator::new()),
    ];

    for migrator in migrators {
        if migrator.can_migrate(path) {
            return migrator.migrate(path);
        }
    }

    Err(CanaveralError::other(
        "No supported release tool configuration found",
    ))
}

/// Detect which release tool is configured
pub fn detect_tool(path: &Path) -> Option<MigrationSource> {
    for source in [
        MigrationSource::SemanticRelease,
        MigrationSource::ReleasePlease,
        MigrationSource::StandardVersion,
        MigrationSource::Lerna,
    ] {
        for config_file in source.config_files() {
            if path.join(config_file).exists() {
                return Some(source);
            }
        }
    }

    // Check package.json for release config
    let package_json = path.join("package.json");
    if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if content.contains("\"release\"") {
                return Some(MigrationSource::SemanticRelease);
            }
            if content.contains("\"standard-version\"") {
                return Some(MigrationSource::StandardVersion);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_migration_source() {
        assert_eq!(MigrationSource::SemanticRelease.as_str(), "semantic-release");
        assert_eq!(MigrationSource::ReleasePlease.as_str(), "release-please");
    }

    #[test]
    fn test_detect_tool_none() {
        let temp = TempDir::new().unwrap();
        assert!(detect_tool(temp.path()).is_none());
    }

    #[test]
    fn test_detect_semantic_release() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".releaserc"), "{}").unwrap();
        assert_eq!(detect_tool(temp.path()), Some(MigrationSource::SemanticRelease));
    }

    #[test]
    fn test_detect_release_please() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("release-please-config.json"), "{}").unwrap();
        assert_eq!(detect_tool(temp.path()), Some(MigrationSource::ReleasePlease));
    }

    #[test]
    fn test_migration_result() {
        let config = Config::default();
        let mut result = MigrationResult::new(MigrationSource::SemanticRelease, config);

        result.warn("Test warning");
        result.unsupported("Unsupported feature");
        result.manual_step("Do this manually");

        assert!(result.has_issues());
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.unsupported.len(), 1);
        assert_eq!(result.manual_steps.len(), 1);
    }
}
