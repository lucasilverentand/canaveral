//! Git hooks and lifecycle hooks configuration

use serde::{Deserialize, Serialize};

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

/// Git hooks configuration (commit-msg, pre-commit, pre-push validation)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitHooksConfig {
    /// Whether to auto-install hooks on `canaveral init`
    pub auto_install: bool,

    /// Commit message validation settings
    pub commit_msg: CommitMsgHookConfig,

    /// Pre-commit hook settings
    pub pre_commit: ScriptHookConfig,

    /// Pre-push hook settings
    pub pre_push: ScriptHookConfig,
}

impl Default for GitHooksConfig {
    fn default() -> Self {
        Self {
            auto_install: true,
            commit_msg: CommitMsgHookConfig::default(),
            pre_commit: ScriptHookConfig::default(),
            pre_push: ScriptHookConfig::default(),
        }
    }
}

/// Commit message hook configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommitMsgHookConfig {
    /// Enforce conventional commits format
    pub conventional_commits: bool,

    /// Allowed commit types (empty = all standard types)
    #[serde(default)]
    pub allowed_types: Vec<String>,

    /// Maximum subject line length
    pub max_subject_length: usize,

    /// Allow WIP commits (skip validation for messages starting with "WIP" or "wip")
    pub allow_wip: bool,
}

impl Default for CommitMsgHookConfig {
    fn default() -> Self {
        Self {
            conventional_commits: true,
            allowed_types: Vec::new(),
            max_subject_length: 72,
            allow_wip: true,
        }
    }
}

/// Script-based hook configuration (pre-commit, pre-push)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct ScriptHookConfig {
    /// Commands to run
    #[serde(default)]
    pub commands: Vec<String>,

    /// Whether to run commands in parallel
    pub parallel: bool,
}
