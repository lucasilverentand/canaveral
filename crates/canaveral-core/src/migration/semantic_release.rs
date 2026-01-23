//! semantic-release migration

use std::path::Path;

use serde::Deserialize;

use crate::config::Config;
use crate::error::{CanaveralError, Result};

use super::{MigrationResult, MigrationSource, Migrator};

/// semantic-release configuration structure
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct SemanticReleaseConfig {
    branches: Option<Vec<BranchConfig>>,
    plugins: Option<Vec<serde_json::Value>>,
    tag_format: Option<String>,
    preset: Option<String>,
    repository_url: Option<String>,
    dry_run: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BranchConfig {
    Simple(String),
    Complex {
        name: String,
        #[serde(default)]
        prerelease: Option<String>,
        #[serde(default)]
        channel: Option<String>,
    },
}

/// Migrator for semantic-release
#[derive(Debug, Clone, Default)]
pub struct SemanticReleaseMigrator;

impl SemanticReleaseMigrator {
    /// Create a new migrator
    pub fn new() -> Self {
        Self
    }

    /// Parse semantic-release configuration
    fn parse_config(&self, path: &Path) -> Result<SemanticReleaseConfig> {
        // Try various config file locations
        let config_files = [
            ".releaserc",
            ".releaserc.json",
            ".releaserc.yaml",
            ".releaserc.yml",
        ];

        for filename in config_files {
            let config_path = path.join(filename);
            if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;

                // Try JSON first
                if let Ok(config) = serde_json::from_str(&content) {
                    return Ok(config);
                }

                // Try YAML
                if let Ok(config) = serde_yaml::from_str(&content) {
                    return Ok(config);
                }
            }
        }

        // Check package.json for "release" key
        let package_json = path.join("package.json");
        if package_json.exists() {
            let content = std::fs::read_to_string(&package_json)?;
            let package: serde_json::Value = serde_json::from_str(&content)?;

            if let Some(release) = package.get("release") {
                return serde_json::from_value(release.clone())
                    .map_err(|e| CanaveralError::other(e.to_string()));
            }
        }

        Ok(SemanticReleaseConfig::default())
    }

    /// Convert branches to release branches
    fn convert_branches(&self, branches: &[BranchConfig]) -> Vec<String> {
        branches
            .iter()
            .map(|b| match b {
                BranchConfig::Simple(name) => name.clone(),
                BranchConfig::Complex { name, .. } => name.clone(),
            })
            .collect()
    }

    /// Analyze plugins and extract relevant configuration
    fn analyze_plugins(
        &self,
        plugins: &[serde_json::Value],
        result: &mut MigrationResult,
    ) -> (bool, bool, bool) {
        let mut has_changelog = false;
        let mut has_npm = false;
        let mut has_github = false;

        for plugin in plugins {
            let plugin_name = match plugin {
                serde_json::Value::String(s) => s.as_str(),
                serde_json::Value::Array(arr) if !arr.is_empty() => {
                    arr[0].as_str().unwrap_or("")
                }
                _ => continue,
            };

            match plugin_name {
                "@semantic-release/changelog" => has_changelog = true,
                "@semantic-release/npm" => has_npm = true,
                "@semantic-release/github" => has_github = true,
                "@semantic-release/git" => {
                    // This is handled automatically by canaveral
                }
                "@semantic-release/commit-analyzer" => {
                    // This is the default behavior
                }
                "@semantic-release/release-notes-generator" => {
                    // This is handled by changelog generation
                }
                "@semantic-release/exec" => {
                    result.warn(
                        "exec plugin detected - use canaveral hooks for custom commands",
                    );
                    result.manual_step(
                        "Convert @semantic-release/exec commands to canaveral hooks",
                    );
                }
                name if name.starts_with("@semantic-release/") => {
                    result.unsupported(format!("Plugin {} has no direct equivalent", name));
                }
                name => {
                    result.unsupported(format!("Custom plugin {} not supported", name));
                }
            }
        }

        (has_changelog, has_npm, has_github)
    }
}

impl Migrator for SemanticReleaseMigrator {
    fn source(&self) -> MigrationSource {
        MigrationSource::SemanticRelease
    }

    fn can_migrate(&self, path: &Path) -> bool {
        self.detect_config(path).is_some()
    }

