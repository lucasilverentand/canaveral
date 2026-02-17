//! CI/CD Templates - Generate configuration files for CI/CD platforms
//!
//! Provides templates for:
//! - GitHub Actions
//! - GitLab CI
//! - CircleCI
//! - Azure Pipelines

use std::path::Path;
use std::str::FromStr;

use crate::error::Result;

mod github;
mod gitlab;
mod registry;

pub use github::GitHubActionsTemplate;
pub use gitlab::GitLabCITemplate;
pub use registry::CITemplateRegistry;

/// Template generation options
#[derive(Debug, Clone, Default)]
pub struct TemplateOptions {
    /// Project name
    pub project_name: Option<String>,
    /// Package manager type
    pub package_type: Option<String>,
    /// JavaScript package manager (npm, pnpm, yarn, bun)
    pub package_manager: Option<String>,
    /// Default branch name
    pub default_branch: String,
    /// Release branches (patterns)
    pub release_branches: Vec<String>,
    /// Whether to include PR validation
    pub include_pr_checks: bool,
    /// Whether to include automatic releases
    pub include_auto_release: bool,
    /// Whether to include changelogs
    pub include_changelog: bool,
    /// Whether to publish to registries
    pub include_publish: bool,
    /// Registry URL (if custom)
    pub registry_url: Option<String>,
    /// Node.js version (for npm projects)
    pub node_version: Option<String>,
    /// Rust version (for Cargo projects)
    pub rust_version: Option<String>,
    /// Python version (for Python projects)
    pub python_version: Option<String>,
    /// Go version (for Go projects)
    pub go_version: Option<String>,
    /// Java version (for Maven projects)
    pub java_version: Option<String>,
}

impl TemplateOptions {
    /// Create new template options with defaults
    pub fn new() -> Self {
        Self {
            default_branch: "main".to_string(),
            release_branches: vec!["main".to_string()],
            include_pr_checks: true,
            include_auto_release: true,
            include_changelog: true,
            include_publish: true,
            ..Default::default()
        }
    }

    /// Set the project name
    pub fn with_project_name(mut self, name: impl Into<String>) -> Self {
        self.project_name = Some(name.into());
        self
    }

    /// Set the package type
    pub fn with_package_type(mut self, package_type: impl Into<String>) -> Self {
        self.package_type = Some(package_type.into());
        self
    }

    /// Set the package manager
    pub fn with_package_manager(mut self, package_manager: impl Into<String>) -> Self {
        self.package_manager = Some(package_manager.into());
        self
    }

    /// Set the default branch
    pub fn with_default_branch(mut self, branch: impl Into<String>) -> Self {
        self.default_branch = branch.into();
        self
    }

    /// Set release branches
    pub fn with_release_branches(mut self, branches: Vec<String>) -> Self {
        self.release_branches = branches;
        self
    }
}

/// CI/CD template generator trait
pub trait CITemplate: Send + Sync {
    /// Get the CI platform name
    fn platform_name(&self) -> &'static str;

    /// Get the configuration file path (relative to repo root)
    fn config_path(&self) -> &'static str;

    /// Generate the template content
    fn generate(&self, options: &TemplateOptions) -> Result<String>;

    /// Write the template to a file
    fn write_to(&self, base_dir: &Path, options: &TemplateOptions) -> Result<()> {
        let content = self.generate(options)?;
        let path = base_dir.join(self.config_path());

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        std::fs::write(&path, content)?;
        Ok(())
    }
}

/// Detect package type from a directory
pub fn detect_package_type(path: &Path) -> Option<String> {
    if path.join("package.json").exists() {
        Some("npm".to_string())
    } else if path.join("Cargo.toml").exists() {
        Some("cargo".to_string())
    } else if path.join("pyproject.toml").exists() || path.join("setup.py").exists() {
        Some("python".to_string())
    } else if path.join("go.mod").exists() {
        Some("go".to_string())
    } else if path.join("pom.xml").exists() {
        Some("maven".to_string())
    } else if path.join("Dockerfile").exists() {
        Some("docker".to_string())
    } else {
        None
    }
}

/// Detect JavaScript package manager from package metadata and lockfiles
pub fn detect_package_manager(path: &Path) -> Option<String> {
    let package_json_path = path.join("package.json");
    if !package_json_path.exists() {
        return None;
    }

    if let Ok(content) = std::fs::read_to_string(&package_json_path) {
        if let Ok(value) = serde_json::Value::from_str(&content) {
            if let Some(manager) = value
                .get("packageManager")
                .and_then(|v| v.as_str())
                .and_then(|raw| raw.split('@').next())
            {
                return Some(manager.to_string());
            }
        }
    }

    if path.join("pnpm-lock.yaml").exists() {
        return Some("pnpm".to_string());
    }
    if path.join("yarn.lock").exists() {
        return Some("yarn".to_string());
    }
    if path.join("bun.lockb").exists() || path.join("bun.lock").exists() {
        return Some("bun".to_string());
    }

    Some("npm".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_template_options_defaults() {
        let opts = TemplateOptions::new();
        assert_eq!(opts.default_branch, "main");
        assert!(opts.include_pr_checks);
        assert!(opts.include_auto_release);
    }

    #[test]
    fn test_template_options_builder() {
        let opts = TemplateOptions::new()
            .with_project_name("my-project")
            .with_package_type("npm")
            .with_default_branch("master");

        assert_eq!(opts.project_name, Some("my-project".to_string()));
        assert_eq!(opts.package_type, Some("npm".to_string()));
        assert_eq!(opts.default_branch, "master");
    }

    #[test]
    fn test_detect_package_type() {
        let temp = TempDir::new().unwrap();

        // No package files
        assert!(detect_package_type(temp.path()).is_none());

        // npm
        std::fs::write(temp.path().join("package.json"), "{}").unwrap();
        assert_eq!(detect_package_type(temp.path()), Some("npm".to_string()));
    }
}
