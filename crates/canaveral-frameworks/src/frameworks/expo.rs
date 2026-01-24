//! Expo (React Native) framework adapter
//!
//! Supports building Expo and bare React Native apps using EAS Build or local builds.

use std::path::Path;

use async_trait::async_trait;

use crate::artifacts::Artifact;
use crate::capabilities::Capabilities;
use crate::context::BuildContext;
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::Result;
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Expo build adapter
pub struct ExpoAdapter;

impl ExpoAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExpoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for ExpoAdapter {
    fn id(&self) -> &'static str {
        "expo"
    }

    fn name(&self) -> &'static str {
        "Expo (React Native)"
    }

    fn detect(&self, path: &Path) -> Detection {
        if !file_exists(path, "package.json") {
            return Detection::No;
        }

        // Check for expo dependency
        if has_npm_dependency(path, "expo") {
            // Check for app.json or app.config.js
            if file_exists(path, "app.json") || file_exists(path, "app.config.js") {
                return Detection::Yes(90);
            }
            return Detection::Maybe(70);
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::expo()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Ios, Platform::Android, Platform::Web]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check for eas-cli
        match which::which("eas") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("eas-cli", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "eas-cli",
                    "Install with: npm install -g eas-cli",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, _ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // TODO: Implement EAS build or local build
        todo!("Expo build not yet implemented")
    }

    async fn clean(&self, _path: &Path) -> Result<()> {
        // TODO: Implement clean
        Ok(())
    }

    fn get_version(&self, _path: &Path) -> Result<VersionInfo> {
        // TODO: Parse app.json or app.config.js
        todo!("Expo version parsing not yet implemented")
    }

    fn set_version(&self, _path: &Path, _version: &VersionInfo) -> Result<()> {
        // TODO: Update app.json or app.config.js
        todo!("Expo version setting not yet implemented")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_expo_detection() {
        let adapter = ExpoAdapter::new();
        let temp = TempDir::new().unwrap();

        // No detection without package.json
        assert!(!adapter.detect(temp.path()).detected());

        // Create Expo project
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "dependencies": {"expo": "^49.0.0"}}"#,
        )
        .unwrap();
        std::fs::write(temp.path().join("app.json"), r#"{"expo": {}}"#).unwrap();

        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }
}
