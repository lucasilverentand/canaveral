//! Parser configuration types

use std::collections::HashSet;

/// Configuration for the commit parser
#[derive(Debug, Clone)]
pub struct ParserConfig {
    /// Commit types to include
    pub include_types: HashSet<String>,
    /// Commit types to exclude
    pub exclude_types: HashSet<String>,
    /// Whether to include commits without a type
    pub include_untyped: bool,
    /// Whether to include merge commits
    pub include_merges: bool,
}

impl Default for ParserConfig {
    fn default() -> Self {
        Self {
            include_types: HashSet::new(),
            exclude_types: HashSet::new(),
            include_untyped: false,
            include_merges: false,
        }
    }
}

impl ParserConfig {
    /// Create a new config that includes all types
    pub fn all() -> Self {
        Self {
            include_types: HashSet::new(),
            exclude_types: HashSet::new(),
            include_untyped: true,
            include_merges: true,
        }
    }

    /// Add a type to include
    pub fn include_type(mut self, type_name: impl Into<String>) -> Self {
        self.include_types.insert(type_name.into());
        self
    }

    /// Add a type to exclude
    pub fn exclude_type(mut self, type_name: impl Into<String>) -> Self {
        self.exclude_types.insert(type_name.into());
        self
    }

    /// Set whether to include untyped commits
    pub fn with_untyped(mut self, include: bool) -> Self {
        self.include_untyped = include;
        self
    }

    /// Set whether to include merge commits
    pub fn with_merges(mut self, include: bool) -> Self {
        self.include_merges = include;
        self
    }
}
