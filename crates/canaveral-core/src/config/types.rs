//! Configuration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration for Canaveral
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Version of the config schema
    #[serde(rename = "$schema")]
    pub schema: Option<String>,

    /// Project name
    pub name: Option<String>,

    /// Versioning configuration
    pub versioning: VersioningConfig,

    /// Git configuration
    pub git: GitConfig,

    /// Changelog configuration
    pub changelog: ChangelogConfig,

    /// Package configurations
    #[serde(default)]
    pub packages: Vec<PackageConfig>,

    /// Hooks configuration
    #[serde(default)]
    pub hooks: HooksConfig,

    /// Publishing configuration
    pub publish: PublishConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: None,
            name: None,
            versioning: VersioningConfig::default(),
            git: GitConfig::default(),
            changelog: ChangelogConfig::default(),
            packages: Vec::new(),
            hooks: HooksConfig::default(),
            publish: PublishConfig::default(),
        }
    }
}

/// Versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VersioningConfig {
    /// Version strategy (semver, calver, etc.)
    pub strategy: String,

    /// Tag format (e.g., "v{version}")
    pub tag_format: String,

    /// Whether to use independent versioning in monorepos
    pub independent: bool,

    /// Pre-release identifier
    pub prerelease_identifier: Option<String>,

    /// Build metadata
    pub build_metadata: Option<String>,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            strategy: "semver".to_string(),
            tag_format: "v{version}".to_string(),
            independent: false,
            prerelease_identifier: None,
            build_metadata: None,
        }
    }
}

/// Git configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Remote name
    pub remote: String,

    /// Branch to release from
    pub branch: String,

    /// Whether to require clean working directory
    pub require_clean: bool,

    /// Whether to push tags
    pub push_tags: bool,

    /// Whether to push commits
    pub push_commits: bool,

    /// Commit message template
    pub commit_message: String,

    /// Whether to sign commits
    pub sign_commits: bool,

    /// Whether to sign tags
    pub sign_tags: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            require_clean: true,
            push_tags: true,
            push_commits: true,
            commit_message: "chore(release): {version}".to_string(),
            sign_commits: false,
            sign_tags: false,
        }
    }
}

/// Changelog configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangelogConfig {
    /// Whether to generate changelog
    pub enabled: bool,

    /// Changelog file path
    pub file: PathBuf,

    /// Changelog format (markdown, etc.)
    pub format: String,

    /// Commit types to include
    #[serde(default)]
    pub types: HashMap<String, CommitTypeConfig>,

    /// Header template
    pub header: Option<String>,

    /// Whether to include commit hashes
    pub include_hashes: bool,

    /// Whether to include authors
    pub include_authors: bool,

    /// Whether to include dates
    pub include_dates: bool,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        let mut types = HashMap::new();
        types.insert(
            "feat".to_string(),
            CommitTypeConfig {
                section: "Features".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "fix".to_string(),
            CommitTypeConfig {
                section: "Bug Fixes".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "docs".to_string(),
            CommitTypeConfig {
                section: "Documentation".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "perf".to_string(),
            CommitTypeConfig {
                section: "Performance".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "refactor".to_string(),
            CommitTypeConfig {
                section: "Refactoring".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "test".to_string(),
            CommitTypeConfig {
                section: "Tests".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "chore".to_string(),
            CommitTypeConfig {
                section: "Chores".to_string(),
                hidden: true,
            },
        );

        Self {
            enabled: true,
            file: PathBuf::from("CHANGELOG.md"),
            format: "markdown".to_string(),
            types,
            header: None,
            include_hashes: true,
            include_authors: false,
            include_dates: true,
        }
    }
}

/// Configuration for a commit type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitTypeConfig {
    /// Section header in changelog
    pub section: String,
    /// Whether to hide this type from changelog
    pub hidden: bool,
}

/// Package-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    /// Package name
    pub name: String,

    /// Path to package (relative to repo root)
    pub path: PathBuf,

    /// Package type (npm, cargo, python, etc.)
    #[serde(rename = "type")]
    pub package_type: String,

    /// Whether to publish this package
    #[serde(default = "default_true")]
    pub publish: bool,

    /// Custom registry URL
    pub registry: Option<String>,

    /// Package-specific tag format
    pub tag_format: Option<String>,

    /// Files to update with version
    #[serde(default)]
    pub version_files: Vec<PathBuf>,
}

fn default_true() -> bool {
    true
}

/// Hooks configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    /// Commands to run before version bump
    #[serde(default)]
    pub pre_version: Vec<String>,

    /// Commands to run after version bump
    #[serde(default)]
    pub post_version: Vec<String>,

    /// Commands to run before changelog generation
    #[serde(default)]
    pub pre_changelog: Vec<String>,

    /// Commands to run after changelog generation
    #[serde(default)]
    pub post_changelog: Vec<String>,

    /// Commands to run before publishing
    #[serde(default)]
    pub pre_publish: Vec<String>,

    /// Commands to run after publishing
    #[serde(default)]
    pub post_publish: Vec<String>,

    /// Commands to run before git operations
    #[serde(default)]
    pub pre_git: Vec<String>,

    /// Commands to run after git operations
    #[serde(default)]
    pub post_git: Vec<String>,
}

/// Publishing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PublishConfig {
    /// Whether to publish packages
    pub enabled: bool,

    /// Registry configurations
    #[serde(default)]
    pub registries: HashMap<String, RegistryConfig>,

    /// Dry run mode
    pub dry_run: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            registries: HashMap::new(),
            dry_run: false,
        }
    }
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry URL
    pub url: String,

    /// Authentication token environment variable
    pub token_env: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.versioning.strategy, "semver");
        assert_eq!(config.git.remote, "origin");
        assert!(config.changelog.enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("strategy: semver"));
    }
}
