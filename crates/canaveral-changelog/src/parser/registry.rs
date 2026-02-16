//! Parser registry

use std::sync::Arc;

use super::CommitParser;
use super::ConventionalParser;

/// Registry of available commit parsers
pub struct ParserRegistry {
    parsers: Vec<Arc<dyn CommitParser>>,
}

impl ParserRegistry {
    /// Create a new registry with all built-in parsers
    pub fn new() -> Self {
        Self {
            parsers: vec![Arc::new(ConventionalParser::new())],
        }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            parsers: Vec::new(),
        }
    }

    /// Register a parser
    pub fn register<P: CommitParser + 'static>(&mut self, parser: P) {
        self.parsers.push(Arc::new(parser));
    }

    /// Get the default (first registered) parser
    pub fn default(&self) -> Option<Arc<dyn CommitParser>> {
        self.parsers.first().cloned()
    }

    /// Get all registered parsers
    pub fn all(&self) -> &[Arc<dyn CommitParser>] {
        &self.parsers
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ParserRegistry::new();
        assert_eq!(registry.parsers.len(), 1);
    }

    #[test]
    fn test_default_parser() {
        let registry = ParserRegistry::new();
        assert!(registry.default().is_some());
    }

    #[test]
    fn test_empty_registry() {
        let registry = ParserRegistry::empty();
        assert!(registry.default().is_none());
        assert!(registry.all().is_empty());
    }
}
