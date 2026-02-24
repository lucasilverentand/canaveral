//! Root configuration struct

use serde::{Deserialize, Serialize};

use super::changelog::ChangelogConfig;
use super::ci::CIConfig;
use super::git::GitConfig;
use super::hooks_cfg::{GitHooksConfig, HooksConfig};
use super::metadata_cfg::MetadataConfig;
use super::pr::PrConfig;
use super::publishing::PublishConfig;
use super::release_notes::ReleaseNotesConfig;
use super::signing::SigningConfig;
use super::stores::StoresConfig;
use super::tasks::TasksConfig;
use super::tools::ToolsConfig;
use super::versioning::VersioningConfig;

/// Package-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    /// Package name
    pub name: String,

    /// Path to package (relative to repo root)
    pub path: std::path::PathBuf,

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
    pub version_files: Vec<std::path::PathBuf>,
}

fn default_true() -> bool {
    true
}

/// Main configuration for Canaveral
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
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

    /// Code signing configuration
    #[serde(default)]
    pub signing: SigningConfig,

    /// App store configurations
    #[serde(default)]
    pub stores: StoresConfig,

    /// Metadata management configuration
    #[serde(default)]
    pub metadata: MetadataConfig,

    /// Task orchestration configuration
    #[serde(default)]
    pub tasks: TasksConfig,

    /// CI/CD configuration
    #[serde(default)]
    pub ci: CIConfig,

    /// PR validation configuration
    #[serde(default)]
    pub pr: PrConfig,

    /// Release notes configuration
    #[serde(default)]
    pub release_notes: ReleaseNotesConfig,

    /// Git hooks configuration (commit-msg, pre-commit, pre-push)
    #[serde(default)]
    pub git_hooks: GitHooksConfig,

    /// Tool version pinning (mise/asdf-style)
    #[serde(default)]
    pub tools: ToolsConfig,
}
