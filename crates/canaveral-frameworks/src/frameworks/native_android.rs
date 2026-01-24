//! Native Android (Kotlin/Java) framework adapter
//!
//! Supports building native Android apps using Gradle.

use std::path::Path;

use async_trait::async_trait;

use crate::artifacts::Artifact;
use crate::capabilities::{Capabilities, Capability};
use crate::context::BuildContext;
use crate::detection::{file_exists, Detection};
use crate::error::Result;
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Native Android build adapter
pub struct NativeAndroidAdapter;

impl NativeAndroidAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NativeAndroidAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for NativeAndroidAdapter {
    fn id(&self) -> &'static str {
        "native-android"
    }

    fn name(&self) -> &'static str {
        "Native Android (Gradle)"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Look for build.gradle or build.gradle.kts
        let has_gradle = file_exists(path, "build.gradle")
            || file_exists(path, "build.gradle.kts");

        if !has_gradle {
            return Detection::No;
        }

        // Check for Android-specific files
        let has_settings = file_exists(path, "settings.gradle")
            || file_exists(path, "settings.gradle.kts");
        let has_app = path.join("app").is_dir();

        // Check for AndroidManifest.xml
        let has_manifest = path.join("app/src/main/AndroidManifest.xml").exists();

        if has_manifest {
            return Detection::Yes(90);
        }

        if has_settings && has_app {
            return Detection::Yes(80);
        }

        Detection::Maybe(50)
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::new()
            .with(Capability::BuildAndroid)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::BuildFlavors)
            .with(Capability::CodeSigning)
            .with(Capability::UnitTests)
            .with(Capability::IntegrationTests)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::BuildNumbers)
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Android]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check for ANDROID_HOME
        if std::env::var("ANDROID_HOME").is_err() && std::env::var("ANDROID_SDK_ROOT").is_err() {
            status = status.with_warning("ANDROID_HOME or ANDROID_SDK_ROOT not set");
        }

        // Check for gradle wrapper or global gradle
        let gradle_wrapper = Path::new("gradlew").exists();
        match which::which("gradle") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found(
                    "gradle",
                    if gradle_wrapper {
                        Some("wrapper".to_string())
                    } else {
                        None
                    },
                ));
            }
            Err(_) if !gradle_wrapper => {
                status = status.with_tool(ToolStatus::missing(
                    "gradle",
                    "Install Gradle or use gradle wrapper (./gradlew)",
                ));
            }
            _ => {
                status = status.with_tool(ToolStatus::found("gradlew", Some("wrapper".to_string())));
            }
        }

        Ok(status)
    }

    async fn build(&self, _ctx: &BuildContext) -> Result<Vec<Artifact>> {
        todo!("Native Android build not yet implemented")
    }

    async fn clean(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    fn get_version(&self, _path: &Path) -> Result<VersionInfo> {
        todo!("Native Android version parsing not yet implemented")
    }

    fn set_version(&self, _path: &Path, _version: &VersionInfo) -> Result<()> {
        todo!("Native Android version setting not yet implemented")
    }
}
