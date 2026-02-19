//! Node.js and npm tool providers

use std::path::Path;

use async_trait::async_trait;

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

/// Provider for Node.js
pub struct NodeProvider;

/// Provider for npm (bundled with Node.js)
pub struct NpmProvider;

async fn run_version_command(binary: &str) -> Result<Option<String>, ToolError> {
    let output = tokio::process::Command::new(binary)
        .arg("--version")
        .output()
        .await;

    match output {
        Ok(out) if out.status.success() => {
            let raw = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Ok(Some(raw))
        }
        Ok(_) => Ok(None),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ToolError::DetectionFailed(format!(
            "failed to run `{binary} --version`: {e}"
        ))),
    }
}

#[async_trait]
impl ToolProvider for NodeProvider {
    fn id(&self) -> &'static str {
        "node"
    }

    fn name(&self) -> &'static str {
        "Node.js"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let version = run_version_command("node").await?;
        // node --version outputs "v22.1.0" — strip the leading 'v'
        Ok(version.map(|v| v.trim_start_matches('v').to_string()))
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        Err(ToolError::InstallFailed {
            tool: "node".to_string(),
            version: version.to_string(),
            reason: "Node.js cannot be auto-installed by canaveral. \
                     Install it with a version manager and canaveral will detect it:\n  \
                     - fnm (recommended): https://github.com/Schniz/fnm\n  \
                     - nvm: https://github.com/nvm-sh/nvm\n  \
                     - volta: https://volta.sh\n  \
                     - mise: https://mise.jdx.dev"
                .to_string(),
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        Err(ToolError::InstallFailed {
            tool: "node".to_string(),
            version: String::new(),
            reason: "Use nvm, fnm, volta, or mise to list and manage Node.js versions".to_string(),
        })
    }

    fn binary_name(&self) -> &'static str {
        "node"
    }

    fn env_vars(&self, _install_path: &Path) -> Vec<(String, String)> {
        Vec::new()
    }
}

#[async_trait]
impl ToolProvider for NpmProvider {
    fn id(&self) -> &'static str {
        "npm"
    }

    fn name(&self) -> &'static str {
        "npm"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        // npm --version outputs "10.2.0" — no 'v' prefix to strip
        run_version_command("npm").await
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        Err(ToolError::InstallFailed {
            tool: "npm".to_string(),
            version: version.to_string(),
            reason: "npm is bundled with Node.js. Install or update Node.js using one of:\n  \
                     - nvm: https://github.com/nvm-sh/nvm\n  \
                     - fnm: https://github.com/Schniz/fnm\n  \
                     - volta: https://volta.sh\n  \
                     - mise: https://mise.jdx.dev"
                .to_string(),
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        Err(ToolError::InstallFailed {
            tool: "npm".to_string(),
            version: String::new(),
            reason: "npm versions are tied to Node.js. Use nvm, fnm, volta, or mise to manage Node.js versions".to_string(),
        })
    }

    fn binary_name(&self) -> &'static str {
        "npm"
    }

    fn env_vars(&self, _install_path: &Path) -> Vec<(String, String)> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_provider_id_and_name() {
        let provider = NodeProvider;
        assert_eq!(provider.id(), "node");
        assert_eq!(provider.name(), "Node.js");
        assert_eq!(provider.binary_name(), "node");
    }

    #[test]
    fn npm_provider_id_and_name() {
        let provider = NpmProvider;
        assert_eq!(provider.id(), "npm");
        assert_eq!(provider.name(), "npm");
        assert_eq!(provider.binary_name(), "npm");
    }

    #[test]
    fn node_strips_v_prefix() {
        // Simulate the stripping behavior used in detect_version
        let raw = "v22.1.0";
        let stripped = raw.trim_start_matches('v');
        assert_eq!(stripped, "22.1.0");
    }

    #[test]
    fn npm_no_v_prefix_needed() {
        // npm outputs without 'v', so no stripping needed
        let raw = "10.2.0";
        assert_eq!(raw, "10.2.0");
    }

    #[test]
    fn node_version_matching() {
        assert!(version_satisfies("22.1.0", "22"));
        assert!(version_satisfies("22.1.0", "22.1"));
        assert!(version_satisfies("22.1.0", "22.1.0"));
        assert!(!version_satisfies("22.1.0", "20"));
        assert!(!version_satisfies("22.1.0", "22.2"));
    }

    #[test]
    fn npm_version_matching() {
        assert!(version_satisfies("10.2.0", "10"));
        assert!(version_satisfies("10.2.0", "10.2"));
        assert!(version_satisfies("10.2.0", "10.2.0"));
        assert!(!version_satisfies("10.2.0", "9"));
    }

    #[tokio::test]
    async fn node_install_returns_helpful_error() {
        let provider = NodeProvider;
        let result = provider.install("22").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("nvm") || err.contains("fnm") || err.contains("volta"));
    }

    #[tokio::test]
    async fn npm_install_returns_helpful_error() {
        let provider = NpmProvider;
        let result = provider.install("10").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Node.js"));
    }

    #[test]
    fn env_vars_is_empty() {
        let node = NodeProvider;
        let npm = NpmProvider;
        assert!(node.env_vars(Path::new("/usr/local/bin")).is_empty());
        assert!(npm.env_vars(Path::new("/usr/local/bin")).is_empty());
    }
}
