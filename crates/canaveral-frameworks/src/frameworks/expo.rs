//! Expo (React Native) framework adapter
//!
//! Supports building Expo and bare React Native apps using EAS Build or local builds.

use std::path::Path;
use std::process::Command;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Expo build adapter
pub struct ExpoAdapter {
    /// Use EAS Build (cloud) instead of local builds
    use_eas_build: bool,
    /// EAS profile to use
    eas_profile: Option<String>,
}

impl ExpoAdapter {
    pub fn new() -> Self {
        Self {
            use_eas_build: false,
            eas_profile: None,
        }
    }

    /// Configure to use EAS Build (cloud builds)
    pub fn with_eas_build(mut self, profile: Option<String>) -> Self {
        self.use_eas_build = true;
        self.eas_profile = profile;
        self
    }

    fn run_npx(&self, args: &[&str], path: &Path) -> Result<std::process::Output> {
        let output = Command::new("npx")
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("npx {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        Ok(output)
    }

    fn run_eas(&self, args: &[&str], path: &Path) -> Result<std::process::Output> {
        let output = Command::new("eas")
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("eas {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        Ok(output)
    }

    fn parse_app_json(&self, path: &Path) -> Result<AppConfig> {
        // Try app.json first
        let app_json_path = path.join("app.json");
        if app_json_path.exists() {
            let content = std::fs::read_to_string(&app_json_path).map_err(|e| {
                FrameworkError::Context {
                    context: "reading app.json".to_string(),
                    message: e.to_string(),
                }
            })?;

            let wrapper: AppJsonWrapper = serde_json::from_str(&content).map_err(|e| {
                FrameworkError::Context {
                    context: "parsing app.json".to_string(),
                    message: e.to_string(),
                }
            })?;

            return Ok(wrapper.expo);
        }

        // Try app.config.js (limited support - just read version from package.json)
        let app_config_path = path.join("app.config.js");
        if app_config_path.exists() {
            // For dynamic configs, fall back to package.json
            return self.parse_package_json_version(path);
        }

        Err(FrameworkError::Context {
            context: "finding app config".to_string(),
            message: "Neither app.json nor app.config.js found".to_string(),
        })
    }

    fn parse_package_json_version(&self, path: &Path) -> Result<AppConfig> {
        let package_json_path = path.join("package.json");
        let content = std::fs::read_to_string(&package_json_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        let pkg: PackageJson = serde_json::from_str(&content).map_err(|e| {
            FrameworkError::Context {
                context: "parsing package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        Ok(AppConfig {
            name: pkg.name.clone(),
            slug: pkg.name,
            version: pkg.version,
            ios: None,
            android: None,
        })
    }

    fn write_app_json(&self, path: &Path, config: &AppConfig) -> Result<()> {
        let app_json_path = path.join("app.json");

        // Read existing content to preserve other fields
        let existing: serde_json::Value = if app_json_path.exists() {
            let content = std::fs::read_to_string(&app_json_path).map_err(|e| {
                FrameworkError::Context {
                    context: "reading app.json".to_string(),
                    message: e.to_string(),
                }
            })?;
            serde_json::from_str(&content).unwrap_or(serde_json::json!({"expo": {}}))
        } else {
            serde_json::json!({"expo": {}})
        };

        let mut new_config = existing.clone();
        if let Some(expo) = new_config.get_mut("expo") {
            if let Some(obj) = expo.as_object_mut() {
                obj.insert("version".to_string(), serde_json::json!(config.version));

                if let Some(ref ios) = config.ios {
                    let ios_obj = obj.entry("ios").or_insert(serde_json::json!({}));
                    if let Some(ios_map) = ios_obj.as_object_mut() {
                        if let Some(bn) = &ios.build_number {
                            ios_map.insert("buildNumber".to_string(), serde_json::json!(bn));
                        }
                    }
                }

                if let Some(ref android) = config.android {
                    let android_obj = obj.entry("android").or_insert(serde_json::json!({}));
                    if let Some(android_map) = android_obj.as_object_mut() {
                        if let Some(vc) = android.version_code {
                            android_map.insert("versionCode".to_string(), serde_json::json!(vc));
                        }
                    }
                }
            }
        }

        let content = serde_json::to_string_pretty(&new_config).map_err(|e| {
            FrameworkError::Context {
                context: "serializing app.json".to_string(),
                message: e.to_string(),
            }
        })?;

        std::fs::write(&app_json_path, content).map_err(|e| FrameworkError::Context {
            context: "writing app.json".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn build_with_eas(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let platform = match ctx.platform {
            Platform::Ios => "ios",
            Platform::Android => "android",
            _ => {
                return Err(FrameworkError::UnsupportedPlatform {
                    platform: ctx.platform.as_str().to_string(),
                    framework: "expo".to_string(),
                })
            }
        };

        let profile = self.eas_profile.as_deref().unwrap_or(match ctx.profile {
            BuildProfile::Debug => "development",
            BuildProfile::Release => "production",
            BuildProfile::Profile => "preview",
        });

        let mut args = vec!["build", "--platform", platform, "--profile", profile, "--non-interactive"];

        // Local build option
        args.push("--local");

        let output = self.run_eas(&args, &ctx.path)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: ctx.platform.as_str().to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        // Find built artifacts
        self.find_artifacts(&ctx.path, ctx.platform, ctx)
    }

    async fn build_local(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // First, run expo prebuild to generate native projects
        let prebuild_output = self.run_npx(&["expo", "prebuild", "--clean"], &ctx.path)?;

        if !prebuild_output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: ctx.platform.as_str().to_string(),
                message: format!(
                    "expo prebuild failed: {}",
                    String::from_utf8_lossy(&prebuild_output.stderr)
                ),
                source: None,
            });
        }

        match ctx.platform {
            Platform::Ios => self.build_ios_local(ctx).await,
            Platform::Android => self.build_android_local(ctx).await,
            Platform::Web => self.build_web(ctx).await,
            _ => Err(FrameworkError::UnsupportedPlatform {
                platform: ctx.platform.as_str().to_string(),
                framework: "expo".to_string(),
            }),
        }
    }

    async fn build_ios_local(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let config = match ctx.profile {
            BuildProfile::Debug => "Debug",
            BuildProfile::Release | BuildProfile::Profile => "Release",
        };

        // Use xcodebuild
        let scheme = self.detect_ios_scheme(&ctx.path)?;

        let mut args = vec![
            "-workspace",
            "ios/App.xcworkspace",
            "-scheme",
            &scheme,
            "-configuration",
            config,
            "-sdk",
            "iphoneos",
            "-archivePath",
            "build/ios/App.xcarchive",
            "archive",
        ];

        // Add code signing if release
        if matches!(ctx.profile, BuildProfile::Release) {
            args.extend_from_slice(&[
                "CODE_SIGN_STYLE=Manual",
                "-allowProvisioningUpdates",
            ]);
        }

        let output = Command::new("xcodebuild")
            .args(&args)
            .current_dir(&ctx.path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcodebuild {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "ios".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        // Export IPA
        if matches!(ctx.profile, BuildProfile::Release) {
            self.export_ipa(ctx).await?;
        }

        self.find_artifacts(&ctx.path, Platform::Ios, ctx)
    }

    async fn export_ipa(&self, ctx: &BuildContext) -> Result<()> {
        // Create export options plist
        let export_options = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store</string>
    <key>teamID</key>
    <string>${CANAVERAL_TEAM_ID}</string>
</dict>
</plist>"#;

        let options_path = ctx.path.join("build/ios/ExportOptions.plist");
        std::fs::create_dir_all(options_path.parent().unwrap()).ok();
        std::fs::write(&options_path, export_options).map_err(|e| FrameworkError::Context {
            context: "writing export options".to_string(),
            message: e.to_string(),
        })?;

        let output = Command::new("xcodebuild")
            .args([
                "-exportArchive",
                "-archivePath",
                "build/ios/App.xcarchive",
                "-exportPath",
                "build/ios/ipa",
                "-exportOptionsPlist",
                "build/ios/ExportOptions.plist",
            ])
            .current_dir(&ctx.path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "xcodebuild -exportArchive".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "ios".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        Ok(())
    }

    async fn build_android_local(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let task = match ctx.profile {
            BuildProfile::Debug => "assembleDebug",
            BuildProfile::Release => "bundleRelease",
            BuildProfile::Profile => "assembleRelease",
        };

        let gradle_wrapper = if cfg!(windows) {
            "gradlew.bat"
        } else {
            "./gradlew"
        };

        let output = Command::new(gradle_wrapper)
            .arg(task)
            .current_dir(ctx.path.join("android"))
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("{} {}", gradle_wrapper, task),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "android".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        self.find_artifacts(&ctx.path, Platform::Android, ctx)
    }

    async fn build_web(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let output = self.run_npx(&["expo", "export", "--platform", "web"], &ctx.path)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "web".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        self.find_artifacts(&ctx.path, Platform::Web, ctx)
    }

    fn detect_ios_scheme(&self, path: &Path) -> Result<String> {
        // Try to find the scheme from workspace
        let workspace = path.join("ios").join("App.xcworkspace");
        if !workspace.exists() {
            // Check for other common names
            let ios_dir = path.join("ios");
            if let Ok(entries) = std::fs::read_dir(&ios_dir) {
                for entry in entries.flatten() {
                    if entry
                        .path()
                        .extension()
                        .map(|e| e == "xcworkspace")
                        .unwrap_or(false)
                    {
                        let name = entry.path().file_stem().unwrap().to_string_lossy().to_string();
                        return Ok(name);
                    }
                }
            }
        }

        // Default to "App" which is common for Expo projects
        Ok("App".to_string())
    }

    fn find_artifacts(
        &self,
        path: &Path,
        platform: Platform,
        ctx: &BuildContext,
    ) -> Result<Vec<Artifact>> {
        let mut artifacts = Vec::new();

        match platform {
            Platform::Ios => {
                // Check for IPA
                let ipa_dir = path.join("build/ios/ipa");
                if let Ok(entries) = std::fs::read_dir(&ipa_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "ipa").unwrap_or(false) {
                            let mut artifact =
                                Artifact::new(entry.path(), ArtifactKind::Ipa, Platform::Ios);
                            artifact.metadata = ArtifactMetadata::new()
                                .with_framework("expo")
                                .with_signed(true);

                            if let Some(ref v) = ctx.version {
                                artifact.metadata = artifact.metadata.with_version(v);
                            }

                            artifacts.push(artifact);
                        }
                    }
                }

                // Check for .app (debug)
                let app_dir = path.join("ios/build/Build/Products/Debug-iphonesimulator");
                if let Ok(entries) = std::fs::read_dir(&app_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "app").unwrap_or(false) {
                            let artifact =
                                Artifact::new(entry.path(), ArtifactKind::App, Platform::Ios);
                            artifacts.push(artifact);
                        }
                    }
                }
            }
            Platform::Android => {
                // Check for AAB
                let aab_dir = path.join("android/app/build/outputs/bundle/release");
                if let Ok(entries) = std::fs::read_dir(&aab_dir) {
                    for entry in entries.flatten() {
                        if entry.path().extension().map(|e| e == "aab").unwrap_or(false) {
                            let artifact =
                                Artifact::new(entry.path(), ArtifactKind::Aab, Platform::Android);
                            artifacts.push(artifact);
                        }
                    }
                }

                // Check for APK
                for variant in ["debug", "release"] {
                    let apk_dir = path.join(format!("android/app/build/outputs/apk/{}", variant));
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
            }
            Platform::Web => {
                let web_dir = path.join("dist");
                if web_dir.exists() {
                    let artifact = Artifact::new(web_dir, ArtifactKind::WebBuild, Platform::Web);
                    artifacts.push(artifact);
                }
            }
            _ => {}
        }

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: path.join(format!("build/{}", platform.as_str())),
            });
        }

        Ok(artifacts)
    }
}

impl Default for ExpoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppJsonWrapper {
    expo: AppConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AppConfig {
    #[serde(default)]
    name: String,
    #[serde(default)]
    slug: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    ios: Option<IosConfig>,
    #[serde(default)]
    android: Option<AndroidConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IosConfig {
    #[serde(rename = "bundleIdentifier")]
    bundle_identifier: Option<String>,
    #[serde(rename = "buildNumber")]
    build_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AndroidConfig {
    #[serde(rename = "package")]
    package: Option<String>,
    #[serde(rename = "versionCode")]
    version_code: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
struct PackageJson {
    name: String,
    #[serde(default)]
    version: Option<String>,
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
        debug!(path = %path.display(), "detecting Expo project");
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

        // Check for Node.js
        match which::which("node") {
            Ok(_) => {
                let version = Command::new("node")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string());

                status = status.with_tool(ToolStatus::found("node", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "node",
                    "Install from https://nodejs.org/",
                ));
            }
        }

        // Check for npx
        match which::which("npx") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("npx", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing("npx", "Install Node.js"));
            }
        }

        // Check for eas-cli (optional but recommended)
        match which::which("eas") {
            Ok(_) => {
                let version = Command::new("eas")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string());

                status = status.with_tool(ToolStatus::found("eas-cli", version));
            }
            Err(_) => {
                // eas-cli is optional
                status = status.with_tool(ToolStatus::missing(
                    "eas-cli",
                    "(Optional) Install with: npm install -g eas-cli",
                ));
            }
        }

