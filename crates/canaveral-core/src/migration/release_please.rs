//! release-please migration

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::info;

use std::path::PathBuf;

use crate::config::Config;
use crate::error::{CanaveralError, Result};

use super::{MigrationResult, MigrationSource, Migrator};

/// release-please configuration structure
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct ReleasePleaseConfig {
    #[serde(default)]
    packages: HashMap<String, PackageConfig>,
    release_type: Option<String>,
    bump_minor_pre_major: Option<bool>,
    bump_patch_for_minor_pre_major: Option<bool>,
    changelog_path: Option<String>,
    changelog_sections: Option<Vec<ChangelogSection>>,
    include_component_in_tag: Option<bool>,
    include_v_in_tag: Option<bool>,
    #[allow(dead_code)]
    tag_separator: Option<String>,
    separate_pull_requests: Option<bool>,
    #[allow(dead_code)]
    bootstrap_sha: Option<String>,
    versioning_strategy: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
struct PackageConfig {
    release_type: Option<String>,
    #[allow(dead_code)]
    component: Option<String>,
    #[allow(dead_code)]
    changelog_path: Option<String>,
    #[allow(dead_code)]
    bump_minor_pre_major: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct ChangelogSection {
    #[serde(rename = "type")]
    commit_type: String,
    section: String,
    #[serde(default)]
    hidden: bool,
}

/// release-please manifest structure
#[derive(Debug, Deserialize, Default)]
struct ReleasePleaseManifest {
    #[serde(flatten)]
    packages: HashMap<String, String>,
}

/// Migrator for release-please
#[derive(Debug, Clone, Default)]
pub struct ReleasePleaseMigrator;

impl ReleasePleaseMigrator {
    /// Create a new migrator
    pub fn new() -> Self {
        Self
    }

    /// Parse release-please configuration
    fn parse_config(&self, path: &Path) -> Result<ReleasePleaseConfig> {
        let config_path = path.join("release-please-config.json");
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            return serde_json::from_str(&content)
                .map_err(|e| CanaveralError::other(e.to_string()));
        }

        Ok(ReleasePleaseConfig::default())
    }

    /// Parse release-please manifest
    fn parse_manifest(&self, path: &Path) -> Result<ReleasePleaseManifest> {
        let manifest_path = path.join(".release-please-manifest.json");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            return serde_json::from_str(&content)
                .map_err(|e| CanaveralError::other(e.to_string()));
        }

        Ok(ReleasePleaseManifest::default())
    }

    /// Map release-please release type to canaveral package type
    fn map_release_type(&self, release_type: &str) -> Option<&'static str> {
        match release_type {
            "node" | "npm" => Some("npm"),
            "rust" | "cargo" => Some("cargo"),
            "python" => Some("python"),
            "go" => Some("go"),
            "maven" | "java" => Some("maven"),
            "simple" | "default" => None,
            _ => None,
        }
    }

    /// Convert changelog sections to canaveral format
    fn convert_changelog_sections(
        &self,
        sections: &[ChangelogSection],
        result: &mut MigrationResult,
    ) -> Vec<(String, String)> {
        let mut converted = Vec::new();

        for section in sections {
            if section.hidden {
                result.warn(format!(
                    "Hidden changelog section '{}' - configure exclusions in canaveral",
                    section.commit_type
                ));
                continue;
            }

            converted.push((section.commit_type.clone(), section.section.clone()));
        }

        converted
    }
}

impl Migrator for ReleasePleaseMigrator {
    fn source(&self) -> MigrationSource {
        MigrationSource::ReleasePlease
    }

    fn can_migrate(&self, path: &Path) -> bool {
        self.detect_config(path).is_some()
    }

    fn detect_config(&self, path: &Path) -> Option<std::path::PathBuf> {
        for filename in MigrationSource::ReleasePlease.config_files() {
            let config_path = path.join(filename);
            if config_path.exists() {
                return Some(config_path);
            }
        }

        None
    }