    fn detect_config(&self, path: &Path) -> Option<std::path::PathBuf> {
        for filename in MigrationSource::SemanticRelease.config_files() {
            let config_path = path.join(filename);
            if config_path.exists() {
                return Some(config_path);
            }
        }

        // Check package.json
        let package_json = path.join("package.json");
        if package_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json) {
                if content.contains("\"release\"") {
                    return Some(package_json);
                }
            }
        }

        None
    }

    fn migrate(&self, path: &Path) -> Result<MigrationResult> {
        let sr_config = self.parse_config(path)?;
        let mut config = Config::default();
        let mut result = MigrationResult::new(MigrationSource::SemanticRelease, config.clone());

        // Convert branches
        if let Some(ref branches) = sr_config.branches {
            let release_branches = self.convert_branches(branches);
            if let Some(first_branch) = release_branches.first() {
                config.git.branch = first_branch.clone();
            }

            // Check for prerelease branches
            for branch in branches {
                if let BranchConfig::Complex {
                    name,
                    prerelease: Some(pre),
                    ..
                } = branch
                {
                    result.warn(format!(
                        "Branch '{}' with prerelease '{}' - configure prerelease in canaveral manually",
                        name, pre
                    ));
                }
            }

            if release_branches.len() > 1 {
                result.warn(format!(
                    "Multiple release branches detected: {:?} - only first branch '{}' configured",
                    release_branches, config.git.branch
                ));
            }
        }

        // Convert tag format
        if let Some(ref tag_format) = sr_config.tag_format {
            // semantic-release uses ${version}, canaveral uses {version}
            config.versioning.tag_format = tag_format.replace("${version}", "{version}");
        }

        // Convert repository URL
        if let Some(ref repo_url) = sr_config.repository_url {
            result.warn(format!(
                "Repository URL '{}' detected - set this manually in changelog config if needed",
                repo_url
            ));
        }

        // Analyze plugins
        if let Some(ref plugins) = sr_config.plugins {
            let (has_changelog, has_npm, _has_github) =
                self.analyze_plugins(plugins, &mut result);

            config.changelog.enabled = has_changelog;

            if has_npm {
                // npm publishing is configured
                result.manual_step("Configure npm registry in publish.registries if needed");
            }
        }

        // Set dry run if configured
        if sr_config.dry_run == Some(true) {
            config.publish.dry_run = true;
        }

        // Common manual steps
        result.manual_step("Review generated .canaveral.yaml configuration");
        result.manual_step("Remove old semantic-release configuration files");
        result.manual_step("Update CI/CD pipeline to use canaveral");

        result.config = config;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_releaserc() {
        let temp = TempDir::new().unwrap();
        let migrator = SemanticReleaseMigrator::new();

        // No config
        assert!(!migrator.can_migrate(temp.path()));

        // With .releaserc
        std::fs::write(temp.path().join(".releaserc"), "{}").unwrap();
        assert!(migrator.can_migrate(temp.path()));
    }

    #[test]
    fn test_detect_package_json() {
        let temp = TempDir::new().unwrap();
        let migrator = SemanticReleaseMigrator::new();

        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "release": {}}"#,
        )
        .unwrap();

        assert!(migrator.can_migrate(temp.path()));
    }

    #[test]
    fn test_migrate_simple() {
        let temp = TempDir::new().unwrap();
        let migrator = SemanticReleaseMigrator::new();

        std::fs::write(
            temp.path().join(".releaserc.json"),
            r#"{
                "branches": ["main", "next"],
                "tagFormat": "v${version}",
                "plugins": [
                    "@semantic-release/commit-analyzer",
                    "@semantic-release/changelog",
                    "@semantic-release/npm"
                ]
            }"#,
        )
        .unwrap();

        let result = migrator.migrate(temp.path()).unwrap();

        assert_eq!(result.source, MigrationSource::SemanticRelease);
        assert_eq!(result.config.git.branch, "main");
        assert_eq!(result.config.versioning.tag_format, "v{version}");
        assert!(result.config.changelog.enabled);
    }

    #[test]
    fn test_convert_branches() {
        let migrator = SemanticReleaseMigrator::new();

        let branches = vec![
            BranchConfig::Simple("main".to_string()),
            BranchConfig::Complex {
                name: "next".to_string(),
                prerelease: Some("beta".to_string()),
                channel: None,
            },
        ];

        let converted = migrator.convert_branches(&branches);
        assert_eq!(converted, vec!["main", "next"]);
    }
}
