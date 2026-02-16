//! Formatter registry

use std::sync::Arc;

use super::ChangelogFormatter;
use super::MarkdownFormatter;

/// Registry of available changelog formatters
pub struct FormatterRegistry {
    formatters: Vec<Arc<dyn ChangelogFormatter>>,
}

impl FormatterRegistry {
    /// Create a new registry with all built-in formatters
    pub fn new() -> Self {
        Self {
            formatters: vec![Arc::new(MarkdownFormatter::new())],
        }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            formatters: Vec::new(),
        }
    }

    /// Register a formatter
    pub fn register<F: ChangelogFormatter + 'static>(&mut self, formatter: F) {
        self.formatters.push(Arc::new(formatter));
    }

    /// Get formatter by file extension
    pub fn get(&self, extension: &str) -> Option<Arc<dyn ChangelogFormatter>> {
        self.formatters
            .iter()
            .find(|f| f.extension() == extension)
            .cloned()
    }

    /// Get all registered formatters
    pub fn all(&self) -> &[Arc<dyn ChangelogFormatter>] {
        &self.formatters
    }

    /// Get all supported file extensions
    pub fn extensions(&self) -> Vec<&'static str> {
        self.formatters.iter().map(|f| f.extension()).collect()
    }
}

impl Default for FormatterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = FormatterRegistry::new();
        assert_eq!(registry.formatters.len(), 1);
    }

    #[test]
    fn test_get_by_extension() {
        let registry = FormatterRegistry::new();
        assert!(registry.get("md").is_some());
        assert!(registry.get("html").is_none());
    }

    #[test]
    fn test_extensions() {
        let registry = FormatterRegistry::new();
        let exts = registry.extensions();
        assert!(exts.contains(&"md"));
    }

    #[test]
    fn test_empty_registry() {
        let registry = FormatterRegistry::empty();
        assert!(registry.all().is_empty());
    }
}
