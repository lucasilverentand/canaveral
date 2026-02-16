//! Strategy registry

use std::sync::Arc;

use tracing::debug;

use crate::buildnum::BuildNumberStrategy;
use crate::calver::CalVerStrategy;
use crate::semver::SemVerStrategy;
use crate::traits::VersionStrategy;

/// Registry of available version strategies
pub struct StrategyRegistry {
    strategies: Vec<Arc<dyn VersionStrategy>>,
}

impl StrategyRegistry {
    /// Create a new registry with all built-in strategies
    pub fn new() -> Self {
        Self {
            strategies: vec![
                Arc::new(SemVerStrategy::new()),
                Arc::new(CalVerStrategy::default()),
                Arc::new(BuildNumberStrategy::new()),
            ],
        }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// Register a strategy
    pub fn register<S: VersionStrategy + 'static>(&mut self, strategy: S) {
        self.strategies.push(Arc::new(strategy));
    }

    /// Get strategy by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn VersionStrategy>> {
        let result = self.strategies.iter().find(|s| s.name() == name).cloned();
        debug!(
            strategy = name,
            found = result.is_some(),
            "strategy registry lookup"
        );
        result
    }

    /// Get all registered strategies
    pub fn all(&self) -> &[Arc<dyn VersionStrategy>] {
        &self.strategies
    }

    /// Get strategy names
    pub fn names(&self) -> Vec<&'static str> {
        self.strategies.iter().map(|s| s.name()).collect()
    }
}

impl Default for StrategyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = StrategyRegistry::empty();
        assert!(registry.all().is_empty());
        assert!(registry.names().is_empty());
        assert!(registry.get("semver").is_none());
    }

    #[test]
    fn test_default_registry_has_builtins() {
        let registry = StrategyRegistry::new();
        let names = registry.names();

        assert!(names.contains(&"semver"));
        assert!(names.contains(&"calver"));
        assert!(names.contains(&"buildnum"));
        assert_eq!(names.len(), 3);
    }

    #[test]
    fn test_get_by_name() {
        let registry = StrategyRegistry::new();

        assert!(registry.get("semver").is_some());
        assert!(registry.get("calver").is_some());
        assert!(registry.get("buildnum").is_some());
        assert!(registry.get("unknown").is_none());
    }

    #[test]
    fn test_register_custom() {
        let mut registry = StrategyRegistry::empty();
        assert!(registry.get("semver").is_none());

        registry.register(SemVerStrategy::new());
        assert!(registry.get("semver").is_some());
        assert_eq!(registry.names().len(), 1);
    }
}
