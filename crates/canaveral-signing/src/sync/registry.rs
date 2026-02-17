//! Storage backend registry

use std::sync::Arc;
use tracing::debug;

use super::storage::StorageBackend;

/// A named storage backend entry
pub struct StorageBackendEntry {
    /// Name identifying this backend
    pub name: String,
    /// The storage backend instance
    pub backend: Arc<dyn StorageBackend>,
}

/// Registry of available storage backends
pub struct StorageBackendRegistry {
    backends: Vec<StorageBackendEntry>,
}

impl StorageBackendRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            backends: Vec::new(),
        }
    }

    /// Register a storage backend with a name
    pub fn register(&mut self, name: impl Into<String>, backend: Arc<dyn StorageBackend>) {
        let name = name.into();
        debug!(backend = %name, "Registered storage backend");
        self.backends.push(StorageBackendEntry { name, backend });
    }

    /// Get a storage backend by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn StorageBackend>> {
        let result = self
            .backends
            .iter()
            .find(|e| e.name == name)
            .map(|e| Arc::clone(&e.backend));
        debug!(
            backend = name,
            found = result.is_some(),
            "Looking up storage backend"
        );
        result
    }

    /// Get all registered backend entries
    pub fn all(&self) -> &[StorageBackendEntry] {
        &self.backends
    }

    /// Get all registered backend names
    pub fn names(&self) -> Vec<&str> {
        self.backends.iter().map(|e| e.name.as_str()).collect()
    }
}

impl Default for StorageBackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use async_trait::async_trait;

    struct MockStorage;

    #[async_trait]
    impl StorageBackend for MockStorage {
        async fn sync(&self) -> Result<()> {
            Ok(())
        }
        async fn read(&self, _path: &str) -> Result<Vec<u8>> {
            Ok(vec![])
        }
        async fn write(&self, _path: &str, _data: &[u8]) -> Result<()> {
            Ok(())
        }
        async fn delete(&self, _path: &str) -> Result<()> {
            Ok(())
        }
        async fn list(&self, _prefix: &str) -> Result<Vec<String>> {
            Ok(vec![])
        }
        async fn exists(&self, _path: &str) -> Result<bool> {
            Ok(false)
        }
    }

    #[test]
    fn test_empty() {
        let registry = StorageBackendRegistry::new();
        assert!(registry.all().is_empty());
        assert!(registry.names().is_empty());
        assert!(registry.get("git").is_none());
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = StorageBackendRegistry::new();
        registry.register("mock", Arc::new(MockStorage));
        registry.register("another", Arc::new(MockStorage));

        assert_eq!(registry.names(), vec!["mock", "another"]);
        assert_eq!(registry.all().len(), 2);
        assert!(registry.get("mock").is_some());
        assert!(registry.get("another").is_some());
        assert!(registry.get("nonexistent").is_none());
    }
}
