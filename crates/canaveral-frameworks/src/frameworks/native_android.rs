//! Native Android (Kotlin/Java) framework adapter
//!
//! Supports building native Android apps using Gradle.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use regex::Regex;

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::{Capabilities, Capability};
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Native Android build adapter
pub struct NativeAndroidAdapter;

impl NativeAndroidAdapter {
    pub fn new() -> Self {
        Self
    }

    /// Get the gradle command (wrapper or global)
    fn gradle_cmd(&self, path: &Path) -> String {
        let wrapper = if cfg!(windows) {
            path.join("gradlew.bat")
        } else {
            path.join("gradlew")
        };

        if wrapper.exists() {
            wrapper.to_string_lossy().to_string()
        } else {
            "gradle".to_string()
        }
    }

    /// Run gradle command
    fn run_gradle(&self, args: &[&str], path: &Path, env: &HashMap<String, String>) -> Result<std::process::Output> {
        let gradle = self.gradle_cmd(path);

        let mut cmd = Command::new(&gradle);
        cmd.args(args)
            .current_dir(path)
            .envs(env);

        // Ensure ANDROID_HOME is set
        if !env.contains_key("ANDROID_HOME") {
            if let Ok(android_home) = std::env::var("ANDROID_HOME") {
                cmd.env("ANDROID_HOME", android_home);
            } else if let Ok(android_sdk) = std::env::var("ANDROID_SDK_ROOT") {
                cmd.env("ANDROID_HOME", android_sdk);
            }
        }

        let output = cmd.output().map_err(|e| FrameworkError::CommandFailed {
            command: format!("{} {}", gradle, args.join(" ")),
            exit_code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        })?;

        Ok(output)
    }

    /// Find app/build.gradle or app/build.gradle.kts
    fn find_app_build_gradle(&self, path: &Path) -> Option<PathBuf> {
        let candidates = [
            path.join("app/build.gradle.kts"),
            path.join("app/build.gradle"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Some(candidate.clone());
            }
        }

        None
    }

    /// Parse version from build.gradle
    fn parse_gradle_version(&self, build_gradle: &Path) -> Result<VersionInfo> {
        let content = std::fs::read_to_string(build_gradle)
            .map_err(|e| FrameworkError::Context {
                context: "reading build.gradle".to_string(),
                message: e.to_string(),
            })?;

        let is_kotlin = build_gradle
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "kts")
            .unwrap_or(false);

        // Parse versionName
        let version_regex = if is_kotlin {
            Regex::new(r#"versionName\s*=\s*"([^"]+)""#).unwrap()
        } else {
            Regex::new(r#"versionName\s+["']([^"']+)["']"#).unwrap()
        };

        let version = version_regex
            .captures(&content)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .ok_or_else(|| FrameworkError::VersionParseError {
                message: "versionName not found in build.gradle".to_string(),
            })?;

        // Parse versionCode
        let code_regex = if is_kotlin {
            Regex::new(r"versionCode\s*=\s*(\d+)").unwrap()
        } else {
            Regex::new(r"versionCode\s+(\d+)").unwrap()
        };

        let build_number = code_regex
            .captures(&content)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok());

        Ok(VersionInfo {
            version,
            build_number,
            version_code: build_number,
            ..Default::default()
        })
    }

    /// Update version in build.gradle
    fn update_gradle_version(&self, build_gradle: &Path, version: &VersionInfo) -> Result<()> {
        let mut content = std::fs::read_to_string(build_gradle)
            .map_err(|e| FrameworkError::Context {
                context: "reading build.gradle".to_string(),
                message: e.to_string(),
            })?;

        let is_kotlin = build_gradle
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e == "kts")
            .unwrap_or(false);

