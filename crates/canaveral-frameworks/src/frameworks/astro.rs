//! Astro web framework adapter
//!
//! Supports building Astro-based static sites and web applications.

use std::path::Path;
use std::process::Command;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile};
use crate::detection::{file_exists, has_npm_dependency, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{BuildAdapter, Platform, PrerequisiteStatus, ToolStatus, VersionInfo};

/// Astro build adapter
pub struct AstroAdapter {
    /// Package manager to use (npm, pnpm, yarn, bun)
    package_manager: Option<PackageManager>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PackageManager {
    Npm,
    Pnpm,
    Yarn,
    Bun,
}

impl PackageManager {
    fn command(&self) -> &'static str {
        match self {
            PackageManager::Npm => "npm",
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
        }
    }

    fn run_args(&self) -> Vec<&'static str> {
        match self {
            PackageManager::Npm => vec!["run"],
            PackageManager::Pnpm => vec!["run"],
            PackageManager::Yarn => vec!["run"],
            PackageManager::Bun => vec!["run"],
        }
    }
}

impl AstroAdapter {
    pub fn new() -> Self {
        Self {
            package_manager: None,
        }
    }

    /// Detect which package manager is being used
    fn detect_package_manager(&self, path: &Path) -> PackageManager {
        if let Some(pm) = self.package_manager {
            return pm;
        }

        // Check for lockfiles to determine package manager
        if path.join("pnpm-lock.yaml").exists() {
            PackageManager::Pnpm
        } else if path.join("yarn.lock").exists() {
            PackageManager::Yarn
        } else if path.join("bun.lockb").exists() {
            PackageManager::Bun
        } else {
            // Default to npm
            PackageManager::Npm
        }
    }

