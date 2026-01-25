//! React Native (bare) framework adapter
//!
//! Supports building bare React Native apps (without Expo).

use std::path::Path;
use std::process::Command;

use async_trait::async_trait;
use serde::Deserialize;

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// React Native (bare) build adapter
pub struct ReactNativeAdapter {
    /// Use Hermes engine
    use_hermes: bool,
    /// Use new architecture
    new_architecture: bool,
}

impl ReactNativeAdapter {
    pub fn new() -> Self {
        Self {
            use_hermes: true,
            new_architecture: false,
        }
    }

    /// Enable or disable Hermes engine
    pub fn with_hermes(mut self, enabled: bool) -> Self {
        self.use_hermes = enabled;
        self
    }

    /// Enable new architecture (Fabric/TurboModules)
    pub fn with_new_architecture(mut self, enabled: bool) -> Self {
        self.new_architecture = enabled;
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

    fn parse_package_json(&self, path: &Path) -> Result<PackageJson> {
        let package_json_path = path.join("package.json");
        let content = std::fs::read_to_string(&package_json_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        serde_json::from_str(&content).map_err(|e| FrameworkError::Context {
            context: "parsing package.json".to_string(),
            message: e.to_string(),
        })
    }

    fn write_package_json_version(&self, path: &Path, version: &str) -> Result<()> {
        let package_json_path = path.join("package.json");
        let content = std::fs::read_to_string(&package_json_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        let mut json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            FrameworkError::Context {
                context: "parsing package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        if let Some(obj) = json.as_object_mut() {
            obj.insert("version".to_string(), serde_json::json!(version));
        }

        let updated =
            serde_json::to_string_pretty(&json).map_err(|e| FrameworkError::Context {
                context: "serializing package.json".to_string(),
                message: e.to_string(),
            })?;

        std::fs::write(&package_json_path, updated).map_err(|e| FrameworkError::Context {
            context: "writing package.json".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    async fn build_ios(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // Find the iOS workspace/project
        let ios_dir = ctx.path.join("ios");
        let (workspace, scheme) = self.find_ios_project(&ios_dir)?;

        let config = match ctx.profile {
            BuildProfile::Debug => "Debug",
            BuildProfile::Release | BuildProfile::Profile => "Release",
        };

        // Bundle JavaScript first for release builds
        if matches!(ctx.profile, BuildProfile::Release) {
            self.bundle_js(&ctx.path, Platform::Ios)?;
        }

        let workspace_path = workspace.to_string_lossy().to_string();
        let mut args = vec![
            "-workspace",
            &workspace_path,
            "-scheme",
            &scheme,
            "-configuration",
            config,
            "-sdk",
            "iphoneos",
            "-archivePath",
            "build/ios/archive.xcarchive",
            "archive",
        ];

        // Code signing for release
        if matches!(ctx.profile, BuildProfile::Release) {
            args.extend_from_slice(&["CODE_SIGN_STYLE=Manual", "-allowProvisioningUpdates"]);
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

        // Export IPA for release builds
        if matches!(ctx.profile, BuildProfile::Release) {
            self.export_ipa(&ctx.path).await?;
        }

        self.find_artifacts(&ctx.path, Platform::Ios, ctx)
    }

    fn find_ios_project(&self, ios_dir: &Path) -> Result<(std::path::PathBuf, String)> {
        // Look for .xcworkspace files first (CocoaPods)
        if let Ok(entries) = std::fs::read_dir(ios_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "xcworkspace").unwrap_or(false) {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    return Ok((path, name));
                }
            }
        }

        // Fall back to .xcodeproj
        if let Ok(entries) = std::fs::read_dir(ios_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().map(|e| e == "xcodeproj").unwrap_or(false) {
                    let name = path.file_stem().unwrap().to_string_lossy().to_string();
                    return Ok((path, name));
                }
            }
        }

        Err(FrameworkError::Context {
            context: "finding iOS project".to_string(),
            message: "No .xcworkspace or .xcodeproj found in ios directory".to_string(),
        })
    }

    fn bundle_js(&self, path: &Path, platform: Platform) -> Result<()> {
        let platform_name = match platform {
            Platform::Ios => "ios",
            Platform::Android => "android",
            _ => return Ok(()),
        };

        let entry_file = if path.join("index.js").exists() {
            "index.js"
        } else {
            "index.tsx"
        };

        let bundle_path = match platform {
            Platform::Ios => "ios/main.jsbundle",
            Platform::Android => "android/app/src/main/assets/index.android.bundle",
            _ => return Ok(()),
        };

        // Ensure assets directory exists for Android
        if platform == Platform::Android {
            std::fs::create_dir_all(path.join("android/app/src/main/assets")).ok();
        }

        let output = self.run_npx(
            &[
                "react-native",
                "bundle",
                "--platform",
                platform_name,
                "--dev",
                "false",
                "--entry-file",
                entry_file,
                "--bundle-output",
                bundle_path,
                "--assets-dest",
                match platform {
                    Platform::Ios => "ios",
                    Platform::Android => "android/app/src/main/res",
                    _ => ".",
                },
            ],
            path,
        )?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: platform_name.to_string(),
                message: format!(
                    "JS bundle failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                source: None,
            });
        }

        Ok(())
    }

    async fn export_ipa(&self, path: &Path) -> Result<()> {
        let export_options = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>method</key>
    <string>app-store</string>
</dict>
</plist>"#;

        let options_path = path.join("build/ios/ExportOptions.plist");
        std::fs::create_dir_all(options_path.parent().unwrap()).ok();
        std::fs::write(&options_path, export_options).map_err(|e| FrameworkError::Context {
            context: "writing export options".to_string(),
            message: e.to_string(),
        })?;

        let output = Command::new("xcodebuild")
            .args([
                "-exportArchive",
                "-archivePath",
                "build/ios/archive.xcarchive",
                "-exportPath",
                "build/ios/ipa",
                "-exportOptionsPlist",
                "build/ios/ExportOptions.plist",
            ])
            .current_dir(path)
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

    async fn build_android(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // Bundle JavaScript first for release builds
        if matches!(ctx.profile, BuildProfile::Release) {
            self.bundle_js(&ctx.path, Platform::Android)?;
        }

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

        let mut cmd = Command::new(gradle_wrapper);
        cmd.arg(task).current_dir(ctx.path.join("android"));

        // Pass Hermes setting
        if self.use_hermes {
            cmd.env("HERMES_ENABLED", "true");
        }

        // Pass new architecture setting
        if self.new_architecture {
            cmd.env("RCT_NEW_ARCH_ENABLED", "1");
        }

        let output = cmd.output().map_err(|e| FrameworkError::CommandFailed {
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
                                .with_framework("react-native")
                                .with_signed(true);

                            if let Some(ref v) = ctx.version {
                                artifact.metadata = artifact.metadata.with_version(v);
                            }

                            artifacts.push(artifact);
                        }
                    }
                }

                // Check for .app (debug builds)
                let ios_dir = path.join("ios");
                if let Ok(entries) = std::fs::read_dir(&ios_dir) {
                    for entry in entries.flatten() {
                        let build_path = entry.path().join("build");
                        if build_path.exists() {
                            if let Ok(build_entries) = std::fs::read_dir(&build_path) {
                                for build_entry in build_entries.flatten() {
                                    if build_entry
                                        .path()
                                        .extension()
                                        .map(|e| e == "app")
                                        .unwrap_or(false)
                                    {
                                        let artifact = Artifact::new(
                                            build_entry.path(),
                                            ArtifactKind::App,
                                            Platform::Ios,
                                        );
                                        artifacts.push(artifact);
                                    }
                                }
                            }
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
                            let mut artifact =
                                Artifact::new(entry.path(), ArtifactKind::Aab, Platform::Android);
                            artifact.metadata =
                                ArtifactMetadata::new().with_framework("react-native");
                            artifacts.push(artifact);
                        }
                    }
                }

                // Check for APK
                for variant in ["debug", "release"] {
                    let apk_dir =
                        path.join(format!("android/app/build/outputs/apk/{}", variant));
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
            _ => {}
        }

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: path.join(format!("build/{}", platform.as_str())),
            });
        }

