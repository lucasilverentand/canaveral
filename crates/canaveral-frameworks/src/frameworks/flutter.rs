//! Flutter framework adapter
//!
//! Supports building Flutter apps for iOS, Android, macOS, Windows, Linux, and Web.

use std::path::Path;
use std::process::Command;

use async_trait::async_trait;

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
#[cfg(test)]
use crate::capabilities::Capability;
use crate::capabilities::Capabilities;
use crate::context::BuildContext;
use crate::detection::{file_exists, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{
    BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo,
};

/// Flutter build adapter
pub struct FlutterAdapter {
    /// Path to flutter executable (auto-detected if None)
    flutter_path: Option<String>,
}

impl FlutterAdapter {
    pub fn new() -> Self {
        Self { flutter_path: None }
    }

    pub fn with_flutter_path(path: impl Into<String>) -> Self {
        Self {
            flutter_path: Some(path.into()),
        }
    }

    fn flutter_cmd(&self) -> String {
        self.flutter_path
            .clone()
            .unwrap_or_else(|| "flutter".to_string())
    }

    fn run_flutter(&self, args: &[&str], path: &Path) -> Result<std::process::Output> {
        let output = Command::new(self.flutter_cmd())
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("flutter {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        Ok(output)
    }

    fn parse_pubspec_version(&self, path: &Path) -> Result<VersionInfo> {
        let pubspec_path = path.join("pubspec.yaml");
        let content = std::fs::read_to_string(&pubspec_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading pubspec.yaml".to_string(),
                message: e.to_string(),
            }
        })?;

        // Simple parsing - could use yaml parser for robustness
        let mut version = None;
        let mut build_number = None;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("version:") {
                let value = line.strip_prefix("version:").unwrap().trim();
                // Handle version+build format (e.g., "1.2.3+42")
                if let Some((ver, build)) = value.split_once('+') {
                    version = Some(ver.trim().to_string());
                    build_number = build.trim().parse().ok();
                } else {
                    version = Some(value.to_string());
                }
                break;
            }
        }

        let version = version.ok_or_else(|| FrameworkError::VersionParseError {
            message: "No version field found in pubspec.yaml".to_string(),
        })?;

        Ok(VersionInfo {
            version,
            build_number,
            ..Default::default()
        })
    }
}

