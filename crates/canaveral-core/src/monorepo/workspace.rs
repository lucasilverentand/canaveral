//! Workspace detection and management

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::error::Result;

/// Type of workspace/monorepo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkspaceType {
    /// Cargo workspace (Rust)
    Cargo,
    /// npm workspaces
    Npm,
    /// pnpm workspace
    Pnpm,
    /// Yarn workspaces (v1 or berry)
    Yarn,
    /// Lerna monorepo
    Lerna,
    /// Nx monorepo
    Nx,
    /// Turborepo
    Turbo,
    /// Python monorepo (using pyproject.toml with tool.poetry or similar)
    Python,
    /// Generic/custom workspace
    Custom,
}

impl WorkspaceType {
    /// Get the configuration file that identifies this workspace type
    pub fn config_file(&self) -> &'static str {
        match self {
            Self::Cargo => "Cargo.toml",
            Self::Npm => "package.json",
            Self::Pnpm => "pnpm-workspace.yaml",
            Self::Yarn => "package.json",
            Self::Lerna => "lerna.json",
            Self::Nx => "nx.json",
            Self::Turbo => "turbo.json",
            Self::Python => "pyproject.toml",
            Self::Custom => "canaveral.yaml",
        }
    }
}

impl std::fmt::Display for WorkspaceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Cargo => write!(f, "cargo"),
            Self::Npm => write!(f, "npm"),
            Self::Pnpm => write!(f, "pnpm"),
            Self::Yarn => write!(f, "yarn"),
            Self::Lerna => write!(f, "lerna"),
            Self::Nx => write!(f, "nx"),
            Self::Turbo => write!(f, "turbo"),
            Self::Python => write!(f, "python"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

/// Represents a detected workspace
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root path of the workspace
    pub root: PathBuf,
    /// Type of workspace
    pub workspace_type: WorkspaceType,
    /// Glob patterns for package locations
    pub package_patterns: Vec<String>,
    /// Whether this is a single-package repo (not really a monorepo)
    pub is_single_package: bool,
}

impl Workspace {
    /// Create a new workspace
    pub fn new(root: PathBuf, workspace_type: WorkspaceType) -> Self {
        Self {
            root,
            workspace_type,
            package_patterns: Vec::new(),
            is_single_package: false,
        }
    }

    /// Detect workspace type and configuration from a directory
    pub fn detect(path: &Path) -> Result<Option<Self>> {
        debug!(path = %path.display(), "detecting workspace type");
        let registry = super::detector::WorkspaceDetectorRegistry::new();
        registry.detect(path)
    }

    /// Detect Cargo workspace
    pub(crate) fn detect_cargo(path: &Path) -> Result<Option<Self>> {
        let cargo_toml = path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&cargo_toml)?;

        #[derive(Deserialize)]
        struct CargoWorkspace {
            workspace: Option<WorkspaceSection>,
            package: Option<serde::de::IgnoredAny>,
        }

        #[derive(Deserialize)]
        struct WorkspaceSection {
            members: Option<Vec<String>>,
            #[allow(dead_code)]
            exclude: Option<Vec<String>>,
        }

        let cargo: CargoWorkspace = toml::from_str(&content).unwrap_or(CargoWorkspace {
            workspace: None,
            package: None,
        });

