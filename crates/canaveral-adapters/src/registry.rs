//! Adapter registry

use std::path::Path;
use std::sync::Arc;

use crate::cargo::CargoAdapter;
use crate::npm::NpmAdapter;
use crate::python::PythonAdapter;
use crate::traits::PackageAdapter;

/// Registry of available package adapters
pub struct AdapterRegistry {
    adapters: Vec<Arc<dyn PackageAdapter>>,
}

impl AdapterRegistry {
    /// Create a new registry with all built-in adapters
    pub fn new() -> Self {
        Self {
            adapters: vec![
                Arc::new(NpmAdapter::new()),
                Arc::new(CargoAdapter::new()),
                Arc::new(PythonAdapter::new()),
            ],
        }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            adapters: Vec::new(),
        }
    }

    /// Register an adapter
    pub fn register<A: PackageAdapter + 'static>(&mut self, adapter: A) {
        self.adapters.push(Arc::new(adapter));
    }

    /// Get adapter by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn PackageAdapter>> {
        self.adapters.iter().find(|a| a.name() == name).cloned()
    }

    /// Detect which adapter applies to a path
    pub fn detect(&self, path: &Path) -> Option<Arc<dyn PackageAdapter>> {
        self.adapters.iter().find(|a| a.detect(path)).cloned()
    }

    /// Get all registered adapters
    pub fn all(&self) -> &[Arc<dyn PackageAdapter>] {
        &self.adapters
    }

    /// Get adapter names
    pub fn names(&self) -> Vec<&'static str> {
        self.adapters.iter().map(|a| a.name()).collect()
    }
}

impl Default for AdapterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = AdapterRegistry::new();
        assert!(registry.adapters.len() >= 3);
    }

    #[test]
    fn test_get_adapter() {
        let registry = AdapterRegistry::new();

        assert!(registry.get("npm").is_some());
        assert!(registry.get("cargo").is_some());
        assert!(registry.get("python").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_adapter_names() {
        let registry = AdapterRegistry::new();
        let names = registry.names();

        assert!(names.contains(&"npm"));
        assert!(names.contains(&"cargo"));
        assert!(names.contains(&"python"));
    }
}
