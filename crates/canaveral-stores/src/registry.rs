//! Store adapter registry

use std::sync::Arc;
use tracing::debug;

use crate::traits::StoreAdapter;
use crate::types::StoreType;

/// Registry of available store adapters
pub struct StoreRegistry {
    stores: Vec<Arc<dyn StoreAdapter>>,
}

impl StoreRegistry {
    /// Create a new empty registry
    ///
    /// Unlike AdapterRegistry, stores require configuration so none are
    /// registered by default.
    pub fn new() -> Self {
        Self {
            stores: Vec::new(),
        }
    }

    /// Register a store adapter
    pub fn register<S: StoreAdapter + 'static>(&mut self, store: S) {
        self.stores.push(Arc::new(store));
    }

    /// Register a pre-built store adapter
    pub fn register_arc(&mut self, store: Arc<dyn StoreAdapter>) {
        self.stores.push(store);
    }

    /// Get store adapter by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn StoreAdapter>> {
        let result = self.stores.iter().find(|s| s.name() == name).cloned();
        debug!(store = name, found = result.is_some(), "Looking up store adapter");
        result
    }

    /// Get all store adapters matching a given store type
    pub fn get_by_type(&self, store_type: StoreType) -> Vec<Arc<dyn StoreAdapter>> {
        let results: Vec<_> = self.stores
            .iter()
            .filter(|s| s.store_type() == store_type)
            .cloned()
            .collect();
        debug!(store_type = ?store_type, count = results.len(), "Looking up stores by type");
        results
    }

    /// Get all registered store adapters
    pub fn all(&self) -> &[Arc<dyn StoreAdapter>] {
        &self.stores
    }

    /// Get names of all registered store adapters
    pub fn names(&self) -> Vec<String> {
        self.stores.iter().map(|s| s.name().to_string()).collect()
    }

    /// Get only store adapters that are currently available
    pub fn available(&self) -> Vec<Arc<dyn StoreAdapter>> {
        let available: Vec<_> = self.stores
            .iter()
            .filter(|s| s.is_available())
            .cloned()
            .collect();
        let names: Vec<_> = available.iter().map(|s| s.name()).collect();
        debug!(count = available.len(), stores = ?names, "Queried available store adapters");
        available
    }
}

impl Default for StoreRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Result;
    use crate::types::*;
    use std::path::Path;

    struct MockStore {
        mock_name: String,
        mock_type: StoreType,
        mock_available: bool,
    }

    impl MockStore {
        fn new(name: &str, store_type: StoreType, available: bool) -> Self {
            Self {
                mock_name: name.to_string(),
                mock_type: store_type,
                mock_available: available,
            }
        }
    }

    #[async_trait::async_trait]
    impl StoreAdapter for MockStore {
        fn name(&self) -> &str {
            &self.mock_name
        }

        fn store_type(&self) -> StoreType {
            self.mock_type
        }

        fn is_available(&self) -> bool {
            self.mock_available
        }

        async fn validate_artifact(&self, _path: &Path) -> Result<ValidationResult> {
            unimplemented!()
        }

        async fn upload(&self, _path: &Path, _options: &UploadOptions) -> Result<UploadResult> {
            unimplemented!()
        }

        async fn get_build_status(&self, _build_id: &str) -> Result<BuildStatus> {
            unimplemented!()
        }

        async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
            unimplemented!()
        }

        fn supported_extensions(&self) -> &[&str] {
            &[]
        }
    }

    #[test]
    fn test_empty_registry() {
        let registry = StoreRegistry::new();
        assert!(registry.all().is_empty());
        assert!(registry.names().is_empty());
        assert!(registry.get("anything").is_none());
        assert!(registry.get_by_type(StoreType::Apple).is_empty());
        assert!(registry.available().is_empty());
    }

    #[test]
    fn test_register_and_get() {
        let mut registry = StoreRegistry::new();
        registry.register(MockStore::new("apple", StoreType::Apple, true));
        registry.register(MockStore::new("google", StoreType::GooglePlay, false));

        assert_eq!(registry.all().len(), 2);
        assert!(registry.get("apple").is_some());
        assert!(registry.get("google").is_some());
        assert!(registry.get("missing").is_none());

        let names = registry.names();
        assert_eq!(names, vec!["apple".to_string(), "google".to_string()]);
    }

    #[test]
    fn test_get_by_type() {
        let mut registry = StoreRegistry::new();
        registry.register(MockStore::new("apple-1", StoreType::Apple, true));
        registry.register(MockStore::new("google", StoreType::GooglePlay, true));
        registry.register(MockStore::new("apple-2", StoreType::Apple, false));

        let apple_stores = registry.get_by_type(StoreType::Apple);
        assert_eq!(apple_stores.len(), 2);

        let google_stores = registry.get_by_type(StoreType::GooglePlay);
        assert_eq!(google_stores.len(), 1);

        let ms_stores = registry.get_by_type(StoreType::Microsoft);
        assert!(ms_stores.is_empty());
    }

    #[test]
    fn test_available() {
        let mut registry = StoreRegistry::new();
        registry.register(MockStore::new("apple", StoreType::Apple, true));
        registry.register(MockStore::new("google", StoreType::GooglePlay, false));
        registry.register(MockStore::new("npm", StoreType::Npm, true));

        let available = registry.available();
        assert_eq!(available.len(), 2);
        assert_eq!(available[0].name(), "apple");
        assert_eq!(available[1].name(), "npm");
    }

    #[test]
    fn test_register_arc() {
        let mut registry = StoreRegistry::new();
        let store: Arc<dyn StoreAdapter> =
            Arc::new(MockStore::new("shared", StoreType::Crates, true));
        registry.register_arc(store.clone());

        assert_eq!(registry.all().len(), 1);
        assert!(registry.get("shared").is_some());
    }
}