        if let Some(workspace) = cargo.workspace {
            let patterns = workspace.members.unwrap_or_default();
            let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Cargo);
            ws.package_patterns = patterns;
            ws.is_single_package = false;
            return Ok(Some(ws));
        }

        // Single package, not a workspace
        if cargo.package.is_some() {
            let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Cargo);
            ws.package_patterns = vec![".".to_string()];
            ws.is_single_package = true;
            return Ok(Some(ws));
        }

        Ok(None)
    }

    /// Detect pnpm workspace
    pub(crate) fn detect_pnpm(path: &Path) -> Result<Option<Self>> {
        let pnpm_workspace = path.join("pnpm-workspace.yaml");
        if !pnpm_workspace.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&pnpm_workspace)?;

        #[derive(Deserialize)]
        struct PnpmWorkspace {
            packages: Option<Vec<String>>,
        }

        let config: PnpmWorkspace = serde_yaml::from_str(&content).unwrap_or(PnpmWorkspace {
            packages: None,
        });

        let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Pnpm);
        ws.package_patterns = config.packages.unwrap_or_else(|| vec!["packages/*".to_string()]);
        Ok(Some(ws))
    }

    /// Detect Lerna monorepo
    pub(crate) fn detect_lerna(path: &Path) -> Result<Option<Self>> {
        let lerna_json = path.join("lerna.json");
        if !lerna_json.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&lerna_json)?;

        #[derive(Deserialize)]
        struct LernaConfig {
            packages: Option<Vec<String>>,
        }

        let config: LernaConfig = serde_json::from_str(&content).unwrap_or(LernaConfig {
            packages: None,
        });

        let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Lerna);
        ws.package_patterns = config.packages.unwrap_or_else(|| vec!["packages/*".to_string()]);
        Ok(Some(ws))
    }

    /// Detect Nx monorepo
    pub(crate) fn detect_nx(path: &Path) -> Result<Option<Self>> {
        let nx_json = path.join("nx.json");
        if !nx_json.exists() {
            return Ok(None);
        }

        // Nx uses project.json files or infers from package.json
        let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Nx);
        ws.package_patterns = vec![
            "packages/*".to_string(),
            "apps/*".to_string(),
            "libs/*".to_string(),
        ];
        Ok(Some(ws))
    }

    /// Detect Turborepo
    pub(crate) fn detect_turbo(path: &Path) -> Result<Option<Self>> {
        let turbo_json = path.join("turbo.json");
        if !turbo_json.exists() {
            return Ok(None);
        }

        // Turbo uses npm/yarn/pnpm workspaces for package discovery
        // Check for package.json workspaces
        let package_json = path.join("package.json");
        if package_json.exists() {
            let content = std::fs::read_to_string(&package_json)?;

            #[derive(Deserialize)]
            struct PackageJson {
                workspaces: Option<WorkspacesField>,
            }

            #[derive(Deserialize)]
            #[serde(untagged)]
            enum WorkspacesField {
                Array(Vec<String>),
                Object { packages: Vec<String> },
            }

            let pkg: PackageJson = serde_json::from_str(&content).unwrap_or(PackageJson {
                workspaces: None,
            });

            let patterns = match pkg.workspaces {
                Some(WorkspacesField::Array(arr)) => arr,
                Some(WorkspacesField::Object { packages }) => packages,
                None => vec!["packages/*".to_string(), "apps/*".to_string()],
            };

            let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Turbo);
            ws.package_patterns = patterns;
            return Ok(Some(ws));
        }

        Ok(None)
    }

    /// Detect npm or Yarn workspaces
    pub(crate) fn detect_npm_yarn(path: &Path) -> Result<Option<Self>> {
        let package_json = path.join("package.json");
        if !package_json.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&package_json)?;

        #[derive(Deserialize)]
        struct PackageJson {
            name: Option<String>,
            workspaces: Option<WorkspacesField>,
        }

        #[derive(Deserialize)]
        #[serde(untagged)]
        enum WorkspacesField {
            Array(Vec<String>),
            Object { packages: Vec<String> },
        }

        let pkg: PackageJson = serde_json::from_str(&content).unwrap_or(PackageJson {
            name: None,
            workspaces: None,
        });

        // Determine if Yarn based on yarn.lock
        let is_yarn = path.join("yarn.lock").exists();
        let workspace_type = if is_yarn {
            WorkspaceType::Yarn
        } else {
            WorkspaceType::Npm
        };

        if let Some(workspaces) = pkg.workspaces {
            let patterns = match workspaces {
                WorkspacesField::Array(arr) => arr,
                WorkspacesField::Object { packages } => packages,
            };

            let mut ws = Workspace::new(path.to_path_buf(), workspace_type);
            ws.package_patterns = patterns;
            return Ok(Some(ws));
        }

        // Single package
        if pkg.name.is_some() {
            let mut ws = Workspace::new(path.to_path_buf(), workspace_type);
            ws.package_patterns = vec![".".to_string()];
            ws.is_single_package = true;
            return Ok(Some(ws));
        }

        Ok(None)
    }

    /// Detect Python monorepo
    pub(crate) fn detect_python(path: &Path) -> Result<Option<Self>> {
        let pyproject = path.join("pyproject.toml");
        if !pyproject.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&pyproject)?;

        // Check for various Python monorepo tools
        #[derive(Deserialize)]
        struct PyProject {
            tool: Option<ToolSection>,
            project: Option<serde::de::IgnoredAny>,
        }

        #[derive(Deserialize)]
        struct ToolSection {
            poetry: Option<PoetrySection>,
            hatch: Option<HatchSection>,
        }

        #[derive(Deserialize)]
        struct PoetrySection {
            packages: Option<Vec<PoetryPackage>>,
        }

        #[derive(Deserialize)]
        struct PoetryPackage {
            include: String,
        }

        #[derive(Deserialize)]
        struct HatchSection {
            build: Option<HatchBuild>,
        }

        #[derive(Deserialize)]
        struct HatchBuild {
            packages: Option<Vec<String>>,
        }

        let pyproj: PyProject = toml::from_str(&content).unwrap_or(PyProject {
            tool: None,
            project: None,
        });

        let mut patterns = Vec::new();

        if let Some(tool) = &pyproj.tool {
            if let Some(poetry) = &tool.poetry {
                if let Some(packages) = &poetry.packages {
                    for pkg in packages {
                        patterns.push(pkg.include.clone());
                    }
                }
            }
            if let Some(hatch) = &tool.hatch {
                if let Some(build) = &hatch.build {
                    if let Some(packages) = &build.packages {
                        patterns.extend(packages.clone());
                    }
                }
            }
        }

        if patterns.is_empty() && pyproj.project.is_some() {
            // Single package
            let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Python);
            ws.package_patterns = vec![".".to_string()];
            ws.is_single_package = true;
            return Ok(Some(ws));
        }

        if !patterns.is_empty() {
            let mut ws = Workspace::new(path.to_path_buf(), WorkspaceType::Python);
            ws.package_patterns = patterns;
            return Ok(Some(ws));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_cargo_workspace() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*", "tools/*"]
"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.workspace_type, WorkspaceType::Cargo);
        assert_eq!(ws.package_patterns, vec!["crates/*", "tools/*"]);
        assert!(!ws.is_single_package);
    }

    #[test]
    fn test_detect_npm_workspaces() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{
                "name": "my-monorepo",
                "workspaces": ["packages/*"]
            }"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.workspace_type, WorkspaceType::Npm);
        assert_eq!(ws.package_patterns, vec!["packages/*"]);
    }

    #[test]
    fn test_detect_yarn_workspaces() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{
                "name": "my-monorepo",
                "workspaces": {"packages": ["packages/*", "apps/*"]}
            }"#,
        )
        .unwrap();
        std::fs::write(temp.path().join("yarn.lock"), "").unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.workspace_type, WorkspaceType::Yarn);
        assert_eq!(ws.package_patterns, vec!["packages/*", "apps/*"]);
    }

    #[test]
    fn test_detect_pnpm_workspace() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("pnpm-workspace.yaml"),
            r#"
packages:
  - 'packages/*'
  - 'components/**'
"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert_eq!(ws.workspace_type, WorkspaceType::Pnpm);
        assert_eq!(ws.package_patterns, vec!["packages/*", "components/**"]);
    }

    #[test]
    fn test_detect_single_package() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "single-package", "version": "1.0.0"}"#,
        )
        .unwrap();

        let ws = Workspace::detect(temp.path()).unwrap().unwrap();
        assert!(ws.is_single_package);
    }

    #[test]
    fn test_workspace_type_display() {
        assert_eq!(WorkspaceType::Cargo.to_string(), "cargo");
        assert_eq!(WorkspaceType::Npm.to_string(), "npm");
        assert_eq!(WorkspaceType::Pnpm.to_string(), "pnpm");
    }
}
