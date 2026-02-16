//! Signing provider registry

use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info};

use crate::provider::SigningProvider;
use crate::providers;

/// Registry of available signing providers
pub struct SigningProviderRegistry {
    providers: Vec<Arc<dyn SigningProvider>>,
}

impl SigningProviderRegistry {
    /// Create a new registry with all built-in providers for the current platform
    pub fn new() -> Self {
        let mut providers: Vec<Arc<dyn SigningProvider>> = vec![
            Arc::new(providers::gpg::GpgProvider::new()),
            Arc::new(providers::android::AndroidProvider::new()),
        ];

        #[cfg(target_os = "macos")]
        providers.push(Arc::new(providers::macos::MacOSProvider::new()));

        #[cfg(target_os = "windows")]
        providers.push(Arc::new(providers::windows::WindowsProvider::new()));

        Self { providers }
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            providers: Vec::new(),
        }
    }

    /// Register a signing provider
    pub fn register<P: SigningProvider + 'static>(&mut self, provider: P) {
        self.providers.push(Arc::new(provider));
    }

    /// Register a pre-built signing provider
    pub fn register_arc(&mut self, provider: Arc<dyn SigningProvider>) {
        self.providers.push(provider);
    }

    /// Get a provider by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn SigningProvider>> {
        self.providers.iter().find(|p| p.name() == name).cloned()
    }

    /// Get all registered providers
    pub fn all(&self) -> &[Arc<dyn SigningProvider>] {
        &self.providers
    }

    /// Get provider names
    pub fn names(&self) -> Vec<String> {
        self.providers.iter().map(|p| p.name().to_string()).collect()
    }

    /// Get only providers that are available on the current system
    pub fn available(&self) -> Vec<Arc<dyn SigningProvider>> {
        let available: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.is_available())
            .cloned()
            .collect();
        let names: Vec<_> = available.iter().map(|p| p.name()).collect();
        debug!(count = available.len(), providers = ?names, "Queried available signing providers");
        available
    }

    /// Detect which providers support the given file
    pub fn detect(&self, path: &Path) -> Vec<Arc<dyn SigningProvider>> {
        let detected: Vec<_> = self
            .providers
            .iter()
            .filter(|p| p.supports_file(path))
            .cloned()
            .collect();
        let names: Vec<_> = detected.iter().map(|p| p.name()).collect();
        info!(path = %path.display(), providers = ?names, "Detected signing providers for file");
        detected
    }
}

impl Default for SigningProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_registry() {
        let registry = SigningProviderRegistry::empty();
        assert!(registry.all().is_empty());
        assert!(registry.names().is_empty());
    }

    #[test]
    fn test_default_registry_has_providers() {
        let registry = SigningProviderRegistry::new();
        assert!(registry.all().len() >= 2);
        assert!(registry.get("gpg").is_some());
        assert!(registry.get("android").is_some());
    }

    #[test]
    fn test_get_by_name() {
        let registry = SigningProviderRegistry::new();
        assert!(registry.get("gpg").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_detect_by_file() {
        let registry = SigningProviderRegistry::new();
        let apk_path = Path::new("app.apk");
        let providers = registry.detect(apk_path);
        assert!(!providers.is_empty());
        assert!(providers.iter().any(|p| p.name() == "android"));
    }
}
