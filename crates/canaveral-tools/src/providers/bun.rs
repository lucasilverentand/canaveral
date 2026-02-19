//! Bun tool provider
//!
//! Detects and installs Bun (<https://bun.sh>).

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

/// Provider for the Bun JavaScript runtime and package manager
pub struct BunProvider;

impl BunProvider {
    pub fn new() -> Self {
        Self
    }

    /// Default installation directory: ~/.bun
    fn default_install_dir() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".bun"))
    }

    /// Run the official Bun install script with `BUN_INSTALL` set to `install_dir`.
    async fn run_install_script(
        &self,
        version: &str,
        install_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let tag = format!("bun-v{}", version.trim_start_matches('v'));
        debug!(version = %version, tag = %tag, dir = %install_dir.display(), "running bun install script");

        let status = tokio::process::Command::new("sh")
            .args([
                "-c",
                &format!("curl -fsSL https://bun.sh/install | bash -s \"{tag}\""),
            ])
            .env("BUN_INSTALL", install_dir)
            .status()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "bun".into(),
                version: version.into(),
                reason: format!("failed to spawn install script: {e}"),
            })?;

        if !status.success() {
            return Err(ToolError::InstallFailed {
                tool: "bun".into(),
                version: version.into(),
                reason: format!("install script exited with status {status}"),
            });
        }

        Ok(InstallResult {
            tool: "bun".into(),
            version: version.trim_start_matches('v').to_string(),
            install_path: install_dir.join("bin"),
        })
    }
}

impl Default for BunProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for BunProvider {
    fn id(&self) -> &'static str {
        "bun"
    }

    fn name(&self) -> &'static str {
        "Bun"
    }

    fn binary_name(&self) -> &'static str {
        "bun"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        // First check if bun is on PATH
        if which::which("bun").is_err() {
            debug!("bun not found on PATH");
            return Ok(None);
        }

        let output = tokio::process::Command::new("bun")
            .arg("--version")
            .output()
            .await
            .map_err(|e| ToolError::DetectionFailed(format!("failed to run bun --version: {e}")))?;

        if !output.status.success() {
            warn!("bun --version exited with non-zero status");
            return Ok(None);
        }

        let version = String::from_utf8_lossy(&output.stdout)
            .trim()
            .trim_start_matches('v')
            .to_string();

        debug!(version = %version, "detected bun version");
        Ok(Some(version))
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        let install_dir =
            Self::default_install_dir().unwrap_or_else(|| PathBuf::from("/usr/local"));
        self.run_install_script(version, &install_dir).await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        // cache_dir is e.g. ~/.canaveral/tools/bun/1.2.3/
        // Bun's install script respects BUN_INSTALL to set the prefix.
        debug!(
            version = %version,
            cache_dir = %cache_dir.display(),
            "installing bun into cache"
        );
        self.run_install_script(version, cache_dir).await
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        // Bun doesn't expose a simple version list API.
        // Return an empty list; users should consult https://github.com/oven-sh/bun/releases.
        Ok(Vec::new())
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        // install_path is the `bin/` directory produced by install_result.install_path.
        // Prepend it to PATH so `bun` is found before any system-installed version.
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            install_path.display().to_string()
        } else {
            format!("{}:{path}", install_path.display())
        };
        vec![
            (
                "BUN_INSTALL".into(),
                install_path
                    .parent()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default(),
            ),
            ("PATH".into(), new_path),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_id_and_name() {
        let p = BunProvider::new();
        assert_eq!(p.id(), "bun");
        assert_eq!(p.name(), "Bun");
        assert_eq!(p.binary_name(), "bun");
    }

    #[test]
    fn env_vars_contains_bun_install_and_path() {
        let p = BunProvider::new();
        let bin_dir = Path::new("/home/user/.canaveral/tools/bun/1.2.3/bin");
        let vars = p.env_vars(bin_dir);
        // BUN_INSTALL should point to the parent of bin/
        let bun_install = vars
            .iter()
            .find(|(k, _)| k == "BUN_INSTALL")
            .map(|(_, v)| v.as_str());
        assert_eq!(bun_install, Some("/home/user/.canaveral/tools/bun/1.2.3"));
        // PATH should contain the bin dir
        let path_val = vars
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        assert!(path_val.contains("/home/user/.canaveral/tools/bun/1.2.3/bin"));
    }

    #[test]
    fn env_vars_path_prepends_bin_dir() {
        let p = BunProvider::new();
        let bin_dir = Path::new("/cache/bun/1.0.0/bin");
        let vars = p.env_vars(bin_dir);
        let path_val = vars
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        // bin_dir should be first segment
        assert!(path_val.starts_with("/cache/bun/1.0.0/bin"));
    }

    // Version satisfaction tests delegate to version_match, but we verify the
    // integration path through the provider struct works at compile time.
    #[test]
    fn version_satisfies_integration() {
        assert!(version_satisfies("1.2.3", "1.2"));
        assert!(!version_satisfies("1.3.0", "1.2"));
    }
}