        // Update versionName
        let version_regex = if is_kotlin {
            Regex::new(r#"(versionName\s*=\s*)"[^"]+""#).unwrap()
        } else {
            Regex::new(r#"(versionName\s+)["'][^"']+["']"#).unwrap()
        };

        let replacement = if is_kotlin {
            format!("$1\"{}\"", version.version)
        } else {
            format!("$1\"{}\"", version.version)
        };

        content = version_regex.replace(&content, replacement.as_str()).to_string();

        // Update versionCode if provided
        if let Some(build_number) = version.build_number {
            let code_regex = if is_kotlin {
                Regex::new(r"(versionCode\s*=\s*)\d+").unwrap()
            } else {
                Regex::new(r"(versionCode\s+)\d+").unwrap()
            };

            content = code_regex
                .replace(&content, format!("${{1}}{}", build_number).as_str())
                .to_string();
        }

        std::fs::write(build_gradle, content)
            .map_err(|e| FrameworkError::Io(e))?;

        Ok(())
    }

    /// Find built artifacts
    fn find_artifacts(&self, path: &Path, profile: BuildProfile, flavor: Option<&str>) -> Vec<Artifact> {
        let mut artifacts = Vec::new();

        // Build type directory name
        let build_type = match profile {
            BuildProfile::Debug => "debug",
            BuildProfile::Release | BuildProfile::Profile => "release",
        };

        // Possible output directories
        let mut output_dirs = vec![
            path.join(format!("app/build/outputs/apk/{}", build_type)),
            path.join(format!("app/build/outputs/bundle/{}Release", build_type)),
            path.join(format!("app/build/outputs/bundle/{}", build_type)),
            path.join("app/build/outputs/apk"),
            path.join("app/build/outputs/bundle"),
        ];

        // Add flavor-specific directories
        if let Some(flavor) = flavor {
            output_dirs.insert(0, path.join(format!("app/build/outputs/apk/{}/{}", flavor, build_type)));
            output_dirs.insert(1, path.join(format!("app/build/outputs/bundle/{}{}", flavor, if build_type == "release" { "Release" } else { "Debug" })));
        }

        // Search for APKs
        for dir in &output_dirs {
            if dir.exists() {
                for apk in self.find_files_with_extension(dir, "apk") {
                    let filename = apk.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    // Skip test APKs
                    if filename.contains("androidTest") || filename.contains("-test-") {
                        continue;
                    }
                    let metadata = ArtifactMetadata::new()
                        .with_framework("native-android");
                    artifacts.push(Artifact::new(&apk, ArtifactKind::Apk, Platform::Android).with_metadata(metadata));
                }
            }
        }

        // Search for AABs (only for release builds)
        if matches!(profile, BuildProfile::Release | BuildProfile::Profile) {
            for dir in &output_dirs {
                if dir.exists() {
                    for aab in self.find_files_with_extension(dir, "aab") {
                        let metadata = ArtifactMetadata::new()
                            .with_framework("native-android")
                            .with_signed(true);
                        artifacts.push(Artifact::new(&aab, ArtifactKind::Aab, Platform::Android).with_metadata(metadata));
                    }
                }
            }
        }

        artifacts
    }

    /// Find files with given extension
    fn find_files_with_extension(&self, path: &Path, ext: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();

        fn walk(dir: &Path, ext: &str, results: &mut Vec<PathBuf>, depth: usize) {
            if depth > 3 {
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

    /// Setup signing from context
    fn setup_signing(&self, ctx: &BuildContext, path: &Path) -> Result<()> {
        if let Some(ref signing) = ctx.signing {
            if let Some(ref keystore_path) = signing.keystore_path {
                // Create or update gradle.properties with signing info
                let gradle_props_path = path.join("gradle.properties");
                let mut props = if gradle_props_path.exists() {
                    std::fs::read_to_string(&gradle_props_path)
                        .map_err(|e| FrameworkError::Io(e))?
                } else {
                    String::new()
                };

                // Add signing properties if not present
                if !props.contains("RELEASE_STORE_FILE") {
                    props.push_str(&format!("\nRELEASE_STORE_FILE={}\n", keystore_path.display()));
                }
                if !props.contains("RELEASE_KEY_ALIAS") {
                    if let Some(ref alias) = signing.key_alias {
                        props.push_str(&format!("RELEASE_KEY_ALIAS={}\n", alias));
                    }
                }

                // Note: passwords should be passed via environment variables, not stored in properties
                std::fs::write(&gradle_props_path, props)
                    .map_err(|e| FrameworkError::Io(e))?;
            }
        }

        Ok(())
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

        // Check for Java
        match which::which("java") {
            Ok(_) => {
                let version = Command::new("java")
                    .arg("-version")
                    .output()
                    .ok()
                    .and_then(|o| {
                        // Java outputs version to stderr
                        let stderr = String::from_utf8_lossy(&o.stderr);
                        let version_regex = Regex::new(r#"version "(\d+[\d.]+)"#).ok()?;
                        version_regex
                            .captures(&stderr)
                            .and_then(|c| c.get(1))
                            .map(|m| m.as_str().to_string())
                    });
                status = status.with_tool(ToolStatus::found("java", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "java",
                    "Install JDK 11 or later",
                ));
            }
        }

        // Check for gradle wrapper or global gradle
        let has_wrapper = Path::new("gradlew").exists() || Path::new("gradlew.bat").exists();
        match which::which("gradle") {
            Ok(_) => {
                let version = Command::new("gradle")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            let stdout = String::from_utf8_lossy(&o.stdout);
                            let version_regex = Regex::new(r"Gradle (\d+[\d.]+)").ok()?;
                            version_regex
                                .captures(&stdout)
                                .and_then(|c| c.get(1))
                                .map(|m| m.as_str().to_string())
                        } else {
                            None
                        }
                    });
                status = status.with_tool(ToolStatus::found(
                    "gradle",
                    if has_wrapper {
                        Some("wrapper available".to_string())
                    } else {
                        version
                    },
                ));
            }
            Err(_) if !has_wrapper => {
                status = status.with_tool(ToolStatus::missing(
                    "gradle",
                    "Install Gradle or use gradle wrapper (./gradlew)",
                ));
            }
            _ => {
                status = status.with_tool(ToolStatus::found("gradlew", Some("wrapper".to_string())));
            }
        }

        // Check for adb (optional but useful)
        match which::which("adb") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("adb", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "adb",
                    "(Optional) Install Android SDK Platform Tools",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let path = &ctx.path;

        // Setup signing if configured
        self.setup_signing(ctx, path)?;

        // Determine build task
        let build_type = match ctx.profile {
            BuildProfile::Debug => "Debug",
            BuildProfile::Release | BuildProfile::Profile => "Release",
        };

        // Build task name
        let task = if let Some(ref flavor) = ctx.flavor {
            // Capitalize flavor for task name
            let cap_flavor = format!("{}{}",
                flavor.chars().next().unwrap().to_uppercase(),
                &flavor[1..]);
            format!("assemble{}{}", cap_flavor, build_type)
        } else {
            format!("assemble{}", build_type)
        };

        // Also build AAB for release builds
        let bundle_task = if matches!(ctx.profile, BuildProfile::Release | BuildProfile::Profile) {
            if let Some(ref flavor) = ctx.flavor {
                let cap_flavor = format!("{}{}",
                    flavor.chars().next().unwrap().to_uppercase(),
                    &flavor[1..]);
                Some(format!("bundle{}{}", cap_flavor, build_type))
            } else {
                Some(format!("bundle{}", build_type))
            }
        } else {
            None
        };

        // Prepare environment
        let mut env = ctx.env.clone();
        if ctx.ci {
            env.insert("CI".to_string(), "true".to_string());
        }

        // Pass signing credentials via environment
        if let Some(ref signing) = ctx.signing {
            if signing.keystore_path.is_some() {
                // Passwords should be set as environment variables
                if let Ok(store_pass) = std::env::var("ANDROID_KEYSTORE_PASSWORD") {
                    env.insert("RELEASE_STORE_PASSWORD".to_string(), store_pass);
                }
                if let Ok(key_pass) = std::env::var("ANDROID_KEY_PASSWORD") {
                    env.insert("RELEASE_KEY_PASSWORD".to_string(), key_pass);
                }
            }
        }

        // Build arguments
        let mut args = vec![&task[..], "--stacktrace"];

        if ctx.ci {
            args.push("--no-daemon");
        }

        // Run build
        let output = self.run_gradle(&args, path, &env)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "Android".to_string(),
                message: format!(
                    "Gradle build failed:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ),
                source: None,
            });
        }

        // Build AAB if release
        if let Some(bundle) = bundle_task {
            let bundle_args = vec![&bundle[..], "--stacktrace"];
            let output = self.run_gradle(&bundle_args, path, &env)?;

            if !output.status.success() {
                // Log warning but don't fail - APK was already built
                eprintln!(
                    "Warning: AAB build failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Find artifacts
        let artifacts = self.find_artifacts(path, ctx.profile, ctx.flavor.as_deref());

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: path.join("app/build/outputs"),
            });
        }

        Ok(artifacts)
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        let output = self.run_gradle(&["clean"], path, &HashMap::new())?;

        if !output.status.success() {
            // Still try to clean manually
            let build_dir = path.join("app/build");
            if build_dir.exists() {
                std::fs::remove_dir_all(&build_dir).ok();
            }
        }

        // Also clean .gradle cache
        let gradle_cache = path.join(".gradle");
        if gradle_cache.exists() {
            std::fs::remove_dir_all(&gradle_cache).ok();
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let build_gradle = self.find_app_build_gradle(path)
            .ok_or_else(|| FrameworkError::Context {
                context: "finding build.gradle".to_string(),
                message: "app/build.gradle or app/build.gradle.kts not found".to_string(),
            })?;

        self.parse_gradle_version(&build_gradle)
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let build_gradle = self.find_app_build_gradle(path)
            .ok_or_else(|| FrameworkError::Context {
                context: "finding build.gradle".to_string(),
                message: "app/build.gradle or app/build.gradle.kts not found".to_string(),
            })?;

        self.update_gradle_version(&build_gradle, version)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;
    use tempfile::tempdir;

    #[test]
    fn test_detection_with_manifest() {
        let dir = tempdir().unwrap();
        let app_dir = dir.path().join("app/src/main");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::write(dir.path().join("build.gradle"), "").unwrap();
        std::fs::write(dir.path().join("settings.gradle"), "").unwrap();
        std::fs::write(app_dir.join("AndroidManifest.xml"), "<manifest/>").unwrap();

        let adapter = NativeAndroidAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::Yes(90)));
    }

    #[test]
    fn test_detection_without_manifest() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();
        std::fs::write(dir.path().join("build.gradle"), "").unwrap();
        std::fs::write(dir.path().join("settings.gradle"), "").unwrap();

        let adapter = NativeAndroidAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::Yes(80)));
    }

    #[test]
    fn test_detection_no_gradle() {
        let dir = tempdir().unwrap();

        let adapter = NativeAndroidAdapter::new();
        let detection = adapter.detect(dir.path());

        assert!(matches!(detection, Detection::No));
    }

    #[test]
    fn test_version_parsing_kotlin_dsl() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();

        let gradle_content = r#"