impl Default for FlutterAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for FlutterAdapter {
    fn id(&self) -> &'static str {
        "flutter"
    }

    fn name(&self) -> &'static str {
        "Flutter"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Must have pubspec.yaml
        if !file_exists(path, "pubspec.yaml") {
            return Detection::No;
        }

        // Check for flutter SDK dependency
        let pubspec = path.join("pubspec.yaml");
        if let Ok(content) = std::fs::read_to_string(pubspec) {
            // Check for flutter SDK
            if content.contains("sdk: flutter") || content.contains("flutter:") {
                // Check for common flutter files
                let has_lib = path.join("lib").is_dir();
                let has_main = path.join("lib/main.dart").exists();

                if has_main {
                    return Detection::Yes(95);
                } else if has_lib {
                    return Detection::Yes(85);
                } else {
                    return Detection::Maybe(70);
                }
            }
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::flutter()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[
            Platform::Ios,
            Platform::Android,
            Platform::MacOs,
            Platform::Windows,
            Platform::Linux,
            Platform::Web,
        ]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check flutter
        match which::which("flutter") {
            Ok(_) => {
                // Get flutter version
                let version = Command::new("flutter")
                    .args(["--version", "--machine"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                        } else {
                            None
                        }
                    })
                    .and_then(|s| {
                        // Parse JSON output for version
                        serde_json::from_str::<serde_json::Value>(&s)
                            .ok()
                            .and_then(|v| v["frameworkVersion"].as_str().map(|s| s.to_string()))
                    });

                status = status.with_tool(ToolStatus::found("flutter", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "flutter",
                    "Install from https://flutter.dev/docs/get-started/install",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let mut args = vec!["build"];

        // Platform-specific subcommand
        let subcommand = match ctx.platform {
            Platform::Ios => "ipa",
            Platform::Android => "appbundle", // AAB for Play Store
            Platform::MacOs => "macos",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
            Platform::Web => "web",
        };
        args.push(subcommand);

        // Profile
        match ctx.profile {
            crate::context::BuildProfile::Debug => args.push("--debug"),
            crate::context::BuildProfile::Release => args.push("--release"),
            crate::context::BuildProfile::Profile => args.push("--profile"),
        }

        // Build number
        if let Some(bn) = ctx.build_number {
            args.push("--build-number");
            let bn_str = bn.to_string();
            args.push(Box::leak(bn_str.into_boxed_str())); // FIXME: proper lifetime
        }

        // Version
        if let Some(ref version) = ctx.version {
            args.push("--build-name");
            args.push(Box::leak(version.clone().into_boxed_str())); // FIXME: proper lifetime
        }

        // Flavor
        if let Some(ref flavor) = ctx.flavor {
            args.push("--flavor");
            args.push(Box::leak(flavor.clone().into_boxed_str())); // FIXME: proper lifetime
        }

        // Framework-specific config (dart defines)
        for (key, value) in &ctx.config {
            if let Some(v) = value.as_str() {
                args.push("--dart-define");
                let define = format!("{}={}", key, v);
                args.push(Box::leak(define.into_boxed_str())); // FIXME: proper lifetime
            }
        }

        // Execute build
        let args_str: Vec<&str> = args.iter().map(|s| *s).collect();
        let output = self.run_flutter(&args_str, &ctx.path)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: ctx.platform.as_str().to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        // Find artifacts
        let artifacts = self.find_artifacts(&ctx.path, ctx.platform, ctx)?;

        Ok(artifacts)
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        let output = self.run_flutter(&["clean"], path)?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "flutter clean".to_string(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        self.parse_pubspec_version(path)
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let pubspec_path = path.join("pubspec.yaml");
        let content = std::fs::read_to_string(&pubspec_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading pubspec.yaml".to_string(),
                message: e.to_string(),
            }
        })?;

        // Build version string
        let version_str = if let Some(bn) = version.build_number {
            format!("{}+{}", version.version, bn)
        } else {
            version.version.clone()
        };

        // Replace version line
        let mut new_content = String::new();
        let mut found = false;

        for line in content.lines() {
            if line.trim_start().starts_with("version:") && !found {
                let indent = line.len() - line.trim_start().len();
                new_content.push_str(&" ".repeat(indent));
                new_content.push_str(&format!("version: {}\n", version_str));
                found = true;
            } else {
                new_content.push_str(line);
                new_content.push('\n');
            }
        }

        if !found {
            return Err(FrameworkError::VersionParseError {
                message: "Could not find version field in pubspec.yaml".to_string(),
            });
        }

        std::fs::write(&pubspec_path, new_content).map_err(|e| FrameworkError::Context {
            context: "writing pubspec.yaml".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

impl FlutterAdapter {
    fn find_artifacts(
        &self,
        path: &Path,
        platform: Platform,
        ctx: &BuildContext,
    ) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        match platform {
            Platform::Ios => {
                // IPA is in build/ios/ipa/
                let ipa_dir = path.join("build/ios/ipa");
                if let Ok(entries) = std::fs::read_dir(&ipa_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "ipa").unwrap_or(false) {
                            let mut artifact = Artifact::new(
                                entry.path(),
                                ArtifactKind::Ipa,
                                Platform::Ios,
                            );
                            artifact.metadata = ArtifactMetadata::new()
                                .with_framework("flutter")
                                .with_signed(true); // Flutter builds signed IPA

                            if let Some(ref v) = ctx.version {
                                artifact.metadata = artifact.metadata.with_version(v);
                            }

                            artifacts.push(artifact);
                        }
                    }
                }
            }
            Platform::Android => {
                // AAB is in build/app/outputs/bundle/release/
                let bundle_dir = path.join("build/app/outputs/bundle/release");
                if let Ok(entries) = std::fs::read_dir(&bundle_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "aab").unwrap_or(false) {
                            let mut artifact = Artifact::new(
                                entry.path(),
                                ArtifactKind::Aab,
                                Platform::Android,
                            );
                            artifact.metadata = ArtifactMetadata::new()
                                .with_framework("flutter");

                            artifacts.push(artifact);
                        }
                    }
                }

                // Also check for APK
                let apk_dir = path.join("build/app/outputs/flutter-apk");
                if let Ok(entries) = std::fs::read_dir(&apk_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "apk").unwrap_or(false) {
                            let artifact = Artifact::new(
                                entry.path(),
                                ArtifactKind::Apk,
                                Platform::Android,
                            );
                            artifacts.push(artifact);
                        }
                    }
                }
            }
            Platform::MacOs => {
                let app_path = path.join("build/macos/Build/Products/Release");
                if let Ok(entries) = std::fs::read_dir(&app_path) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "app").unwrap_or(false) {
                            let artifact = Artifact::new(
                                entry.path(),
                                ArtifactKind::MacApp,
                                Platform::MacOs,
                            );
                            artifacts.push(artifact);
                        }
                    }
                }
            }
            Platform::Windows => {
                let exe_path = path.join("build/windows/x64/runner/Release");
                if let Ok(entries) = std::fs::read_dir(&exe_path) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "exe").unwrap_or(false) {
                            let artifact = Artifact::new(
                                entry.path(),
                                ArtifactKind::Exe,
                                Platform::Windows,
                            );
                            artifacts.push(artifact);
                        }
                    }
                }
            }
            Platform::Linux => {
                let bundle_path = path.join("build/linux/x64/release/bundle");
                if bundle_path.exists() {
                    // Linux produces a bundle directory, not a single file
                    let artifact = Artifact::new(
                        bundle_path,
                        ArtifactKind::Other,
                        Platform::Linux,
                    );
                    artifacts.push(artifact);
                }
            }
            Platform::Web => {
                let web_path = path.join("build/web");
                if web_path.exists() {
                    let artifact = Artifact::new(
                        web_path,
                        ArtifactKind::WebBuild,
                        Platform::Web,
                    );
                    artifacts.push(artifact);
                }
            }
        }

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: path.join(format!("build/{}", platform.as_str())),
            });
        }

        Ok(artifacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_flutter_project(temp: &TempDir) {
        std::fs::write(
            temp.path().join("pubspec.yaml"),
            r#"
name: test_app
description: A test Flutter application.
version: 1.2.3+42

environment:
  sdk: '>=3.0.0 <4.0.0'

dependencies:
  flutter:
    sdk: flutter
"#,
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("lib")).unwrap();
        std::fs::write(
            temp.path().join("lib/main.dart"),
            "void main() {}",
        )
        .unwrap();
    }

    #[test]
    fn test_flutter_detection() {
        let adapter = FlutterAdapter::new();
        let temp = TempDir::new().unwrap();

        // Empty directory - no detection
        assert!(!adapter.detect(temp.path()).detected());

        // Create Flutter project
        create_flutter_project(&temp);

        // Should detect with high confidence
        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }

    #[test]
    fn test_flutter_version_parsing() {
        let adapter = FlutterAdapter::new();
        let temp = TempDir::new().unwrap();
        create_flutter_project(&temp);

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version.version, "1.2.3");
        assert_eq!(version.build_number, Some(42));
    }

    #[test]
    fn test_flutter_version_writing() {
        let adapter = FlutterAdapter::new();
        let temp = TempDir::new().unwrap();
        create_flutter_project(&temp);

        let new_version = VersionInfo::new("2.0.0").with_build_number(100);
        adapter.set_version(temp.path(), &new_version).unwrap();

        let read_version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(read_version.version, "2.0.0");
        assert_eq!(read_version.build_number, Some(100));
    }

    #[test]
    fn test_flutter_capabilities() {
        let adapter = FlutterAdapter::new();
        let caps = adapter.capabilities();

        assert!(caps.has(Capability::BuildIos));
        assert!(caps.has(Capability::BuildAndroid));
        assert!(caps.has(Capability::BuildWeb));
        assert!(caps.has(Capability::HotReload));
        assert!(caps.can_build_mobile());
        assert!(caps.can_build_desktop());
    }
}