    fn run_package_manager(
        &self,
        args: &[&str],
        path: &Path,
    ) -> Result<std::process::Output> {
        let pm = self.detect_package_manager(path);
        let mut full_args = pm.run_args();
        full_args.extend_from_slice(args);

        let output = Command::new(pm.command())
            .args(&full_args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("{} {}", pm.command(), full_args.join(" ")),
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

        let pkg: PackageJson = serde_json::from_str(&content).map_err(|e| {
            FrameworkError::Context {
                context: "parsing package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        Ok(pkg)
    }

    fn write_package_json(&self, path: &Path, pkg: &PackageJson) -> Result<()> {
        let package_json_path = path.join("package.json");
        let content = serde_json::to_string_pretty(pkg).map_err(|e| {
            FrameworkError::Context {
                context: "serializing package.json".to_string(),
                message: e.to_string(),
            }
        })?;

        std::fs::write(&package_json_path, content).map_err(|e| FrameworkError::Context {
            context: "writing package.json".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }

    fn get_dist_directory(&self, path: &Path) -> std::path::PathBuf {
        // Default Astro output is dist/
        // Could be customized in astro.config, but we'll use the default
        path.join("dist")
    }
}

impl Default for AstroAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuildAdapter for AstroAdapter {
    fn id(&self) -> &'static str {
        "astro"
    }

    fn name(&self) -> &'static str {
        "Astro"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Check for astro.config.* files (prioritize .mjs as it's most common for Astro)
        let has_astro_config = file_exists(path, "astro.config.mjs")
            || file_exists(path, "astro.config.js")
            || file_exists(path, "astro.config.ts")
            || file_exists(path, "astro.config.mts");

        // Check for astro dependency in package.json
        let has_astro_dep = has_npm_dependency(path, "astro");

        if has_astro_config && has_astro_dep {
            Detection::Yes(95)
        } else if has_astro_config || has_astro_dep {
            Detection::Yes(70)
        } else {
            Detection::No
        }
    }

    fn capabilities(&self) -> Capabilities {
        use crate::capabilities::Capability;

        Capabilities::from_list(&[
            Capability::BuildWeb,
            Capability::HotReload,
            Capability::DebugBuild,
            Capability::ReleaseBuild,
        ])
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Web]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        // Check Node.js
        match Command::new("node").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                status = status.with_tool(ToolStatus::found("node", Some(version)));
            }
            _ => {
                status = status.with_tool(ToolStatus::missing(
                    "node",
                    "Install Node.js from https://nodejs.org",
                ));
            }
        }

        // Check npm (or package manager)
        match Command::new("npm").arg("--version").output() {
            Ok(output) if output.status.success() => {
                let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                status = status.with_tool(ToolStatus::found("npm", Some(version)));
            }
            _ => {
                status = status.with_tool(ToolStatus::missing(
                    "npm",
                    "npm comes with Node.js",
                ));
            }
        }

        Ok(status)
    }

    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        // Astro only supports web platform
        if ctx.platform != Platform::Web {
            return Err(FrameworkError::UnsupportedPlatform {
                platform: ctx.platform.as_str().to_string(),
                framework: "astro".to_string(),
            });
        }

        // Determine build command based on profile
        let build_cmd = match ctx.profile {
            BuildProfile::Debug => "build",      // Astro doesn't have separate debug build
            BuildProfile::Release => "build",
            BuildProfile::Profile => "build",
        };

        // Run build
        let output = self.run_package_manager(&[build_cmd], &ctx.path)?;

        if !output.status.success() {
            return Err(FrameworkError::BuildFailed {
                platform: "web".to_string(),
                message: String::from_utf8_lossy(&output.stderr).to_string(),
                source: None,
            });
        }

        // Get package.json for metadata
        let pkg = self.parse_package_json(&ctx.path)?;
        let dist_dir = self.get_dist_directory(&ctx.path);

        if !dist_dir.exists() {
            return Err(FrameworkError::BuildFailed {
                platform: "web".to_string(),
                message: "Build output directory (dist/) not found after build".to_string(),
                source: None,
            });
        }

        // Calculate directory size
        let size = calculate_dir_size(&dist_dir)?;

        // Create artifact metadata
        let metadata = ArtifactMetadata::new()
            .with_identifier(&pkg.name)
            .with_version(&pkg.version)
            .with_build_number(0);

        // Create artifact
        let artifact = Artifact {
            path: dist_dir.clone(),
            kind: ArtifactKind::WebBuild,
            platform: Platform::Web,
            size,
            sha256: None,
            metadata,
        };

        Ok(vec![artifact])
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        let dist_dir = self.get_dist_directory(path);

        if dist_dir.exists() {
            std::fs::remove_dir_all(&dist_dir).map_err(|e| FrameworkError::Context {
                context: "removing dist directory".to_string(),
                message: e.to_string(),
            })?;
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let pkg = self.parse_package_json(path)?;

        Ok(VersionInfo {
            version: pkg.version.clone(),
            build_number: None,
            build_name: None,
            version_code: None,
            marketing_version: None,
            platform_metadata: std::collections::HashMap::new(),
        })
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let mut pkg = self.parse_package_json(path)?;
        pkg.version = version.version.clone();
        self.write_package_json(path, &pkg)?;
        Ok(())
    }
}

// Helper types

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PackageJson {
    name: String,
    version: String,
    #[serde(default)]
    dependencies: std::collections::HashMap<String, String>,
    #[serde(default, rename = "devDependencies")]
    dev_dependencies: std::collections::HashMap<String, String>,
    #[serde(default)]
    scripts: std::collections::HashMap<String, String>,
}

// Helper function to calculate directory size
fn calculate_dir_size(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    if path.is_dir() {
        for entry in std::fs::read_dir(path).map_err(|e| FrameworkError::Context {
            context: format!("reading directory: {}", path.display()),
            message: e.to_string(),
        })? {
            let entry = entry.map_err(|e| FrameworkError::Context {
                context: "reading directory entry".to_string(),
                message: e.to_string(),
            })?;
            let path = entry.path();

            if path.is_file() {
                total_size += path.metadata().map(|m| m.len()).unwrap_or(0);
            } else if path.is_dir() {
                total_size += calculate_dir_size(&path)?;
            }
        }
    }

    Ok(total_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_id() {
        let adapter = AstroAdapter::new();
        assert_eq!(adapter.id(), "astro");
        assert_eq!(adapter.name(), "Astro");
    }

    #[test]
    fn test_supported_platforms() {
        let adapter = AstroAdapter::new();
        let platforms = adapter.supported_platforms();
        assert_eq!(platforms, &[Platform::Web]);
    }

    #[test]
    fn test_capabilities() {
        use crate::capabilities::Capability;

        let adapter = AstroAdapter::new();
        let caps = adapter.capabilities();
        assert!(caps.has(Capability::BuildWeb));
        assert!(caps.has(Capability::HotReload));
        assert!(caps.has(Capability::DebugBuild));
        assert!(caps.has(Capability::ReleaseBuild));
    }

    #[test]
    fn test_package_manager_command() {
        assert_eq!(PackageManager::Npm.command(), "npm");
        assert_eq!(PackageManager::Pnpm.command(), "pnpm");
        assert_eq!(PackageManager::Yarn.command(), "yarn");
        assert_eq!(PackageManager::Bun.command(), "bun");
    }
}
