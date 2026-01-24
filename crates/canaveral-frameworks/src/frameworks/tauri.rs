//! Tauri framework adapter
//!
//! Supports building Tauri desktop apps for macOS, Windows, and Linux.

use std::path::Path;

use async_trait::async_trait;

use crate::artifacts::Artifact;
use crate::capabilities::Capabilities;
use crate::context::BuildContext;
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::Result;
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Tauri build adapter
pub struct TauriAdapter;

impl TauriAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TauriAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for TauriAdapter {
    fn id(&self) -> &'static str {
        "tauri"
    }

    fn name(&self) -> &'static str {
        "Tauri"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Check for tauri.conf.json (v1) or tauri.conf.json in src-tauri (v2)
        let has_tauri_conf = file_exists(path, "tauri.conf.json")
            || file_exists(path, "src-tauri/tauri.conf.json");

        if has_tauri_conf {
            return Detection::Yes(95);
        }

        // Check for Cargo.toml with tauri dependency
        let cargo_toml = path.join("src-tauri/Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("tauri") {
                    return Detection::Yes(90);
                }
            }
        }

        // Check for @tauri-apps/cli in package.json
        if has_npm_dependency(path, "@tauri-apps/cli") {
            return Detection::Maybe(70);
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::tauri()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::MacOs, Platform::Windows, Platform::Linux]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check for cargo
        match which::which("cargo") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("cargo", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "cargo",
                    "Install Rust from https://rustup.rs",
                ));
            }
        }

        // Check for tauri-cli
        match which::which("cargo-tauri") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("cargo-tauri", None));
            }
            Err(_) => {
                // Also check npm-based CLI
                if which::which("tauri").is_ok() {
                    status = status.with_tool(ToolStatus::found("tauri", Some("npm".to_string())));
                } else {
                    status = status.with_tool(ToolStatus::missing(
                        "tauri-cli",
                        "Install with: cargo install tauri-cli",
                    ));
                }
            }
        }

        Ok(status)
    }

    async fn build(&self, _ctx: &BuildContext) -> Result<Vec<Artifact>> {
        todo!("Tauri build not yet implemented")
    }

    async fn clean(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    fn get_version(&self, _path: &Path) -> Result<VersionInfo> {
        todo!("Tauri version parsing not yet implemented")
    }

    fn set_version(&self, _path: &Path, _version: &VersionInfo) -> Result<()> {
        todo!("Tauri version setting not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_tauri_detection() {
        let adapter = TauriAdapter::new();
        let temp = TempDir::new().unwrap();

        // No detection without tauri files
        assert!(!adapter.detect(temp.path()).detected());

        // Create Tauri project structure
        std::fs::create_dir_all(temp.path().join("src-tauri")).unwrap();
        std::fs::write(
            temp.path().join("src-tauri/tauri.conf.json"),
            r#"{"build": {}}"#,
        )
        .unwrap();

        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }
}
