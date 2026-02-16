//! CI template registry

use std::sync::Arc;

use super::github::GitHubActionsTemplate;
use super::gitlab::GitLabCITemplate;
use super::CITemplate;

/// Registry of available CI templates
pub struct CITemplateRegistry {
    templates: Vec<Arc<dyn CITemplate>>,
}

impl CITemplateRegistry {
    /// Create a new registry with all built-in templates
    pub fn new() -> Self {
        Self {
            templates: vec![
                Arc::new(GitHubActionsTemplate::new()),
                Arc::new(GitLabCITemplate::new()),
            ],
        }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            templates: Vec::new(),
        }
    }

    /// Register a template
    pub fn register<T: CITemplate + 'static>(&mut self, template: T) {
        self.templates.push(Arc::new(template));
    }

    /// Get template by platform name
    ///
    /// Matches exactly or case-insensitively against the start of the platform name,
    /// so both "github" and "GitHub Actions" will match.
    pub fn get(&self, platform: &str) -> Option<Arc<dyn CITemplate>> {
        let lower = platform.to_lowercase();
        self.templates
            .iter()
            .find(|t| {
                let name = t.platform_name();
                name == platform || name.to_lowercase().starts_with(&lower)
            })
            .cloned()
    }

    /// Get all registered templates
    pub fn all(&self) -> &[Arc<dyn CITemplate>] {
        &self.templates
    }

    /// Get platform names
    pub fn platform_names(&self) -> Vec<&'static str> {
        self.templates.iter().map(|t| t.platform_name()).collect()
    }
}

impl Default for CITemplateRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = CITemplateRegistry::empty();
        assert!(registry.all().is_empty());
        assert!(registry.get("github").is_none());
        assert!(registry.platform_names().is_empty());
    }

    #[test]
    fn test_default_has_builtins() {
        let registry = CITemplateRegistry::new();
        assert!(registry.get("github").is_some());
        assert!(registry.get("gitlab").is_some());
    }

    #[test]
    fn test_get_by_platform() {
        let registry = CITemplateRegistry::new();
        let github = registry.get("github").unwrap();
        assert_eq!(github.platform_name(), "GitHub Actions");

        let gitlab = registry.get("gitlab").unwrap();
        assert_eq!(gitlab.platform_name(), "GitLab CI");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_platform_names() {
        let registry = CITemplateRegistry::new();
        let names = registry.platform_names();
        assert!(names.contains(&"GitHub Actions"));
        assert!(names.contains(&"GitLab CI"));
    }
}
