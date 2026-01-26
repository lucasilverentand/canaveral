//! Native iOS (Swift/Objective-C) framework adapter
//!
//! Supports building native iOS apps using xcodebuild.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use plist::Value as PlistValue;

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Native iOS build adapter
pub struct NativeIosAdapter;

impl NativeIosAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Find xcworkspace or xcodeproj in the given path
    fn find_xcode_project(&self, path: &Path) -> Result<XcodeProject> {
        let mut workspace: Option<PathBuf> = None;
        let mut project: Option<PathBuf> = None;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.ends_with(".xcworkspace") && !name_str.starts_with("project.") {
                    workspace = Some(entry_path);
                } else if name_str.ends_with(".xcodeproj") {
                    project = Some(entry_path);
                }
            }
        }

        // Prefer workspace over project (CocoaPods/SPM)
        if let Some(ws) = workspace {
            Ok(XcodeProject::Workspace(ws))
        } else if let Some(proj) = project {
            Ok(XcodeProject::Project(proj))
        } else {
            Err(FrameworkError::InvalidConfig {
                message: "No .xcworkspace or .xcodeproj found".to_string(),
            })
        }
    }

    /// List available schemes
    fn list_schemes(&self, path: &Path, project: &XcodeProject) -> Result<Vec<String>> {
        let mut args = vec!["-list", "-json"];
        match project {
            XcodeProject::Workspace(ws) => {
                args.push("-workspace");
                args.push(ws.to_str().unwrap());
            }
            XcodeProject::Project(proj) => {
                args.push("-project");
                args.push(proj.to_str().unwrap());
            }
        }

        let output = Command::new("xcodebuild")
            .args(&args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcodebuild {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "xcodebuild -list".to_string(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| FrameworkError::Context {
                context: "parsing xcodebuild -list output".to_string(),
                message: e.to_string(),
            })?;

        let schemes = if let Some(ws) = json.get("workspace") {
            ws.get("schemes")
        } else {
            json.get("project").and_then(|p| p.get("schemes"))
        };

        let schemes = schemes
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(schemes)
    }

    /// Get default scheme (first non-test scheme or first scheme)
    fn get_default_scheme(&self, schemes: &[String]) -> Option<String> {
        // Prefer schemes that don't end with Tests
        schemes
            .iter()
            .find(|s| !s.ends_with("Tests") && !s.ends_with("UITests"))
            .or_else(|| schemes.first())
            .cloned()
    }

    /// Find Info.plist path
    fn find_info_plist(&self, path: &Path) -> Result<PathBuf> {
        // Common locations
        let candidates = [
            path.join("Info.plist"),
            path.join("App/Info.plist"),
            path.join("Sources/Info.plist"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        // Search recursively (up to 3 levels)
        self.find_file_recursive(path, "Info.plist", 3)
            .ok_or_else(|| FrameworkError::Context {
                context: "finding Info.plist".to_string(),
                message: "Info.plist not found".to_string(),
            })
    }

    /// Find a file recursively up to max_depth
    fn find_file_recursive(&self, path: &Path, filename: &str, max_depth: usize) -> Option<PathBuf> {
        if max_depth == 0 {
            return None;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file() && entry_path.file_name().map(|n| n == filename).unwrap_or(false) {
                    // Skip build directories
                    let path_str = entry_path.to_string_lossy();
                    if !path_str.contains("DerivedData")
                        && !path_str.contains("build/")
                        && !path_str.contains(".build/")
                        && !path_str.contains("Pods/")
                    {
                        return Some(entry_path);
                    }
                }
            }

            // Recurse into subdirectories
            for entry in std::fs::read_dir(path).ok()?.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let name = entry_path.file_name()?.to_string_lossy();
                    // Skip common non-source directories
                    if !name.starts_with('.')
                        && name != "DerivedData"
                        && name != "build"
                        && name != "Pods"
                        && name != "Carthage"
                    {
                        if let Some(found) = self.find_file_recursive(&entry_path, filename, max_depth - 1) {
                            return Some(found);
                        }
                    }
                }
            }
        }

        None
    }

    /// Create export options plist for IPA export
    fn create_export_options(&self, ctx: &BuildContext, path: &Path) -> Result<PathBuf> {
        let export_options_path = path.join("ExportOptions.plist");

        let method = match ctx.profile {
            BuildProfile::Debug => "development",
            BuildProfile::Release | BuildProfile::Profile => {
                // Check if signing config specifies adhoc or enterprise
                if let Some(ref signing) = ctx.signing {
                    if signing.provisioning_profile.as_ref().map(|p| p.contains("AdHoc")).unwrap_or(false) {
                        "ad-hoc"
                    } else if signing.provisioning_profile.as_ref().map(|p| p.contains("Enterprise")).unwrap_or(false) {
                        "enterprise"
                    } else {
                        "app-store"
                    }
                } else {
                    "app-store"
                }
            }
        };

        let mut options = plist::Dictionary::new();
        options.insert("method".to_string(), PlistValue::String(method.to_string()));

        if let Some(ref signing) = ctx.signing {
            if let Some(ref team_id) = signing.team_id {
                options.insert("teamID".to_string(), PlistValue::String(team_id.clone()));
            }
            if signing.automatic {
                options.insert("signingStyle".to_string(), PlistValue::String("automatic".to_string()));
            } else {
                options.insert("signingStyle".to_string(), PlistValue::String("manual".to_string()));
                if let Some(ref profile) = signing.provisioning_profile {
                    options.insert("provisioningProfiles".to_string(), PlistValue::Dictionary({
                        let mut profiles = plist::Dictionary::new();
                        // Get bundle ID from context or Info.plist
                        let bundle_id = ctx.config.get("bundle_id")
                            .and_then(|v| v.as_str())
                            .map(String::from)
                            .unwrap_or_else(|| "com.example.app".to_string());
                        profiles.insert(bundle_id, PlistValue::String(profile.clone()));
                        profiles
                    }));
                }
            }
        } else {
            // Default to automatic signing
            options.insert("signingStyle".to_string(), PlistValue::String("automatic".to_string()));
        }

        // Add common options
        options.insert("compileBitcode".to_string(), PlistValue::Boolean(false));
        options.insert("uploadSymbols".to_string(), PlistValue::Boolean(true));

        let plist_value = PlistValue::Dictionary(options);
        let file = std::fs::File::create(&export_options_path)
            .map_err(|e| FrameworkError::Io(e))?;
        plist::to_writer_xml(file, &plist_value)
            .map_err(|e| FrameworkError::Context {
                context: "writing ExportOptions.plist".to_string(),
                message: e.to_string(),
            })?;

        Ok(export_options_path)
    }

    /// Run xcodebuild command
    fn run_xcodebuild(&self, args: &[&str], path: &Path, env: &HashMap<String, String>) -> Result<std::process::Output> {
        let mut cmd = Command::new("xcodebuild");
        cmd.args(args)
            .current_dir(path)
            .envs(env);

        let output = cmd.output().map_err(|e| FrameworkError::CommandFailed {
            command: format!("xcodebuild {}", args.join(" ")),
            exit_code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        })?;

        Ok(output)
    }

    /// Find built artifacts in archive/export directory
    fn find_artifacts(&self, path: &Path, is_archive: bool) -> Vec<Artifact> {
        let mut artifacts = Vec::new();

        // Check for IPA files
        if let Some(ipa) = self.find_file_recursive(path, "*.ipa", 5)
            .or_else(|| self.find_files_with_extension(path, "ipa").into_iter().next())
        {
            let metadata = ArtifactMetadata::new()
                .with_framework("native-ios")
                .with_signed(true);
            artifacts.push(Artifact::new(ipa, ArtifactKind::Ipa, Platform::Ios).with_metadata(metadata));
        }

        // Check for xcarchive
        if is_archive {
            if let Some(archive) = self.find_files_with_extension(path, "xcarchive").into_iter().next() {
                let metadata = ArtifactMetadata::new().with_framework("native-ios");
                artifacts.push(Artifact::new(archive, ArtifactKind::XcArchive, Platform::Ios).with_metadata(metadata));
            }
        }

        // Check for app bundles
        for app in self.find_files_with_extension(path, "app") {
            // Skip if we already have an IPA
            if artifacts.iter().any(|a| matches!(a.kind, ArtifactKind::Ipa)) {
                continue;
            }
            let metadata = ArtifactMetadata::new().with_framework("native-ios");
            artifacts.push(Artifact::new(app, ArtifactKind::App, Platform::Ios).with_metadata(metadata));
        }

        artifacts
    }

    /// Find files with given extension
    fn find_files_with_extension(&self, path: &Path, ext: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();

        fn walk(dir: &Path, ext: &str, results: &mut Vec<PathBuf>, depth: usize) {
            if depth > 5 {
                return;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, ext, results, depth + 1);
                    } else if path.extension().map(|e| e == ext).unwrap_or(false) {
                        results.push(path);
                    }
                }
            }
        }

        walk(path, ext, &mut results, 0);
        results
    }

    /// Parse version from Info.plist
    fn parse_info_plist_version(&self, plist_path: &Path) -> Result<VersionInfo> {
        let plist: plist::Dictionary = plist::from_file(plist_path)
            .map_err(|e| FrameworkError::Context {
                context: "parsing Info.plist".to_string(),
                message: e.to_string(),
            })?;

        let version = plist.get("CFBundleShortVersionString")
            .and_then(|v| v.as_string())
            .map(String::from)
            .ok_or_else(|| FrameworkError::VersionParseError {
                message: "CFBundleShortVersionString not found in Info.plist".to_string(),
            })?;

        let build_number = plist.get("CFBundleVersion")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse::<u64>().ok());

        Ok(VersionInfo {
            version,
            build_number,
            ..Default::default()
        })
    }

    /// Update version in Info.plist
    fn update_info_plist_version(&self, plist_path: &Path, version: &VersionInfo) -> Result<()> {
        let mut plist: plist::Dictionary = plist::from_file(plist_path)
            .map_err(|e| FrameworkError::Context {
                context: "reading Info.plist".to_string(),
                message: e.to_string(),
            })?;

        plist.insert(
            "CFBundleShortVersionString".to_string(),
            PlistValue::String(version.version.clone()),
        );

        if let Some(build) = version.build_number {
            plist.insert(
                "CFBundleVersion".to_string(),
                PlistValue::String(build.to_string()),
            );
        }

        let file = std::fs::File::create(plist_path)
            .map_err(|e| FrameworkError::Io(e))?;
        plist::to_writer_xml(file, &PlistValue::Dictionary(plist))
            .map_err(|e| FrameworkError::Context {
                context: "writing Info.plist".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }
}

impl Default for NativeIosAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Xcode project type
enum XcodeProject {
    Workspace(PathBuf),
    Project(PathBuf),
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

                if name_str.ends_with(".xcworkspace") && !name_str.starts_with("project.") {
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
                // Get Xcode version
                let version = Command::new("xcodebuild")
                    .arg("-version")
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
                        s.lines()
                            .next()
                            .and_then(|line| line.strip_prefix("Xcode "))
                            .map(String::from)
                    });

                status = status.with_tool(ToolStatus::found("xcodebuild", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "xcodebuild",
                    "Install Xcode from the App Store",
                ));
            }
        }

        // Check for xcrun
        match which::which("xcrun") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("xcrun", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "xcrun",
                    "Install Xcode Command Line Tools: xcode-select --install",
                ));
            }
        }

        // Check for CocoaPods (optional)
        match which::which("pod") {
            Ok(_) => {
                let version = Command::new("pod")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok().map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    });
                status = status.with_tool(ToolStatus::found("pod", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "pod",
                    "(Optional) Install CocoaPods: gem install cocoapods",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let path = &ctx.path;

        // Find Xcode project
        let xcode_project = self.find_xcode_project(path)?;

        // List and select scheme
        let schemes = self.list_schemes(path, &xcode_project)?;
        let scheme = ctx.config.get("scheme")
            .and_then(|v| v.as_str())
            .map(String::from)
            .or_else(|| self.get_default_scheme(&schemes))
            .ok_or_else(|| FrameworkError::InvalidConfig {
                message: "No scheme found or specified. Use --config scheme=<name>".to_string(),
            })?;

        // Determine configuration
        let configuration = match ctx.profile {
            BuildProfile::Debug => "Debug",
            BuildProfile::Release | BuildProfile::Profile => "Release",
        };

        // Output directory
        let output_dir = ctx.output_dir.clone().unwrap_or_else(|| path.join("build/ios"));
        std::fs::create_dir_all(&output_dir).map_err(|e| FrameworkError::Io(e))?;

        let archive_path = output_dir.join(format!("{}.xcarchive", scheme));
        let export_path = output_dir.clone();

        // Install CocoaPods if Podfile exists
        if file_exists(path, "Podfile") {
            let pods_installed = file_exists(path, "Pods");
            if !pods_installed {
                let output = Command::new("pod")
                    .args(["install"])
                    .current_dir(path)
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: "pod install".to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;

                if !output.status.success() {
                    return Err(FrameworkError::BuildFailed {
                        platform: "iOS".to_string(),
                        message: format!(
                            "pod install failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ),
                        source: None,
                    });
                }
            }
        }

        // Prepare environment
        let mut env = ctx.env.clone();
        if ctx.ci {
            env.insert("CI".to_string(), "true".to_string());
        }

        // Build archive
        let mut archive_args = vec![
            "archive",
            "-scheme", &scheme,
            "-configuration", configuration,
            "-archivePath", archive_path.to_str().unwrap(),
            "-destination", "generic/platform=iOS",
            "-allowProvisioningUpdates",
        ];

        match &xcode_project {
            XcodeProject::Workspace(ws) => {
                archive_args.push("-workspace");
                archive_args.push(ws.to_str().unwrap());
            }
            XcodeProject::Project(proj) => {
                archive_args.push("-project");
                archive_args.push(proj.to_str().unwrap());
            }
        }

        // Add signing if configured
        if let Some(ref signing) = ctx.signing {
            if let Some(ref team_id) = signing.team_id {
                archive_args.push("DEVELOPMENT_TEAM");
                let team_arg = format!("DEVELOPMENT_TEAM={}", team_id);
                archive_args.push(Box::leak(team_arg.into_boxed_str()));
            }
            if !signing.automatic {
                archive_args.push("CODE_SIGN_STYLE=Manual");
                if let Some(ref identity) = signing.identity {
                    let identity_arg = format!("CODE_SIGN_IDENTITY={}", identity);
                    archive_args.push(Box::leak(identity_arg.into_boxed_str()));
                }
            }
        }

        let output = self.run_xcodebuild(&archive_args, path, &env)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "iOS".to_string(),
                message: format!(
                    "xcodebuild archive failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                source: None,
            });
        }

        // For release builds, export IPA
        if matches!(ctx.profile, BuildProfile::Release | BuildProfile::Profile) {
            let export_options = self.create_export_options(ctx, path)?;

            let export_args = vec![
                "-exportArchive",
                "-archivePath", archive_path.to_str().unwrap(),
                "-exportPath", export_path.to_str().unwrap(),
                "-exportOptionsPlist", export_options.to_str().unwrap(),
                "-allowProvisioningUpdates",
            ];

            let output = self.run_xcodebuild(&export_args, path, &env)?;

            if !output.status.success() {
                return Err(FrameworkError::BuildFailed {
                    platform: "iOS".to_string(),
                    message: format!(
                        "xcodebuild export failed:\n{}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                    source: None,
                });
            }

            // Clean up export options
            let _ = std::fs::remove_file(&export_options);
        }

        // Find and return artifacts
        let artifacts = self.find_artifacts(&output_dir, true);

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: output_dir,
            });
        }

        Ok(artifacts)
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        // Clean DerivedData
        let derived_data = path.join("DerivedData");
        if derived_data.exists() {
            std::fs::remove_dir_all(&derived_data).map_err(|e| FrameworkError::Io(e))?;
        }

        // Clean build directory
        let build_dir = path.join("build");
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir).map_err(|e| FrameworkError::Io(e))?;
        }

        // Run xcodebuild clean if project exists
        if let Ok(xcode_project) = self.find_xcode_project(path) {
            let (flag, project_path) = match &xcode_project {
                XcodeProject::Workspace(ws) => ("-workspace", ws.to_string_lossy().to_string()),
                XcodeProject::Project(proj) => ("-project", proj.to_string_lossy().to_string()),
            };
            let args = vec!["clean", flag, &project_path];

            let _ = self.run_xcodebuild(&args, path, &HashMap::new());
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let info_plist = self.find_info_plist(path)?;
        self.parse_info_plist_version(&info_plist)
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let info_plist = self.find_info_plist(path)?;
        self.update_info_plist_version(&info_plist, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_detection_xcodeproj() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcodeproj")).unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::Yes(80)));
    }

    #[test]
    fn test_detection_xcworkspace() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcworkspace")).unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::Yes(85)));
    }

    #[test]
    fn test_detection_no_project() {
        let dir = tempdir().unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::No));
    }

    #[test]
    fn test_version_parsing() {
        let dir = tempdir().unwrap();
        let plist_path = dir.path().join("Info.plist");

        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleShortVersionString</key>
    <string>1.2.3</string>
    <key>CFBundleVersion</key>
    <string>42</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.myapp</string>
</dict>
</plist>"#;

        let mut file = std::fs::File::create(&plist_path).unwrap();
        file.write_all(plist_content.as_bytes()).unwrap();

        let adapter = NativeIosAdapter::new();
        let version = adapter.parse_info_plist_version(&plist_path).unwrap();

        assert_eq!(version.version, "1.2.3");
        assert_eq!(version.build_number, Some(42));
    }

    #[test]
    fn test_capabilities() {
        let adapter = NativeIosAdapter::new();
        let caps = adapter.capabilities();

        assert!(caps.has(Capability::BuildIos));
        assert!(caps.has(Capability::ReleaseBuild));
        assert!(caps.has(Capability::DebugBuild));
    }
}
