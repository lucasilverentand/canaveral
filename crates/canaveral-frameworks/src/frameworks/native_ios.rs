//! Native iOS (Swift/Objective-C) framework adapter
//!
//! Supports building native iOS apps using xcodebuild.

use std::path::Path;

use async_trait::async_trait;

use crate::artifacts::Artifact;
use crate::capabilities::Capabilities;
use crate::context::BuildContext;
use crate::detection::{file_exists, Detection};
use crate::error::Result;
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Native iOS build adapter
pub struct NativeIosAdapter;

impl NativeIosAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeIosAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for NativeIosAdapter {
    fn id(&self) -> &'static str {
        "native-ios"
    }

    fn name(&self) -> &'static str {
        "Native iOS (Xcode)"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Look for .xcodeproj or .xcworkspace
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.ends_with(".xcworkspace") {
                    // Workspace has higher priority (could be CocoaPods/SPM)
                    return Detection::Yes(85);
                }
                if name_str.ends_with(".xcodeproj") {
                    return Detection::Yes(80);
                }
            }
        }

        // Check for Package.swift (Swift Package)
        if file_exists(path, "Package.swift") {
            return Detection::Maybe(50);
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::native_ios()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Ios]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check for xcodebuild
        match which::which("xcodebuild") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("xcodebuild", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "xcodebuild",
                    "Install Xcode from the App Store",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, _ctx: &BuildContext) -> Result<Vec<Artifact>> {
        todo!("Native iOS build not yet implemented")
    }

    async fn clean(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    fn get_version(&self, _path: &Path) -> Result<VersionInfo> {
        todo!("Native iOS version parsing not yet implemented")
    }

    fn set_version(&self, _path: &Path, _version: &VersionInfo) -> Result<()> {
        todo!("Native iOS version setting not yet implemented")
    }
}
