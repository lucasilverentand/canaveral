//! React Native (bare) framework adapter
//!
//! Supports building bare React Native apps (without Expo).

use std::path::Path;

use async_trait::async_trait;

use crate::artifacts::Artifact;
use crate::capabilities::Capabilities;
use crate::context::BuildContext;
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::Result;
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, VersionInfo};

/// React Native (bare) build adapter
pub struct ReactNativeAdapter;

impl ReactNativeAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReactNativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for ReactNativeAdapter {
    fn id(&self) -> &'static str {
        "react-native"
    }

    fn name(&self) -> &'static str {
        "React Native"
    }

    fn detect(&self, path: &Path) -> Detection {
        if !file_exists(path, "package.json") {
            return Detection::No;
        }

        // Check for react-native but not expo
        let has_rn = has_npm_dependency(path, "react-native");
        let has_expo = has_npm_dependency(path, "expo");

        if has_rn && !has_expo {
            // Check for native directories
            let has_ios = path.join("ios").is_dir();
            let has_android = path.join("android").is_dir();

            if has_ios && has_android {
                return Detection::Yes(90);
            } else if has_ios || has_android {
                return Detection::Yes(80);
            }
            return Detection::Maybe(60);
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::react_native()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Ios, Platform::Android]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        Ok(PrerequisiteStatus::ok())
    }

    async fn build(&self, _ctx: &BuildContext) -> Result<Vec<Artifact>> {
        todo!("React Native build not yet implemented")
    }

    async fn clean(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    fn get_version(&self, _path: &Path) -> Result<VersionInfo> {
        todo!("React Native version parsing not yet implemented")
    }

    fn set_version(&self, _path: &Path, _version: &VersionInfo) -> Result<()> {
        todo!("React Native version setting not yet implemented")
    }
}
