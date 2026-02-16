//! Migrator registry — central catalog of migration providers.

use std::path::Path;

use crate::error::{CanaveralError, Result};

use super::{MigrationResult, MigrationSource, Migrator, ReleasePleaseMigrator, SemanticReleaseMigrator};

/// Central registry of migration providers.
///
/// Holds all known [`Migrator`] implementations and provides helpers for
/// detecting which tool is configured in a project directory and running
/// the appropriate migration.
pub struct MigratorRegistry {
    migrators: Vec<Box<dyn Migrator>>,
}

impl MigratorRegistry {
    /// Create a registry pre-populated with all built-in migrators.
    pub fn new() -> Self {
        Self {
            migrators: vec![
                Box::new(SemanticReleaseMigrator::new()),
                Box::new(ReleasePleaseMigrator::new()),
            ],
        }
    }

    /// Create an empty registry (useful for testing or fully custom setups).
    pub fn empty() -> Self {
        Self {
            migrators: Vec::new(),
        }
    }

    /// Register an additional migrator.
    pub fn register(&mut self, migrator: Box<dyn Migrator>) {
        self.migrators.push(migrator);
    }

    /// Detect which migrator can handle the project at `path`.
    ///
    /// Returns the first migrator whose [`Migrator::can_migrate`] returns `true`.
    pub fn detect(&self, path: &Path) -> Option<&dyn Migrator> {
        self.migrators
            .iter()
            .find(|m| m.can_migrate(path))
            .map(|m| m.as_ref())
    }

    /// Auto-detect and run the appropriate migration for `path`.
    pub fn migrate(&self, path: &Path) -> Result<MigrationResult> {
        if let Some(migrator) = self.detect(path) {
            migrator.migrate(path)
        } else {
            Err(CanaveralError::other(
                "No supported release tool configuration found",
            ))
        }
    }

    /// Return all registered migrators.
    pub fn all(&self) -> &[Box<dyn Migrator>] {
        &self.migrators
    }

    /// List the [`MigrationSource`] for every registered migrator.
    pub fn sources(&self) -> Vec<MigrationSource> {
        self.migrators.iter().map(|m| m.source()).collect()
    }
}

impl Default for MigratorRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_empty_registry() {
        let registry = MigratorRegistry::empty();
        assert!(registry.all().is_empty());
        assert!(registry.sources().is_empty());

        let temp = TempDir::new().unwrap();
        assert!(registry.detect(temp.path()).is_none());
        assert!(registry.migrate(temp.path()).is_err());
    }

    #[test]
    fn test_default_has_builtins() {
        let registry = MigratorRegistry::default();
        assert_eq!(registry.all().len(), 2);

        let sources = registry.sources();
        assert!(sources.contains(&MigrationSource::SemanticRelease));
        assert!(sources.contains(&MigrationSource::ReleasePlease));
    }

    #[test]
    fn test_detect_semantic_release() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(".releaserc"), "{}").unwrap();

        let registry = MigratorRegistry::new();
        let migrator = registry.detect(temp.path());
        assert!(migrator.is_some());
        assert_eq!(migrator.unwrap().source(), MigrationSource::SemanticRelease);
    }

    #[test]
    fn test_detect_release_please() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("release-please-config.json"), "{}").unwrap();

        let registry = MigratorRegistry::new();
        let migrator = registry.detect(temp.path());
        assert!(migrator.is_some());
        assert_eq!(migrator.unwrap().source(), MigrationSource::ReleasePlease);
    }

    #[test]
    fn test_register_custom() {
        let mut registry = MigratorRegistry::empty();
        assert!(registry.all().is_empty());

        registry.register(Box::new(SemanticReleaseMigrator::new()));
        assert_eq!(registry.all().len(), 1);
        assert_eq!(registry.sources(), vec![MigrationSource::SemanticRelease]);
    }
}