        Ok(status)
    }

    #[instrument(skip(self, ctx), fields(framework = "expo", platform = %ctx.platform.as_str()))]
    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        info!(
            platform = %ctx.platform.as_str(),
            use_eas = self.use_eas_build,
            "building Expo project"
        );
        if self.use_eas_build {
            self.build_with_eas(ctx).await
        } else {
            self.build_local(ctx).await
        }
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        // Clean Expo cache
        let _ = self.run_npx(&["expo", "prebuild", "--clean"], path);

        // Clean native builds
        let ios_build = path.join("ios/build");
        if ios_build.exists() {
            std::fs::remove_dir_all(&ios_build).ok();
        }

        let android_build = path.join("android/app/build");
        if android_build.exists() {
            std::fs::remove_dir_all(&android_build).ok();
        }

        // Clean dist
        let dist = path.join("dist");
        if dist.exists() {
            std::fs::remove_dir_all(&dist).ok();
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let config = self.parse_app_json(path)?;

        let version = config.version.unwrap_or_else(|| "1.0.0".to_string());

        // Get build number from platform-specific config
        let build_number: Option<u64> = config
            .ios
            .as_ref()
            .and_then(|ios| ios.build_number.as_ref())
            .and_then(|bn| bn.parse().ok())
            .or_else(|| config.android.as_ref().and_then(|a| a.version_code));

        Ok(VersionInfo {
            version,
            build_number,
            ..Default::default()
        })
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let mut config = self.parse_app_json(path)?;

        config.version = Some(version.version.clone());

        if let Some(bn) = version.build_number {
            // Update iOS build number
            let ios = config.ios.get_or_insert(IosConfig {
                bundle_identifier: None,
                build_number: None,
            });
            ios.build_number = Some(bn.to_string());

            // Update Android version code
            let android = config.android.get_or_insert(AndroidConfig {
                package: None,
                version_code: None,
            });
            android.version_code = Some(bn);
        }

        self.write_app_json(path, &config)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_expo_project(temp: &TempDir) {
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test-app", "version": "1.0.0", "dependencies": {"expo": "^49.0.0", "react": "18.2.0", "react-native": "0.72.0"}}"#,
        )
        .unwrap();

        std::fs::write(
            temp.path().join("app.json"),
            r#"{
                "expo": {
                    "name": "Test App",
                    "slug": "test-app",
                    "version": "1.2.3",
                    "ios": {
                        "bundleIdentifier": "com.example.testapp",
                        "buildNumber": "42"
                    },
                    "android": {
                        "package": "com.example.testapp",
                        "versionCode": 42
                    }
                }
            }"#,
        )
        .unwrap();
    }

    #[test]
    fn test_expo_detection() {
        let adapter = ExpoAdapter::new();
        let temp = TempDir::new().unwrap();

        // No detection without package.json
        assert!(!adapter.detect(temp.path()).detected());

        // Create Expo project
        create_expo_project(&temp);

        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }

    #[test]
    fn test_expo_version_parsing() {
        let adapter = ExpoAdapter::new();
        let temp = TempDir::new().unwrap();
        create_expo_project(&temp);

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version.version, "1.2.3");
        assert_eq!(version.build_number, Some(42));
    }

    #[test]
    fn test_expo_version_writing() {
        let adapter = ExpoAdapter::new();
        let temp = TempDir::new().unwrap();
        create_expo_project(&temp);

        let new_version = VersionInfo::new("2.0.0").with_build_number(100);
        adapter.set_version(temp.path(), &new_version).unwrap();

        let read_version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(read_version.version, "2.0.0");
        assert_eq!(read_version.build_number, Some(100));
    }

    #[test]
    fn test_expo_not_detected_for_bare_rn() {
        let adapter = ExpoAdapter::new();
        let temp = TempDir::new().unwrap();

        // Create bare React Native project (no expo dependency)
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "dependencies": {"react-native": "^0.72.0"}}"#,
        )
        .unwrap();

        assert!(!adapter.detect(temp.path()).detected());
    }
}
