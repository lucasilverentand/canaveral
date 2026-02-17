//! Package discovery in monorepos

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use glob::glob;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;
use crate::types::PackageInfo;

use super::workspace::{Workspace, WorkspaceType};

/// A discovered package in the workspace
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredPackage {
    /// Package name
    pub name: String,
    /// Package version
    pub version: String,
    /// Path to the package directory
    pub path: PathBuf,
    /// Path to the manifest file
    pub manifest_path: PathBuf,
    /// Package type (npm, cargo, python)
    pub package_type: String,
    /// Whether this is a private package
    pub private: bool,
    /// Dependencies on other packages in the workspace
    pub workspace_dependencies: Vec<String>,
}

impl DiscoveredPackage {
    /// Create from PackageInfo
    pub fn from_info(info: PackageInfo, workspace_deps: Vec<String>) -> Self {
        Self {
            name: info.name,
            version: info.version,
            path: info
                .manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf(),
            manifest_path: info.manifest_path,
            package_type: info.package_type,
            private: info.private,
            workspace_dependencies: workspace_deps,
        }
    }
}

/// Package discovery for workspaces
pub struct PackageDiscovery {
    workspace: Workspace,
}

impl PackageDiscovery {
    /// Create a new package discovery instance
    pub fn new(workspace: Workspace) -> Self {
        Self { workspace }
    }

    /// Discover all packages in the workspace
    pub fn discover(&self) -> Result<Vec<DiscoveredPackage>> {
        debug!(
            workspace_type = %self.workspace.workspace_type,
            patterns = self.workspace.package_patterns.len(),
            "discovering packages"
        );
        let mut packages = Vec::new();
        let mut package_names: HashMap<PathBuf, String> = HashMap::new();

        // First pass: discover all packages
        for pattern in &self.workspace.package_patterns {
            let full_pattern = if pattern == "." {
                self.workspace.root.to_string_lossy().to_string()
            } else {
                self.workspace
                    .root
                    .join(pattern)
                    .to_string_lossy()
                    .to_string()
            };

            // Get the manifest file name based on workspace type
            let manifest_name = self.manifest_name();

            for entry in
                glob(&full_pattern).map_err(|e| crate::error::ConfigError::InvalidValue {
                    field: "package_patterns".to_string(),
                    message: e.to_string(),
                })?
            {
                let path = entry.map_err(|e| crate::error::ConfigError::InvalidValue {
                    field: "package_patterns".to_string(),
                    message: e.to_string(),
                })?;

                let manifest_path = if path.is_dir() {
                    path.join(manifest_name)
                } else if path
                    .file_name()
                    .map(|f| f == manifest_name)
                    .unwrap_or(false)
                {
                    path.clone()
                } else {
                    continue;
                };

                if manifest_path.exists() {
                    if let Some(pkg) = self.parse_package(&manifest_path)? {
                        package_names.insert(pkg.path.clone(), pkg.name.clone());
                        packages.push(pkg);
                    }
                }
            }
        }

        // Second pass: resolve workspace dependencies
        let all_names: Vec<String> = package_names.values().cloned().collect();
        for pkg in &mut packages {
            pkg.workspace_dependencies = self.find_workspace_deps(pkg, &all_names)?;
        }

        info!(count = packages.len(), "discovered packages");
        Ok(packages)
    }