        Ok(artifacts)
    }

    fn update_ios_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        // Find Info.plist
        let ios_dir = path.join("ios");
        let info_plist = self.find_info_plist(&ios_dir)?;

        let output = Command::new("/usr/libexec/PlistBuddy")
            .args([
                "-c",
                &format!("Set :CFBundleShortVersionString {}", version.version),
                info_plist.to_string_lossy().as_ref(),
            ])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "PlistBuddy".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::Context {
                context: "updating iOS version".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        if let Some(bn) = version.build_number {
            let output = Command::new("/usr/libexec/PlistBuddy")
                .args([
                    "-c",
                    &format!("Set :CFBundleVersion {}", bn),
                    info_plist.to_string_lossy().as_ref(),
                ])
                .output()
                .map_err(|e| FrameworkError::CommandFailed {
                    command: "PlistBuddy".to_string(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                })?;

            if !output.status.success() {
                return Err(FrameworkError::Context {
                    context: "updating iOS build number".to_string(),
                    message: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
        }

        Ok(())
    }

    fn find_info_plist(&self, ios_dir: &Path) -> Result<std::path::PathBuf> {
        // Common locations for Info.plist in RN projects
        let common_paths = ["Info.plist", "*/Info.plist"];

        for pattern in common_paths {
            for entry in glob::glob(&ios_dir.join(pattern).to_string_lossy()).into_iter().flatten()
            {
                if let Ok(path) = entry {
                    if path.is_file() {
                        return Ok(path);
                    }
                }
            }
        }

        Err(FrameworkError::Context {
            context: "finding Info.plist".to_string(),
            message: "Info.plist not found in ios directory".to_string(),
        })
    }

    fn update_android_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let build_gradle = path.join("android/app/build.gradle");

        if !build_gradle.exists() {
            // Try build.gradle.kts
            let build_gradle_kts = path.join("android/app/build.gradle.kts");
            if build_gradle_kts.exists() {
                return self.update_android_version_kts(&build_gradle_kts, version);
            }
            return Err(FrameworkError::Context {
                context: "finding build.gradle".to_string(),
                message: "android/app/build.gradle not found".to_string(),
            });
        }

        let content = std::fs::read_to_string(&build_gradle).map_err(|e| {
            FrameworkError::Context {
                context: "reading build.gradle".to_string(),
                message: e.to_string(),
            }
        })?;

        // Update versionName
        let re_version = regex::Regex::new(r#"versionName\s+"[^"]*""#).unwrap();
        let content = re_version
            .replace(&content, format!(r#"versionName "{}""#, version.version))
            .to_string();

        // Update versionCode
        let content = if let Some(bn) = version.build_number {
            let re_code = regex::Regex::new(r"versionCode\s+\d+").unwrap();
            re_code
                .replace(&content, format!("versionCode {}", bn))
                .to_string()
        } else {
            content
        };

        std::fs::write(&build_gradle, content).map_err(|e| FrameworkError::Context {
            context: "writing build.gradle".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    fn update_android_version_kts(
        &self,
        build_gradle: &Path,
        version: &VersionInfo,
    ) -> Result<()> {
        let content = std::fs::read_to_string(build_gradle).map_err(|e| {
            FrameworkError::Context {
                context: "reading build.gradle.kts".to_string(),
                message: e.to_string(),
            }
        })?;

        // Update versionName
        let re_version = regex::Regex::new(r#"versionName\s*=\s*"[^"]*""#).unwrap();
        let content = re_version
            .replace(&content, format!(r#"versionName = "{}""#, version.version))
            .to_string();

        // Update versionCode
        let content = if let Some(bn) = version.build_number {
            let re_code = regex::Regex::new(r"versionCode\s*=\s*\d+").unwrap();
            re_code
                .replace(&content, format!("versionCode = {}", bn))
                .to_string()
        } else {
            content
        };

        std::fs::write(build_gradle, content).map_err(|e| FrameworkError::Context {
            context: "writing build.gradle.kts".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

impl Default for ReactNativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct PackageJson {
    #[allow(dead_code)]
    name: String,
    #[serde(default)]
    version: Option<String>,
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

        // Check for watchman (recommended)
        match which::which("watchman") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("watchman", None));
            }
            Err(_) => {
                // watchman is optional
                status = status.with_tool(ToolStatus::missing(
                    "watchman",
                    "(Optional) Install with: brew install watchman",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // Install dependencies first
        let install_output = self.run_npx(&["npm", "install"], &ctx.path)?;
        if !install_output.status.success() {
            // Try yarn
            let _ = Command::new("yarn")
                .arg("install")
                .current_dir(&ctx.path)
                .output();
        }

        match ctx.platform {
            Platform::Ios => self.build_ios(ctx).await,
            Platform::Android => self.build_android(ctx).await,
            _ => Err(FrameworkError::UnsupportedPlatform {
                platform: ctx.platform.as_str().to_string(),
                framework: "react-native".to_string(),
            }),
        }
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        // Clean iOS
        let ios_build = path.join("ios/build");
        if ios_build.exists() {
            std::fs::remove_dir_all(&ios_build).ok();
        }

        // Clean Android
        let android_build = path.join("android/app/build");
        if android_build.exists() {
            std::fs::remove_dir_all(&android_build).ok();
        }

        // Clean node_modules/.cache
        let cache = path.join("node_modules/.cache");
        if cache.exists() {
            std::fs::remove_dir_all(&cache).ok();
        }

        // Clean Metro cache
        let _ = self.run_npx(&["react-native", "start", "--reset-cache"], path);

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let pkg = self.parse_package_json(path)?;

        let version = pkg.version.unwrap_or_else(|| "1.0.0".to_string());

        // Try to get build number from native projects
        let build_number: Option<u64> = self.get_ios_build_number(path).or_else(|| self.get_android_version_code(path));

        Ok(VersionInfo {
            version,
            build_number,
            ..Default::default()
        })
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        // Update package.json
        self.write_package_json_version(path, &version.version)?;

        // Update iOS
        if path.join("ios").exists() {
            self.update_ios_version(path, version)?;
        }

        // Update Android
        if path.join("android").exists() {
            self.update_android_version(path, version)?;
        }

        Ok(())
    }
}

impl ReactNativeAdapter {
    fn get_ios_build_number(&self, path: &Path) -> Option<u64> {
        let ios_dir = path.join("ios");
        let info_plist = self.find_info_plist(&ios_dir).ok()?;

        let output = Command::new("/usr/libexec/PlistBuddy")
            .args(["-c", "Print :CFBundleVersion", info_plist.to_string_lossy().as_ref()])
            .output()
            .ok()?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .ok()
                .and_then(|s| s.trim().parse().ok())
        } else {
            None
        }
    }

    fn get_android_version_code(&self, path: &Path) -> Option<u64> {
        let build_gradle = path.join("android/app/build.gradle");
        let content = std::fs::read_to_string(&build_gradle).ok()?;

        let re = regex::Regex::new(r"versionCode\s+(\d+)").ok()?;
        re.captures(&content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse().ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_rn_project(temp: &TempDir) {
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test-app", "version": "1.2.3", "dependencies": {"react": "18.2.0", "react-native": "0.72.0"}}"#,
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("ios")).unwrap();
        std::fs::create_dir_all(temp.path().join("android")).unwrap();

        std::fs::write(temp.path().join("index.js"), "import { AppRegistry } from 'react-native';").unwrap();
    }

    #[test]
    fn test_react_native_detection() {
        let adapter = ReactNativeAdapter::new();
        let temp = TempDir::new().unwrap();

        // No detection without package.json
        assert!(!adapter.detect(temp.path()).detected());

        // Create React Native project
        create_rn_project(&temp);

        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }

    #[test]
    fn test_react_native_not_detected_for_expo() {
        let adapter = ReactNativeAdapter::new();
        let temp = TempDir::new().unwrap();

        // Create Expo project
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "dependencies": {"react-native": "^0.72.0", "expo": "^49.0.0"}}"#,
        )
        .unwrap();

        // Should NOT be detected as bare RN (has expo)
        assert!(!adapter.detect(temp.path()).detected());
    }

    #[test]
    fn test_react_native_version_parsing() {
        let adapter = ReactNativeAdapter::new();
        let temp = TempDir::new().unwrap();
        create_rn_project(&temp);

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version.version, "1.2.3");
    }
}
