//! Tauri framework adapter
//!
//! Supports building Tauri desktop apps for macOS, Windows, and Linux.

use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, info, warn};
use walkdir::WalkDir;

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
#[cfg(test)]
use crate::capabilities::Capability;
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Tauri build adapter
pub struct TauriAdapter {
    /// Path to cargo executable (auto-detected if None)
    cargo_path: Option<String>,
    /// Use npm/pnpm/yarn tauri CLI instead of cargo-tauri
    use_npm_cli: bool,
}

impl TauriAdapter {
    pub fn new() -> Self {
        Self {
            cargo_path: None,
            use_npm_cli: false,
        }
    }

    pub fn with_cargo_path(path: impl Into<String>) -> Self {
        Self {
            cargo_path: Some(path.into()),
            use_npm_cli: false,
        }
    }

    pub fn with_npm_cli(mut self) -> Self {
        self.use_npm_cli = true;
        self
    }

    fn cargo_cmd(&self) -> String {
        self.cargo_path
            .clone()
            .unwrap_or_else(|| "cargo".to_string())
    }

    fn run_cargo(&self, args: &[&str], path: &Path) -> Result<std::process::Output> {
        let output = Command::new(self.cargo_cmd())
            .args(args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("cargo {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        Ok(output)
    }

    fn run_tauri_cli(&self, args: &[&str], path: &Path) -> Result<std::process::Output> {
        let (cmd, all_args) = if self.use_npm_cli {
            // Use npm/npx to run tauri
            let mut npx_args = vec!["tauri"];
            npx_args.extend_from_slice(args);
            ("npx".to_string(), npx_args)
        } else {
            // Use cargo tauri
            let mut cargo_args = vec!["tauri"];
            cargo_args.extend_from_slice(args);
            (self.cargo_cmd(), cargo_args)
        };

        debug!("Running: {} {}", cmd, all_args.join(" "));

        let output = Command::new(&cmd)
            .args(&all_args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("{} {}", cmd, all_args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        Ok(output)
    }

    /// Find the src-tauri directory
    fn find_tauri_dir(&self, path: &Path) -> Option<PathBuf> {
        let src_tauri = path.join("src-tauri");
        if src_tauri.exists() {
            return Some(src_tauri);
        }
        // Tauri v1 might have tauri.conf.json at root
        if path.join("tauri.conf.json").exists() {
            return Some(path.to_path_buf());
        }
        None
    }

    /// Parse version from tauri.conf.json
    fn parse_tauri_conf_version(&self, path: &Path) -> Result<Option<String>> {
        let tauri_dir = self.find_tauri_dir(path).ok_or_else(|| {
            FrameworkError::Context {
                context: "finding tauri directory".to_string(),
                message: "No src-tauri directory found".to_string(),
            }
        })?;

        let conf_path = tauri_dir.join("tauri.conf.json");
        if !conf_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&conf_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        let json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            FrameworkError::Context {
                context: "parsing tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        // Tauri v2: version is in "version" field
        // Tauri v1: version is in "package.version" field
        if let Some(version) = json.get("version").and_then(|v| v.as_str()) {
            return Ok(Some(version.to_string()));
        }

        if let Some(version) = json
            .get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
        {
            return Ok(Some(version.to_string()));
        }

        Ok(None)
    }

    /// Parse version from Cargo.toml
    fn parse_cargo_version(&self, path: &Path) -> Result<String> {
        let tauri_dir = self.find_tauri_dir(path).ok_or_else(|| {
            FrameworkError::Context {
                context: "finding tauri directory".to_string(),
                message: "No src-tauri directory found".to_string(),
            }
        })?;

        let cargo_path = tauri_dir.join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading Cargo.toml".to_string(),
                message: e.to_string(),
            }
        })?;

        // Parse version from [package] section
        let version_re = Regex::new(r#"(?m)^\s*version\s*=\s*"([^"]+)""#).unwrap();

        if let Some(caps) = version_re.captures(&content) {
            return Ok(caps[1].to_string());
        }

        Err(FrameworkError::VersionParseError {
            message: "No version field found in Cargo.toml".to_string(),
        })
    }

    /// Update version in tauri.conf.json
    fn update_tauri_conf_version(&self, path: &Path, version: &str) -> Result<()> {
        let tauri_dir = self.find_tauri_dir(path).ok_or_else(|| {
            FrameworkError::Context {
                context: "finding tauri directory".to_string(),
                message: "No src-tauri directory found".to_string(),
            }
        })?;

        let conf_path = tauri_dir.join("tauri.conf.json");
        if !conf_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&conf_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        let mut json: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
            FrameworkError::Context {
                context: "parsing tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        // Update version in the appropriate location
        // Tauri v2 uses top-level "version"
        // Tauri v1 uses "package.version"
        if json.get("version").is_some() {
            json["version"] = serde_json::Value::String(version.to_string());
        } else if json.get("package").is_some() {
            json["package"]["version"] = serde_json::Value::String(version.to_string());
        } else {
            // Default to v2 style
            json["version"] = serde_json::Value::String(version.to_string());
        }

        let new_content = serde_json::to_string_pretty(&json).map_err(|e| {
            FrameworkError::Context {
                context: "serializing tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        std::fs::write(&conf_path, new_content).map_err(|e| {
            FrameworkError::Context {
                context: "writing tauri.conf.json".to_string(),
                message: e.to_string(),
            }
        })?;

        Ok(())
    }

    /// Update version in Cargo.toml
    fn update_cargo_version(&self, path: &Path, version: &str) -> Result<()> {
        let tauri_dir = self.find_tauri_dir(path).ok_or_else(|| {
            FrameworkError::Context {
                context: "finding tauri directory".to_string(),
                message: "No src-tauri directory found".to_string(),
            }
        })?;

        let cargo_path = tauri_dir.join("Cargo.toml");
        let content = std::fs::read_to_string(&cargo_path).map_err(|e| {
            FrameworkError::Context {
                context: "reading Cargo.toml".to_string(),
                message: e.to_string(),
            }
        })?;

        // Replace version in [package] section
        let version_re = Regex::new(r#"(?m)^(\s*version\s*=\s*)"[^"]+""#).unwrap();

        let new_content = version_re.replace(&content, format!(r#"$1"{}""#, version));

        std::fs::write(&cargo_path, new_content.as_bytes()).map_err(|e| {
            FrameworkError::Context {
                context: "writing Cargo.toml".to_string(),
                message: e.to_string(),
            }
        })?;

        Ok(())
    }

    /// Find build artifacts
    fn find_artifacts(&self, path: &Path, platform: Platform) -> Result<Vec<Artifact>> {
        let tauri_dir = self.find_tauri_dir(path).unwrap_or_else(|| path.to_path_buf());
        let target_dir = tauri_dir.join("target");

        let mut artifacts = Vec::new();

        // Tauri outputs to target/release/bundle/<platform>/
        let bundle_dir = target_dir.join("release/bundle");
        if !bundle_dir.exists() {
            debug!("Bundle directory does not exist: {:?}", bundle_dir);
            return Ok(artifacts);
        }

        // Get version for metadata
        let version = self.parse_cargo_version(path).ok();

        // Walk the bundle directory and find artifacts
        for entry in WalkDir::new(&bundle_dir).max_depth(3) {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let entry_path = entry.path();
            if !entry_path.is_file() && !entry_path.is_dir() {
                continue;
            }

            let kind = match entry_path.extension().and_then(|e| e.to_str()) {
                Some("dmg") if platform == Platform::MacOs => ArtifactKind::Dmg,
                Some("app") if platform == Platform::MacOs && entry_path.is_dir() => {
                    ArtifactKind::MacApp
                }
                Some("pkg") if platform == Platform::MacOs => ArtifactKind::Pkg,
                Some("exe") if platform == Platform::Windows => ArtifactKind::Exe,
                Some("msi") if platform == Platform::Windows => ArtifactKind::Msi,
                Some("deb") if platform == Platform::Linux => ArtifactKind::Deb,
                Some("rpm") if platform == Platform::Linux => ArtifactKind::Rpm,
                Some("AppImage") if platform == Platform::Linux => ArtifactKind::AppImage,
                _ => {
                    // Check for AppImage without extension
                    if platform == Platform::Linux
                        && entry_path
                            .file_name()
                            .map(|n| n.to_string_lossy().contains(".AppImage"))
                            .unwrap_or(false)
                    {
                        ArtifactKind::AppImage
                    } else {
                        continue;
                    }
                }
            };

            let metadata = ArtifactMetadata::new()
                .with_framework("tauri")
                .with_signed(false);

            let metadata = if let Some(ref v) = version {
                metadata.with_version(v)
            } else {
                metadata
            };

            let artifact = Artifact::new(entry_path.to_path_buf(), kind, platform)
                .with_metadata(metadata)
                .with_sha256();

            artifacts.push(artifact);
        }

        // Also check for platform-specific directories
        let platform_dirs = match platform {
            Platform::MacOs => vec!["macos", "dmg", "app"],
            Platform::Windows => vec!["msi", "nsis"],
            Platform::Linux => vec!["deb", "rpm", "appimage"],
            _ => vec![],
        };

        for dir_name in platform_dirs {
            let platform_dir = bundle_dir.join(dir_name);
            if platform_dir.exists() {
                for entry in std::fs::read_dir(&platform_dir).into_iter().flatten() {
                    if let Ok(entry) = entry {
                        let entry_path = entry.path();
                        let kind = ArtifactKind::from_path(&entry_path);
                        if !matches!(kind, ArtifactKind::Other) {
                            let metadata = ArtifactMetadata::new()
                                .with_framework("tauri")
                                .with_signed(false);

                            let metadata = if let Some(ref v) = version {
                                metadata.with_version(v)
                            } else {
                                metadata
                            };

                            // Avoid duplicates
                            if !artifacts.iter().any(|a| a.path == entry_path) {
                                let artifact = Artifact::new(entry_path, kind, platform)
                                    .with_metadata(metadata)
                                    .with_sha256();

                                artifacts.push(artifact);
                            }
                        }
                    }
                }
            }
        }

        Ok(artifacts)
    }

    /// Determine which package manager is being used
    fn detect_package_manager(&self, path: &Path) -> &'static str {
        if path.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if path.join("yarn.lock").exists() {
            "yarn"
        } else if path.join("bun.lockb").exists() {
            "bun"
        } else {
            "npm"
        }
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

        // Platform-specific checks
        #[cfg(target_os = "macos")]
        {
            // Check for Xcode tools on macOS
            if which::which("xcodebuild").is_ok() {
                status = status.with_tool(ToolStatus::found("xcodebuild", None));
            } else {
                status = status.with_tool(ToolStatus::missing(
                    "xcodebuild",
                    "Install Xcode Command Line Tools: xcode-select --install",
                ));
            }
        }

        #[cfg(target_os = "linux")]
        {
            // Check for common Linux build dependencies
            for lib in &["webkit2gtk-4.1", "libgtk-3-dev"] {
                // Just note these as warnings, not hard requirements
                debug!("Linux build may require: {}", lib);
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let project_path = &ctx.path;
        let platform = ctx.platform;

        info!("Building Tauri app for {:?}", platform);

        // Install frontend dependencies if needed
        let package_manager = self.detect_package_manager(project_path);
        if project_path.join("package.json").exists() {
            let node_modules = project_path.join("node_modules");
            if !node_modules.exists() {
                info!("Installing frontend dependencies with {}", package_manager);
                let install_cmd = match package_manager {
                    "pnpm" => "pnpm install",
                    "yarn" => "yarn install",
                    "bun" => "bun install",
                    _ => "npm install",
                };
                let parts: Vec<&str> = install_cmd.split_whitespace().collect();
                let output = Command::new(parts[0])
                    .args(&parts[1..])
                    .current_dir(project_path)
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: install_cmd.to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;

                if !output.status.success() {
                    return Err(FrameworkError::CommandFailed {
                        command: install_cmd.to_string(),
                        exit_code: output.status.code(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    });
                }
            }
        }

        // Build arguments
        let mut args = vec!["build"];

        // Add release flag if not debug profile
        let is_debug = matches!(ctx.profile, BuildProfile::Debug);
        if is_debug {
            args.push("--debug");
        }

        // Add bundle types based on platform
        let bundles = match platform {
            Platform::MacOs => {
                if matches!(ctx.profile, BuildProfile::Release) {
                    vec!["dmg", "app"]
                } else {
                    vec!["app"]
                }
            }
            Platform::Windows => vec!["msi", "nsis"],
            Platform::Linux => vec!["deb", "appimage"],
            _ => vec![],
        };

        if !bundles.is_empty() {
            args.push("--bundles");
            for bundle in &bundles {
                args.push(bundle);
            }
        }

        // Verbose output for CI
        if ctx.ci {
            args.push("--verbose");
        }

        info!("Running: cargo tauri {}", args.join(" "));

        let output = self.run_tauri_cli(&args, project_path)?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            return Err(FrameworkError::CommandFailed {
                command: format!("cargo tauri {}", args.join(" ")),
                exit_code: output.status.code(),
                stdout: stdout.to_string(),
                stderr: stderr.to_string(),
            });
        }

        // Find and return artifacts
        let artifacts = self.find_artifacts(project_path, platform)?;

        if artifacts.is_empty() {
            warn!("No artifacts found after build");
        } else {
            info!("Found {} artifact(s)", artifacts.len());
            for artifact in &artifacts {
                info!("  - {:?}: {:?}", artifact.kind, artifact.path);
            }
        }

        Ok(artifacts)
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        info!("Cleaning Tauri build artifacts");

        // Clean cargo target
        let tauri_dir = self.find_tauri_dir(path).unwrap_or_else(|| path.to_path_buf());
        let target_dir = tauri_dir.join("target");

        if target_dir.exists() {
            let output = self.run_cargo(&["clean"], &tauri_dir)?;

            if !output.status.success() {
                warn!(
                    "cargo clean failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                );
            }
        }

        // Clean frontend build artifacts
        let frontend_dirs = ["dist", "build", ".next", ".nuxt", ".output"];
        for dir in &frontend_dirs {
            let dir_path = path.join(dir);
            if dir_path.exists() {
                if let Err(e) = std::fs::remove_dir_all(&dir_path) {
                    warn!("Failed to remove {}: {}", dir, e);
                }
            }
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        // Try tauri.conf.json first
        if let Ok(Some(version)) = self.parse_tauri_conf_version(path) {
            return Ok(VersionInfo {
                version,
                ..Default::default()
            });
        }

        // Fall back to Cargo.toml
        let version = self.parse_cargo_version(path)?;

        Ok(VersionInfo {
            version,
            ..Default::default()
        })
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        info!("Setting Tauri version to {}", version.version);

        // Update Cargo.toml
        self.update_cargo_version(path, &version.version)?;

        // Update tauri.conf.json if it exists
        self.update_tauri_conf_version(path, &version.version)?;

        // Also update package.json if it exists (for consistency)
        let package_json = path.join("package.json");
        if package_json.exists() {
            let content = std::fs::read_to_string(&package_json).map_err(|e| {
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

            json["version"] = serde_json::Value::String(version.version.clone());

            let new_content = serde_json::to_string_pretty(&json).map_err(|e| {
                FrameworkError::Context {
                    context: "serializing package.json".to_string(),
                    message: e.to_string(),
                }
            })?;

            std::fs::write(&package_json, new_content).map_err(|e| {
                FrameworkError::Context {
                    context: "writing package.json".to_string(),
                    message: e.to_string(),
                }
            })?;
        }

        Ok(())
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

    #[test]
    fn test_tauri_capabilities() {
        let adapter = TauriAdapter::new();
        let caps = adapter.capabilities();

        assert!(caps.has(Capability::BuildMacos));
        assert!(caps.has(Capability::BuildWindows));
        assert!(caps.has(Capability::BuildLinux));
        // Tauri doesn't build mobile (yet, v2 has mobile but it's separate)
        assert!(!caps.has(Capability::BuildIos));
        assert!(!caps.has(Capability::BuildAndroid));
    }

    #[test]
    fn test_version_parsing() {
        let adapter = TauriAdapter::new();
        let temp = TempDir::new().unwrap();

        // Create Tauri v2 style project
        std::fs::create_dir_all(temp.path().join("src-tauri")).unwrap();
        std::fs::write(
            temp.path().join("src-tauri/tauri.conf.json"),
            r#"{"version": "1.2.3", "build": {}}"#,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("src-tauri/Cargo.toml"),
            r#"
[package]
name = "test-app"
version = "1.2.3"
edition = "2021"

[dependencies]
tauri = "2"
"#,
        )
        .unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version.version, "1.2.3");
    }

    #[test]
    fn test_version_update() {
        let adapter = TauriAdapter::new();
        let temp = TempDir::new().unwrap();

        // Create Tauri project
        std::fs::create_dir_all(temp.path().join("src-tauri")).unwrap();
        std::fs::write(
            temp.path().join("src-tauri/tauri.conf.json"),
            r#"{"version": "1.0.0", "build": {}}"#,
        )
        .unwrap();
        std::fs::write(
            temp.path().join("src-tauri/Cargo.toml"),
            r#"
[package]
name = "test-app"
version = "1.0.0"
edition = "2021"
"#,
        )
        .unwrap();

        // Update version
        let new_version = VersionInfo {
            version: "2.0.0".to_string(),
            ..Default::default()
        };
        adapter.set_version(temp.path(), &new_version).unwrap();

        // Verify updates
        let cargo_content =
            std::fs::read_to_string(temp.path().join("src-tauri/Cargo.toml")).unwrap();
        assert!(cargo_content.contains(r#"version = "2.0.0""#));

        let conf_content =
            std::fs::read_to_string(temp.path().join("src-tauri/tauri.conf.json")).unwrap();
        assert!(conf_content.contains(r#""version": "2.0.0""#));
    }

    #[test]
    fn test_package_manager_detection() {
        let adapter = TauriAdapter::new();
        let temp = TempDir::new().unwrap();

        // Default to npm
        assert_eq!(adapter.detect_package_manager(temp.path()), "npm");

        // pnpm
        std::fs::write(temp.path().join("pnpm-lock.yaml"), "").unwrap();
        assert_eq!(adapter.detect_package_manager(temp.path()), "pnpm");
        std::fs::remove_file(temp.path().join("pnpm-lock.yaml")).unwrap();

        // yarn
        std::fs::write(temp.path().join("yarn.lock"), "").unwrap();
        assert_eq!(adapter.detect_package_manager(temp.path()), "yarn");
    }

    #[test]
    fn test_supported_platforms() {
        let adapter = TauriAdapter::new();
        let platforms = adapter.supported_platforms();

        assert!(platforms.contains(&Platform::MacOs));
        assert!(platforms.contains(&Platform::Windows));
        assert!(platforms.contains(&Platform::Linux));
        assert!(!platforms.contains(&Platform::Ios));
        assert!(!platforms.contains(&Platform::Android));
    }
}