    /// Get the manifest file name for this workspace type
    fn manifest_name(&self) -> &'static str {
        match self.workspace.workspace_type {
            WorkspaceType::Cargo => "Cargo.toml",
            WorkspaceType::Npm
            | WorkspaceType::Yarn
            | WorkspaceType::Pnpm
            | WorkspaceType::Lerna
            | WorkspaceType::Turbo
            | WorkspaceType::Nx => "package.json",
            WorkspaceType::Python => "pyproject.toml",
            WorkspaceType::Custom => "canaveral.toml",
        }
    }

    /// Parse a package from its manifest
    fn parse_package(&self, manifest_path: &Path) -> Result<Option<DiscoveredPackage>> {
        match self.workspace.workspace_type {
            WorkspaceType::Cargo => self.parse_cargo_package(manifest_path),
            WorkspaceType::Npm
            | WorkspaceType::Yarn
            | WorkspaceType::Pnpm
            | WorkspaceType::Lerna
            | WorkspaceType::Turbo
            | WorkspaceType::Nx => self.parse_npm_package(manifest_path),
            WorkspaceType::Python => self.parse_python_package(manifest_path),
            WorkspaceType::Custom => Ok(None),
        }
    }

    /// Parse a Cargo package
    fn parse_cargo_package(&self, manifest_path: &Path) -> Result<Option<DiscoveredPackage>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct CargoToml {
            package: Option<PackageSection>,
        }

        #[derive(Deserialize)]
        struct PackageSection {
            name: String,
            version: String,
            publish: Option<bool>,
        }

        let cargo: CargoToml = toml::from_str(&content)?;

        if let Some(package) = cargo.package {
            let path = manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            return Ok(Some(DiscoveredPackage {
                name: package.name,
                version: package.version,
                path,
                manifest_path: manifest_path.to_path_buf(),
                package_type: "cargo".to_string(),
                private: package.publish == Some(false),
                workspace_dependencies: Vec::new(),
            }));
        }

        Ok(None)
    }

    /// Parse an npm package
    fn parse_npm_package(&self, manifest_path: &Path) -> Result<Option<DiscoveredPackage>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct PackageJson {
            name: Option<String>,
            version: Option<String>,
            private: Option<bool>,
        }

        let pkg: PackageJson = serde_json::from_str(&content)?;

        if let (Some(name), Some(version)) = (pkg.name, pkg.version) {
            let path = manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            return Ok(Some(DiscoveredPackage {
                name,
                version,
                path,
                manifest_path: manifest_path.to_path_buf(),
                package_type: "npm".to_string(),
                private: pkg.private.unwrap_or(false),
                workspace_dependencies: Vec::new(),
            }));
        }

        Ok(None)
    }

    /// Parse a Python package
    fn parse_python_package(&self, manifest_path: &Path) -> Result<Option<DiscoveredPackage>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct PyProject {
            project: Option<ProjectSection>,
        }

        #[derive(Deserialize)]
        struct ProjectSection {
            name: String,
            version: Option<String>,
        }

        let pyproj: PyProject = toml::from_str(&content)?;

        if let Some(project) = pyproj.project {
            let version = project.version.unwrap_or_else(|| "0.0.0".to_string());
            let path = manifest_path
                .parent()
                .unwrap_or(Path::new("."))
                .to_path_buf();
            return Ok(Some(DiscoveredPackage {
                name: project.name,
                version,
                path,
                manifest_path: manifest_path.to_path_buf(),
                package_type: "python".to_string(),
                private: false,
                workspace_dependencies: Vec::new(),
            }));
        }

        Ok(None)
    }

    /// Find workspace dependencies for a package
    fn find_workspace_deps(
        &self,
        pkg: &DiscoveredPackage,
        all_names: &[String],
    ) -> Result<Vec<String>> {
        match self.workspace.workspace_type {
            WorkspaceType::Cargo => self.find_cargo_workspace_deps(&pkg.manifest_path, all_names),
            WorkspaceType::Npm
            | WorkspaceType::Yarn
            | WorkspaceType::Pnpm
            | WorkspaceType::Lerna
            | WorkspaceType::Turbo
            | WorkspaceType::Nx => self.find_npm_workspace_deps(&pkg.manifest_path, all_names),
            WorkspaceType::Python => self.find_python_workspace_deps(&pkg.manifest_path, all_names),
            WorkspaceType::Custom => Ok(Vec::new()),
        }
    }

    /// Find Cargo workspace dependencies
    fn find_cargo_workspace_deps(
        &self,
        manifest_path: &Path,
        all_names: &[String],
    ) -> Result<Vec<String>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct CargoToml {
            dependencies: Option<HashMap<String, toml::Value>>,
            #[serde(rename = "dev-dependencies")]
            dev_dependencies: Option<HashMap<String, toml::Value>>,
            #[serde(rename = "build-dependencies")]
            build_dependencies: Option<HashMap<String, toml::Value>>,
        }

        let cargo: CargoToml = toml::from_str(&content)?;

        let mut deps = Vec::new();

        for section in [
            cargo.dependencies,
            cargo.dev_dependencies,
            cargo.build_dependencies,
        ]
        .into_iter()
        .flatten()
        {
            for name in section.keys() {
                if all_names.contains(name) {
                    deps.push(name.clone());
                }
            }
        }

        deps.sort();
        deps.dedup();
        Ok(deps)
    }

    /// Find npm workspace dependencies
    fn find_npm_workspace_deps(
        &self,
        manifest_path: &Path,
        all_names: &[String],
    ) -> Result<Vec<String>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct PackageJson {
            dependencies: Option<HashMap<String, String>>,
            #[serde(rename = "devDependencies")]
            dev_dependencies: Option<HashMap<String, String>>,
            #[serde(rename = "peerDependencies")]
            peer_dependencies: Option<HashMap<String, String>>,
        }

        let pkg: PackageJson = serde_json::from_str(&content)?;

        let mut deps = Vec::new();

        for section in [
            pkg.dependencies,
            pkg.dev_dependencies,
            pkg.peer_dependencies,
        ]
        .into_iter()
        .flatten()
        {
            for name in section.keys() {
                if all_names.contains(name) {
                    deps.push(name.clone());
                }
            }
        }

        deps.sort();
        deps.dedup();
        Ok(deps)
    }

    /// Find Python workspace dependencies
    fn find_python_workspace_deps(
        &self,
        manifest_path: &Path,
        all_names: &[String],
    ) -> Result<Vec<String>> {
        let content = std::fs::read_to_string(manifest_path)?;

        #[derive(Deserialize)]
        struct PyProject {
            project: Option<ProjectSection>,
        }

        #[derive(Deserialize)]
        struct ProjectSection {
            dependencies: Option<Vec<String>>,
        }

        let pyproj: PyProject = toml::from_str(&content)?;

        let mut deps = Vec::new();

        if let Some(project) = pyproj.project {
            if let Some(dependencies) = project.dependencies {
                for dep in dependencies {
                    // Extract package name from dependency specification
                    // e.g., "my-package>=1.0.0" -> "my-package"
                    let name = dep
                        .split(|c: char| !c.is_alphanumeric() && c != '-' && c != '_')
                        .next()
                        .unwrap_or(&dep);

                    if all_names.contains(&name.to_string()) {
                        deps.push(name.to_string());
                    }
                }
            }
        }

        deps.sort();
        deps.dedup();
        Ok(deps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_discover_cargo_packages() {
        let temp = TempDir::new().unwrap();

        // Create workspace
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();

        // Create packages
        std::fs::create_dir_all(temp.path().join("crates/pkg-a")).unwrap();
        std::fs::write(
            temp.path().join("crates/pkg-a/Cargo.toml"),
            r#"
[package]
name = "pkg-a"
version = "1.0.0"
"#,
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("crates/pkg-b")).unwrap();
        std::fs::write(
            temp.path().join("crates/pkg-b/Cargo.toml"),
            r#"
[package]
name = "pkg-b"
version = "2.0.0"

[dependencies]
pkg-a = { path = "../pkg-a" }
"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        let discovery = PackageDiscovery::new(ws);
        let packages = discovery.discover().unwrap();

        assert_eq!(packages.len(), 2);

        let pkg_a = packages.iter().find(|p| p.name == "pkg-a").unwrap();
        let pkg_b = packages.iter().find(|p| p.name == "pkg-b").unwrap();

        assert_eq!(pkg_a.version, "1.0.0");
        assert_eq!(pkg_b.version, "2.0.0");
        assert!(pkg_b.workspace_dependencies.contains(&"pkg-a".to_string()));
    }

    #[test]
    fn test_discover_npm_packages() {
        let temp = TempDir::new().unwrap();

        // Create workspace
        std::fs::write(
            temp.path().join("package.json"),
            r#"{
                "name": "my-monorepo",
                "workspaces": ["packages/*"]
            }"#,
        )
        .unwrap();

        // Create packages
        std::fs::create_dir_all(temp.path().join("packages/core")).unwrap();
        std::fs::write(
            temp.path().join("packages/core/package.json"),
            r#"{"name": "@my/core", "version": "1.0.0"}"#,
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("packages/utils")).unwrap();
        std::fs::write(
            temp.path().join("packages/utils/package.json"),
            r#"{
                "name": "@my/utils",
                "version": "1.0.0",
                "dependencies": {
                    "@my/core": "workspace:*"
                }
            }"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        let discovery = PackageDiscovery::new(ws);
        let packages = discovery.discover().unwrap();

        assert_eq!(packages.len(), 2);

        let utils = packages.iter().find(|p| p.name == "@my/utils").unwrap();
        assert!(utils
            .workspace_dependencies
            .contains(&"@my/core".to_string()));
    }
}