plugins {
    id("com.android.application")
    kotlin("android")
}

android {
    namespace = "com.example.myapp"
    compileSdk = 34

    defaultConfig {
        applicationId = "com.example.myapp"
        minSdk = 24
        targetSdk = 34
        versionCode = 42
        versionName = "1.2.3"
    }
}
"#;

        let gradle_path = dir.path().join("app/build.gradle.kts");
        std::fs::write(&gradle_path, gradle_content).unwrap();

        let adapter = NativeAndroidAdapter::new();
        let version = adapter.parse_gradle_version(&gradle_path).unwrap();

        assert_eq!(version.version, "1.2.3");
        assert_eq!(version.build_number, Some(42));
        assert_eq!(version.version_code, Some(42));
    }

    #[test]
    fn test_version_parsing_groovy_dsl() {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("app")).unwrap();

        let gradle_content = r#"
plugins {
    id 'com.android.application'
    id 'kotlin-android'
}

android {
    namespace 'com.example.myapp'
    compileSdk 34

    defaultConfig {
        applicationId "com.example.myapp"
        minSdkVersion 24
        targetSdkVersion 34
        versionCode 15
        versionName "2.0.0"
    }
}
"#;

        let gradle_path = dir.path().join("app/build.gradle");
        std::fs::write(&gradle_path, gradle_content).unwrap();

        let adapter = NativeAndroidAdapter::new();
        let version = adapter.parse_gradle_version(&gradle_path).unwrap();

        assert_eq!(version.version, "2.0.0");
        assert_eq!(version.build_number, Some(15));
        assert_eq!(version.version_code, Some(15));
    }

    #[test]
    fn test_version_update() {
        let dir = tempdir().unwrap();

        let gradle_content = r#"android {
    defaultConfig {
        applicationId = "com.example.myapp"
        versionCode = 1
        versionName = "1.0.0"
    }
}"#;

        let gradle_path = dir.path().join("build.gradle.kts");
        std::fs::write(&gradle_path, gradle_content).unwrap();

        let adapter = NativeAndroidAdapter::new();
        adapter.update_gradle_version(&gradle_path, &VersionInfo {
            version: "2.0.0".to_string(),
            build_number: Some(42),
            ..Default::default()
        }).unwrap();

        let updated = std::fs::read_to_string(&gradle_path).unwrap();
        assert!(updated.contains("versionName = \"2.0.0\""));
        assert!(updated.contains("versionCode = 42"));
    }

    #[test]
    fn test_capabilities() {
        let adapter = NativeAndroidAdapter::new();
        let caps = adapter.capabilities();

        assert!(caps.has(Capability::BuildAndroid));
        assert!(caps.has(Capability::ReleaseBuild));
        assert!(caps.has(Capability::DebugBuild));
    }
}
