//! Tool provider traits

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::error::ToolError;

/// Information about an installed tool
#[derive(Debug, Clone)]
pub struct ToolInfo {
    pub name: String,
    pub current_version: Option<String>,
    pub requested_version: Option<String>,
    pub install_path: Option<PathBuf>,
    pub is_satisfied: bool,
}

/// Result of a successful tool installation
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub tool: String,
    pub version: String,
    pub install_path: PathBuf,
}

/// Trait for tool providers (bun, node, python, etc.)
#[async_trait]
pub trait ToolProvider: Send + Sync {
    /// Unique identifier, e.g. "bun", "node"
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Detect currently installed version (None if not installed)
    async fn detect_version(&self) -> Result<Option<String>, ToolError>;

    /// Check if the requested version is satisfied by the installed version
    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError>;

    /// Install a specific version into the provider's default location.
    async fn install(&self, version: &str) -> Result<InstallResult, ToolError>;

    /// Install a specific version into `cache_dir` (the versioned cache directory).
    ///
    /// The default implementation delegates to [`install`](Self::install). Providers
    /// that support a configurable install prefix should override this to install
    /// directly into `cache_dir` so the cache is the source of truth.
    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let _ = cache_dir;
        self.install(version).await
    }

    /// List available/installable versions
    async fn list_available(&self) -> Result<Vec<String>, ToolError>;

    /// Get the binary name for this tool
    fn binary_name(&self) -> &'static str;

    /// Get environment variables to set for this tool version
    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)>;
}
