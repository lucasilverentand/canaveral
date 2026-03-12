//! Tool provider registry

use std::collections::HashMap;
use std::sync::Arc;

use tracing::debug;

use crate::cache::ToolCache;
use crate::error::ToolError;
use crate::providers::{
    BunProvider, DartProvider, FlutterProvider, GenericProvider, GoProvider, GradleProvider,
    JavaProvider, NodeProvider, NpmProvider, PipProvider, PythonProvider,
};
use crate::traits::{ToolInfo, ToolProvider};

/// Registry of available tool providers
pub struct ToolRegistry {
    providers: HashMap<String, Arc<dyn ToolProvider>>,
}

impl ToolRegistry {
    /// Create a new registry with all built-in providers
    pub fn with_builtins() -> Self {
        let mut registry = Self::empty();
        // Providers are registered in dedicated tasks (bun, node, etc.)
        // This is the entry point for built-in provider registration.
        registry.register_builtins();
        registry
    }

    /// Create an empty registry
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Register a tool provider
    pub fn register<T: ToolProvider + 'static>(&mut self, provider: T) {
        let id = provider.id().to_string();
        debug!(tool = %id, "registering tool provider");
        self.providers.insert(id, Arc::new(provider));
    }

    /// Register an already-boxed provider
    pub fn register_arc(&mut self, provider: Arc<dyn ToolProvider>) {
        let id = provider.id().to_string();
        debug!(tool = %id, "registering tool provider");
        self.providers.insert(id, provider);
    }

    /// Get a provider by tool name.
    ///
    /// Falls back to the embedded tool definitions when no builtin
    /// provider is registered for the given name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn ToolProvider>> {
        if let Some(provider) = self.providers.get(name).cloned() {
            debug!(tool = name, "tool provider lookup: builtin");
            return Some(provider);
        }

        // Fallback: try embedded tool definitions
        if let Some(def) = crate::tool_defs::definitions().get(name) {
            debug!(tool = name, "tool provider lookup: embedded definition");
            return Some(Arc::new(GenericProvider::new(def.clone())));
        }

        debug!(tool = name, "tool provider lookup: not found");
        None
    }

    /// Get a provider by tool name with an explicit `source` override.
    ///
    /// When the user specifies `source = "owner/repo"` in the config,
    /// this creates a generic provider pointing at that repo directly.
    pub fn get_with_source(&self, name: &str, source: &str) -> Arc<dyn ToolProvider> {
        // Builtins take priority even if source is specified
        if let Some(provider) = self.providers.get(name).cloned() {
            return provider;
        }
        Arc::new(GenericProvider::from_repo(name, source))
    }

    /// Get all registered providers
    pub fn all(&self) -> Vec<Arc<dyn ToolProvider>> {
        self.providers.values().cloned().collect()
    }

    /// Check all tools against a requested version config, returning status for each
    pub async fn check_all(&self, config: &HashMap<String, String>) -> Vec<ToolInfo> {
        let mut results = Vec::new();

        for (tool_name, requested_version) in config {
            if let Some(provider) = self.get(tool_name) {
                let current_version = provider.detect_version().await.ok().flatten();
                let is_satisfied = if current_version.is_some() {
                    provider
                        .is_satisfied(requested_version)
                        .await
                        .unwrap_or(false)
                } else {
                    false
                };

                let install_path = which::which(provider.binary_name())
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()));

                results.push(ToolInfo {
                    name: tool_name.clone(),
                    current_version,
                    requested_version: Some(requested_version.clone()),
                    install_path,
                    is_satisfied,
                });
            } else {
                results.push(ToolInfo {
                    name: tool_name.clone(),
                    current_version: None,
                    requested_version: Some(requested_version.clone()),
                    install_path: None,
                    is_satisfied: false,
                });
            }
        }

        results
    }

    /// Ensure a tool version is available, installing it into the cache if needed.
    ///
    /// - If the version is already cached, touches its `.last_used` timestamp and
    ///   returns a `ToolInfo` pointing at the cached directory.
    /// - If not cached, calls `install_to_cache` on the provider, then touches.
    pub async fn ensure_tool(
        &self,
        tool: &str,
        version: &str,
        cache: &ToolCache,
    ) -> Result<ToolInfo, ToolError> {
        let provider = self
            .get(tool)
            .ok_or_else(|| ToolError::NotFound(tool.to_string()))?;

        let install_path = if cache.is_cached(tool, version) {
            debug!(tool, version, "cache hit — using cached install");
            cache.touch(tool, version)?;
            Some(cache.version_dir(tool, version).join("bin"))
        } else {
            debug!(tool, version, "cache miss — installing");
            let cache_dir = cache.version_dir(tool, version);
            std::fs::create_dir_all(&cache_dir)?;
            let result = provider.install_to_cache(version, &cache_dir).await?;
            cache.touch(tool, version)?;
            Some(result.install_path)
        };

        Ok(ToolInfo {
            name: tool.to_string(),
            current_version: Some(version.to_string()),
            requested_version: Some(version.to_string()),
            install_path,
            is_satisfied: true,
        })
    }

    fn register_builtins(&mut self) {
        // JavaScript runtimes & package managers
        self.register(BunProvider::new());
        self.register(NodeProvider::new());
        self.register(NpmProvider::new());

        // Language runtimes
        self.register(PythonProvider::new());
        self.register(PipProvider::new());
        self.register(GoProvider::new());
        self.register(JavaProvider::new());
        self.register(GradleProvider::new());
        self.register(FlutterProvider::new());
        self.register(DartProvider::new());

        // System/detect-only tools
        use crate::providers::system;
        self.register(system::git());
        self.register(system::docker());
        self.register(system::rustc());
        self.register(system::cargo());
        self.register(system::xcodebuild());
        self.register(system::xcrun());
        self.register(system::gpg());
        self.register(system::adb());
        self.register(system::pod());
        self.register(system::npx());
        self.register(system::yarn());
        self.register(system::eas());

        // TypeScript ecosystem
        self.register(system::tsc());
        self.register(system::turbo());
        self.register(system::esbuild());

        // iOS / Swift ecosystem
        self.register(system::swift());
        self.register(system::xcode_select());

        // Expo / React Native ecosystem
        self.register(system::expo());

        // Android SDK & JDK tools
        self.register(system::keytool());
        self.register(system::sdkmanager());
        self.register(system::apksigner());
        self.register(system::zipalign());
        self.register(system::emulator());
        self.register(system::aapt2());
        self.register(system::avdmanager());
        self.register(system::bundletool());
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use async_trait::async_trait;
    use tempfile::TempDir;

    use super::*;
    use crate::traits::InstallResult;
    use canaveral_core::config::ToolsCacheConfig;

    /// A minimal provider that records calls and installs a marker file.
    struct FakeTool {
        id: &'static str,
    }

    #[async_trait]
    impl ToolProvider for FakeTool {
        fn id(&self) -> &'static str {
            self.id
        }
        fn name(&self) -> &'static str {
            "Fake"
        }
        fn binary_name(&self) -> &'static str {
            self.id
        }
        async fn detect_version(&self) -> Result<Option<String>, ToolError> {
            Ok(None)
        }
        async fn is_satisfied(&self, _requested: &str) -> Result<bool, ToolError> {
            Ok(false)
        }
        async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
            Ok(InstallResult {
                tool: self.id.to_string(),
                version: version.to_string(),
                install_path: std::path::PathBuf::from("/fake/bin"),
            })
        }
        async fn install_to_cache(
            &self,
            version: &str,
            cache_dir: &Path,
        ) -> Result<InstallResult, ToolError> {
            // Write a marker so tests can verify the dir was used
            std::fs::create_dir_all(cache_dir).ok();
            std::fs::write(cache_dir.join(".installed"), version).ok();
            Ok(InstallResult {
                tool: self.id.to_string(),
                version: version.to_string(),
                install_path: cache_dir.join("bin"),
            })
        }
        async fn list_available(&self) -> Result<Vec<String>, ToolError> {
            Ok(vec![])
        }
        fn env_vars(&self, _install_path: &Path) -> Vec<(String, String)> {
            vec![]
        }
    }

    fn make_cache(tmp: &TempDir) -> ToolCache {
        let config = ToolsCacheConfig {
            dir: tmp.path().join("tools"),
            max_age_days: 30,
            max_size: None,
        };
        ToolCache::new(&config)
    }

    #[test]
    fn test_empty_registry() {
        let registry = ToolRegistry::empty();
        assert!(registry.all().is_empty());
    }

    #[test]
    fn test_with_builtins() {
        let registry = ToolRegistry::with_builtins();
        assert!(registry.get("bun").is_some());
        assert!(registry.get("node").is_some());
        assert!(registry.get("npm").is_some());
    }

    #[test]
    fn test_get_unknown_tool() {
        let registry = ToolRegistry::empty();
        assert!(registry.get("unknown-tool").is_none());
    }

    #[tokio::test]
    async fn test_check_all_unknown_tools() {
        let registry = ToolRegistry::empty();
        let config = HashMap::from([("unknown".to_string(), "1.0.0".to_string())]);
        let results = registry.check_all(&config).await;
        assert_eq!(results.len(), 1);
        assert!(!results[0].is_satisfied);
        assert!(results[0].current_version.is_none());
    }

    #[tokio::test]
    async fn test_ensure_tool_unknown_returns_error() {
        let registry = ToolRegistry::empty();
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp);
        let result = registry.ensure_tool("notreal", "1.0.0", &cache).await;
        assert!(matches!(result, Err(ToolError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_ensure_tool_installs_into_cache() {
        let mut registry = ToolRegistry::empty();
        registry.register(FakeTool { id: "faketool" });

        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp);

        let info = registry
            .ensure_tool("faketool", "2.0.0", &cache)
            .await
            .unwrap();

        assert_eq!(info.name, "faketool");
        assert_eq!(info.current_version.as_deref(), Some("2.0.0"));
        assert!(info.is_satisfied);

        // Verify install_to_cache was called with the right dir
        let marker = cache.version_dir("faketool", "2.0.0").join(".installed");
        assert!(marker.exists());
        assert_eq!(std::fs::read_to_string(&marker).unwrap(), "2.0.0");

        // Verify .last_used was written
        let last_used = cache.version_dir("faketool", "2.0.0").join(".last_used");
        assert!(last_used.exists());
    }

    #[tokio::test]
    async fn test_ensure_tool_uses_cache_on_second_call() {
        let mut registry = ToolRegistry::empty();
        registry.register(FakeTool { id: "faketool2" });

        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp);

        // First call: installs
        registry
            .ensure_tool("faketool2", "3.0.0", &cache)
            .await
            .unwrap();

        // Remove the marker to detect if install_to_cache is called again
        let marker = cache.version_dir("faketool2", "3.0.0").join(".installed");
        std::fs::remove_file(&marker).unwrap();

        // Second call: should use cache (is_cached returns true since dir exists)
        let info = registry
            .ensure_tool("faketool2", "3.0.0", &cache)
            .await
            .unwrap();
        assert!(info.is_satisfied);
        // Marker should NOT have been recreated — install_to_cache not called
        assert!(!marker.exists());
    }
}