    fn migrate(&self, path: &Path) -> Result<MigrationResult> {
        info!(path = %path.display(), "migrating from release-please");
        let rp_config = self.parse_config(path)?;
        let manifest = self.parse_manifest(path)?;
        let mut config = Config::default();
        let mut result = MigrationResult::new(MigrationSource::ReleasePlease, config.clone());

        // Determine if this is a monorepo
        let is_monorepo = !rp_config.packages.is_empty() || manifest.packages.len() > 1;

        if is_monorepo {
            result.warn("Monorepo detected - configure packages in canaveral");
            result.manual_step(
                "Set up monorepo configuration with package paths and versioning mode",
            );

            // List detected packages
            for (pkg_path, pkg_config) in &rp_config.packages {
                result.manual_step(format!(
                    "Configure package at '{}' (type: {:?})",
                    pkg_path, pkg_config.release_type
                ));
            }
        }

        // Convert release type
        if let Some(ref release_type) = rp_config.release_type {
            if let Some(pkg_type) = self.map_release_type(release_type) {
                result.warn(format!(
                    "Detected package type '{}' from release-type '{}'",
                    pkg_type, release_type
                ));
            }
        }

        // Convert tag format
        let include_v = rp_config.include_v_in_tag.unwrap_or(true);
        config.versioning.tag_format = if include_v {
            "v{version}".to_string()
        } else {
            "{version}".to_string()
        };

        if rp_config.include_component_in_tag == Some(true) {
            result.warn("Component in tag is enabled - configure tag format in canaveral");
            result.manual_step("Set versioning.tag_format to include package name for monorepo");
        }

        // Convert changelog configuration
        config.changelog.enabled = true;
        if let Some(ref changelog_path) = rp_config.changelog_path {
            config.changelog.file = PathBuf::from(changelog_path);
        }

        // Convert changelog sections
        if let Some(ref sections) = rp_config.changelog_sections {
            let converted = self.convert_changelog_sections(sections, &mut result);
            if !converted.is_empty() {
                result.manual_step(
                    "Configure changelog sections in canaveral.yaml changelog.sections",
                );
            }
        }

        // Handle versioning strategy
        if let Some(ref strategy) = rp_config.versioning_strategy {
            match strategy.as_str() {
                "default" | "semver" => {
                    // Default behavior
                }
                "always-bump-patch" => {
                    result.warn(
                        "always-bump-patch strategy detected - this is the default in canaveral",
                    );
                }
                "always-bump-minor" => {
                    result.warn("always-bump-minor strategy - configure bump rules in canaveral");
                    result.manual_step("Set version.default_bump to 'minor' in configuration");
                }
                other => {
                    result.unsupported(format!("Versioning strategy '{}' not supported", other));
                }
            }
        }

        // Handle pre-major bumping options
        if rp_config.bump_minor_pre_major == Some(true) {
            result.warn(
                "bump-minor-pre-major is enabled - breaking changes bump minor before 1.0",
            );
        }
        if rp_config.bump_patch_for_minor_pre_major == Some(true) {
            result.warn(
                "bump-patch-for-minor-pre-major is enabled - features bump patch before 1.0",
            );
        }

        // Handle separate pull requests
        if rp_config.separate_pull_requests == Some(true) {
            result.warn("separate-pull-requests is enabled - canaveral handles releases differently");
            result.manual_step(
                "Consider using monorepo independent versioning for similar behavior",
            );
        }

        // Common manual steps
        result.manual_step("Review generated .canaveral.yaml configuration");
        result.manual_step("Remove release-please configuration files");
        result.manual_step("Update CI/CD pipeline to use canaveral instead of release-please");
        result.manual_step("Remove release-please GitHub App if installed");

        result.config = config;
        info!(
            warnings = result.warnings.len(),
            unsupported = result.unsupported.len(),
            manual_steps = result.manual_steps.len(),
            "release-please migration complete"
        );
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_config() {
        let temp = TempDir::new().unwrap();
        let migrator = ReleasePleaseMigrator::new();

        // No config
        assert!(!migrator.can_migrate(temp.path()));

        // With config
        std::fs::write(temp.path().join("release-please-config.json"), "{}").unwrap();
        assert!(migrator.can_migrate(temp.path()));
    }

    #[test]
    fn test_detect_manifest() {
        let temp = TempDir::new().unwrap();
        let migrator = ReleasePleaseMigrator::new();

        std::fs::write(
            temp.path().join(".release-please-manifest.json"),
            r#"{"packages/a": "1.0.0"}"#,
        )
        .unwrap();

        assert!(migrator.can_migrate(temp.path()));
    }

    #[test]
    fn test_migrate_simple() {
        let temp = TempDir::new().unwrap();
        let migrator = ReleasePleaseMigrator::new();

        std::fs::write(
            temp.path().join("release-please-config.json"),
            r#"{
                "release-type": "node",
                "include-v-in-tag": true,
                "changelog-path": "CHANGELOG.md"
            }"#,
        )
        .unwrap();

        let result = migrator.migrate(temp.path()).unwrap();

        assert_eq!(result.source, MigrationSource::ReleasePlease);
        assert_eq!(result.config.versioning.tag_format, "v{version}");
        assert!(result.config.changelog.enabled);
        assert_eq!(result.config.changelog.file, std::path::PathBuf::from("CHANGELOG.md"));
    }

    #[test]
    fn test_migrate_monorepo() {
        let temp = TempDir::new().unwrap();
        let migrator = ReleasePleaseMigrator::new();

        std::fs::write(
            temp.path().join("release-please-config.json"),
            r#"{
                "packages": {
                    "packages/a": {"release-type": "node"},
                    "packages/b": {"release-type": "rust"}
                }
            }"#,
        )
        .unwrap();

        let result = migrator.migrate(temp.path()).unwrap();

        // Should warn about monorepo
        assert!(result.warnings.iter().any(|w| w.contains("Monorepo")));
    }

    #[test]
    fn test_map_release_type() {
        let migrator = ReleasePleaseMigrator::new();

        assert_eq!(migrator.map_release_type("node"), Some("npm"));
        assert_eq!(migrator.map_release_type("npm"), Some("npm"));
        assert_eq!(migrator.map_release_type("rust"), Some("cargo"));
        assert_eq!(migrator.map_release_type("python"), Some("python"));
        assert_eq!(migrator.map_release_type("go"), Some("go"));
        assert_eq!(migrator.map_release_type("simple"), None);
    }
}
