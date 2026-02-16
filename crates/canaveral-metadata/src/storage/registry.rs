//! Registry for metadata storage backends.

use std::path::Path;
use std::sync::Arc;

use tracing::debug;

use super::{FastlaneStorage, MetadataStorage, StorageFormat, UnifiedStorage};

/// A named entry in the metadata storage registry.
pub struct MetadataStorageEntry {
    /// Human-readable name for this storage backend.
    pub name: String,
    /// The storage format this entry uses.
    pub format: StorageFormat,
    /// The storage implementation.
    pub storage: Arc<dyn MetadataStorage>,
}

/// Registry of available metadata storage backends.
///
/// Provides lookup by name or format, and a factory constructor that
/// pre-registers the built-in backends (Fastlane and Unified).
pub struct MetadataStorageRegistry {
    entries: Vec<MetadataStorageEntry>,
}

impl MetadataStorageRegistry {
    /// Create a registry pre-populated with all built-in storage backends.
    pub fn new(base_path: &Path) -> Self {
        Self {
            entries: vec![
                MetadataStorageEntry {
                    name: "fastlane".to_string(),
                    format: StorageFormat::Fastlane,
                    storage: Arc::new(FastlaneStorage::new(base_path)),
                },
                MetadataStorageEntry {
                    name: "unified".to_string(),
                    format: StorageFormat::Unified,
                    storage: Arc::new(UnifiedStorage::new(base_path)),
                },
            ],
        }
    }

    /// Create an empty registry with no backends registered.
    pub fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a new storage backend.
    pub fn register(
        &mut self,
        name: impl Into<String>,
        format: StorageFormat,
        storage: Arc<dyn MetadataStorage>,
    ) {
        let name = name.into();
        debug!(name = %name, format = ?format, "registering metadata storage backend");
        self.entries.push(MetadataStorageEntry {
            name,
            format,
            storage,
        });
    }

    /// Look up a storage backend by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn MetadataStorage>> {
        self.entries
            .iter()
            .find(|e| e.name == name)
            .map(|e| Arc::clone(&e.storage))
    }

    /// Look up the first storage backend matching a given format.
    pub fn get_by_format(&self, format: StorageFormat) -> Option<Arc<dyn MetadataStorage>> {
        self.entries
            .iter()
            .find(|e| e.format == format)
            .map(|e| Arc::clone(&e.storage))
    }

    /// Return all registered backends as `(name, storage)` pairs.
    pub fn all(&self) -> Vec<(&str, Arc<dyn MetadataStorage>)> {
        self.entries
            .iter()
            .map(|e| (e.name.as_str(), Arc::clone(&e.storage)))
            .collect()
    }

    /// Return the names of all registered backends.
    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.name.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_empty() {
        let registry = MetadataStorageRegistry::empty();
        assert!(registry.names().is_empty());
        assert!(registry.get("fastlane").is_none());
        assert!(registry.get_by_format(StorageFormat::Fastlane).is_none());
        assert!(registry.all().is_empty());
    }

    #[test]
    fn test_new_has_builtins() {
        let registry = MetadataStorageRegistry::new(Path::new("/tmp/metadata"));
        assert_eq!(registry.names(), vec!["fastlane", "unified"]);
        assert!(registry.get("fastlane").is_some());
        assert!(registry.get("unified").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_get_by_format() {
        let registry = MetadataStorageRegistry::new(Path::new("/tmp/metadata"));
        assert!(registry.get_by_format(StorageFormat::Fastlane).is_some());
        assert!(registry.get_by_format(StorageFormat::Unified).is_some());
    }

    #[test]
    fn test_register_custom() {
        let mut registry = MetadataStorageRegistry::empty();
        let storage = Arc::new(FastlaneStorage::new("/tmp/custom"));
        registry.register("custom", StorageFormat::Fastlane, storage);
        assert_eq!(registry.names(), vec!["custom"]);
        assert!(registry.get("custom").is_some());
    }

    #[test]
    fn test_all() {
        let registry = MetadataStorageRegistry::new(Path::new("/tmp/metadata"));
        let all = registry.all();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].0, "fastlane");
        assert_eq!(all[1].0, "unified");
    }
}
