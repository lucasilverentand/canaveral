//! Hook System - Execute shell commands at lifecycle stages
//!
//! Hooks allow running custom commands at various points during the release process:
//! - pre-version: Before version is bumped
//! - post-version: After version is bumped
//! - pre-changelog: Before changelog is generated
//! - post-changelog: After changelog is generated
//! - pre-commit: Before git commit
//! - post-commit: After git commit
//! - pre-tag: Before git tag
//! - post-tag: After git tag
//! - pre-publish: Before publishing
//! - post-publish: After publishing
//! - pre-release: Before the entire release process
//! - post-release: After the entire release process

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{HookError, Result};

/// Hook lifecycle stages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HookStage {
    /// Before the entire release process
    PreRelease,
    /// After the entire release process
    PostRelease,
    /// Before version bump
    PreVersion,
    /// After version bump
    PostVersion,
    /// Before changelog generation
    PreChangelog,
    /// After changelog generation
    PostChangelog,
    /// Before git commit
    PreCommit,
    /// After git commit
    PostCommit,
    /// Before git tag
    PreTag,
    /// After git tag
    PostTag,
    /// Before publishing
    PrePublish,
    /// After publishing
    PostPublish,
}

impl HookStage {
    /// Get the stage name as a string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PreRelease => "pre-release",
            Self::PostRelease => "post-release",
            Self::PreVersion => "pre-version",
            Self::PostVersion => "post-version",
            Self::PreChangelog => "pre-changelog",
            Self::PostChangelog => "post-changelog",
            Self::PreCommit => "pre-commit",
            Self::PostCommit => "post-commit",
            Self::PreTag => "pre-tag",
            Self::PostTag => "post-tag",
            Self::PrePublish => "pre-publish",
            Self::PostPublish => "post-publish",
        }
    }

    /// Parse stage from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pre-release" => Some(Self::PreRelease),
            "post-release" => Some(Self::PostRelease),
            "pre-version" => Some(Self::PreVersion),
            "post-version" => Some(Self::PostVersion),
            "pre-changelog" => Some(Self::PreChangelog),
            "post-changelog" => Some(Self::PostChangelog),
            "pre-commit" => Some(Self::PreCommit),
            "post-commit" => Some(Self::PostCommit),
            "pre-tag" => Some(Self::PreTag),
            "post-tag" => Some(Self::PostTag),
            "pre-publish" => Some(Self::PrePublish),
            "post-publish" => Some(Self::PostPublish),
            _ => None,
        }
    }

    /// Get all stages in order
    pub fn all() -> &'static [HookStage] {
        &[
            Self::PreRelease,
            Self::PreVersion,
            Self::PostVersion,
            Self::PreChangelog,
            Self::PostChangelog,
            Self::PreCommit,
            Self::PostCommit,
            Self::PreTag,
            Self::PostTag,
            Self::PrePublish,
            Self::PostPublish,
            Self::PostRelease,
        ]
    }
}

/// A hook command to execute
#[derive(Debug, Clone)]
pub struct Hook {
    /// The command to run
    pub command: String,
    /// Working directory (defaults to project root)
    pub cwd: Option<String>,
    /// Environment variables to set
    pub env: HashMap<String, String>,
    /// Whether to fail the release if hook fails
    pub fail_on_error: bool,
    /// Timeout in seconds
    pub timeout: Option<u64>,
    /// Description for logging
    pub description: Option<String>,
}

impl Hook {
    /// Create a new hook with just a command
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            cwd: None,
            env: HashMap::new(),
            fail_on_error: true,
            timeout: None,
            description: None,
        }
    }

    /// Set the working directory
    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set whether to fail on error
    pub fn with_fail_on_error(mut self, fail: bool) -> Self {
        self.fail_on_error = fail;
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = Some(seconds);
        self
    }

    /// Set the description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }
}

/// Result of executing a hook
#[derive(Debug, Clone)]
pub struct HookResult {
    /// The stage that was executed
    pub stage: HookStage,
    /// The command that was run
    pub command: String,
    /// Whether execution succeeded
    pub success: bool,
    /// Exit code if available
    pub exit_code: Option<i32>,
    /// Standard output
    pub stdout: String,
    /// Standard error
    pub stderr: String,
    /// Execution time in milliseconds
    pub duration_ms: u64,
}

/// Hook execution context
#[derive(Debug, Clone, Default)]
pub struct HookContext {
    /// Current version
    pub version: Option<String>,
    /// Previous version
    pub previous_version: Option<String>,
    /// Package name
    pub package_name: Option<String>,
    /// Release type (major, minor, patch)
    pub release_type: Option<String>,
    /// Git tag
    pub tag: Option<String>,
    /// Whether this is a dry run
    pub dry_run: bool,
    /// Additional custom variables
    pub custom: HashMap<String, String>,
}

impl HookContext {
    /// Create a new hook context
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Set the previous version
    pub fn with_previous_version(mut self, version: impl Into<String>) -> Self {
        self.previous_version = Some(version.into());
        self
    }

    /// Set the package name
    pub fn with_package_name(mut self, name: impl Into<String>) -> Self {
        self.package_name = Some(name.into());
        self
    }

    /// Set the release type
    pub fn with_release_type(mut self, release_type: impl Into<String>) -> Self {
        self.release_type = Some(release_type.into());
        self
    }

    /// Set the tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Set dry run mode
    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Add a custom variable
    pub fn with_custom(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.custom.insert(key.into(), value.into());
        self
    }

    /// Convert context to environment variables
    pub fn to_env(&self) -> HashMap<String, String> {
        let mut env = HashMap::new();

        if let Some(ref v) = self.version {
            env.insert("CANAVERAL_VERSION".to_string(), v.clone());
        }
        if let Some(ref v) = self.previous_version {
            env.insert("CANAVERAL_PREVIOUS_VERSION".to_string(), v.clone());
        }
        if let Some(ref v) = self.package_name {
            env.insert("CANAVERAL_PACKAGE".to_string(), v.clone());
        }
        if let Some(ref v) = self.release_type {
            env.insert("CANAVERAL_RELEASE_TYPE".to_string(), v.clone());
        }
        if let Some(ref v) = self.tag {
            env.insert("CANAVERAL_TAG".to_string(), v.clone());
        }
        env.insert("CANAVERAL_DRY_RUN".to_string(), self.dry_run.to_string());

        for (k, v) in &self.custom {
            env.insert(format!("CANAVERAL_{}", k.to_uppercase()), v.clone());
        }

        env
    }
}

/// Hook runner for executing hooks at lifecycle stages
#[derive(Debug, Clone, Default)]
pub struct HookRunner {
    /// Registered hooks by stage
    hooks: HashMap<HookStage, Vec<Hook>>,
    /// Base directory for relative paths
    base_dir: Option<String>,
}

impl HookRunner {
    /// Create a new hook runner
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the base directory
    pub fn with_base_dir(mut self, dir: impl Into<String>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// Register a hook for a stage
    pub fn register(&mut self, stage: HookStage, hook: Hook) {
        self.hooks.entry(stage).or_default().push(hook);
    }

    /// Register multiple hooks for a stage
    pub fn register_all(&mut self, stage: HookStage, hooks: Vec<Hook>) {
        for hook in hooks {
            self.register(stage, hook);
        }
    }

    /// Get hooks for a stage
    pub fn get_hooks(&self, stage: HookStage) -> &[Hook] {
        self.hooks.get(&stage).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Check if there are any hooks for a stage
    pub fn has_hooks(&self, stage: HookStage) -> bool {
        self.hooks.get(&stage).map(|v| !v.is_empty()).unwrap_or(false)
    }

    /// Execute all hooks for a stage
    pub fn run(&self, stage: HookStage, context: &HookContext) -> Result<Vec<HookResult>> {
        let hooks = self.get_hooks(stage);
        if hooks.is_empty() {
            return Ok(Vec::new());
        }

        let mut results = Vec::new();
        let context_env = context.to_env();

        for hook in hooks {
            let result = self.execute_hook(stage, hook, &context_env)?;
            let failed = !result.success && hook.fail_on_error;
            results.push(result);

            if failed {
                return Err(HookError::ExecutionFailed {
                    stage: stage.as_str().to_string(),
                    command: hook.command.clone(),
                    message: "Hook failed with non-zero exit code".to_string(),
                }
                .into());
            }
        }

        Ok(results)
    }

    /// Execute a single hook
    fn execute_hook(
        &self,
        stage: HookStage,
        hook: &Hook,
        context_env: &HashMap<String, String>,
    ) -> Result<HookResult> {
        let start = std::time::Instant::now();

        // Determine working directory
        let cwd = hook
            .cwd
            .as_ref()
            .or(self.base_dir.as_ref())
            .map(|s| s.as_str());

        // Build command
        let shell = if cfg!(windows) { "cmd" } else { "sh" };
        let shell_arg = if cfg!(windows) { "/C" } else { "-c" };

        let mut cmd = Command::new(shell);
        cmd.arg(shell_arg).arg(&hook.command);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        // Set environment variables
        for (k, v) in context_env {
            cmd.env(k, v);
        }
        for (k, v) in &hook.env {
            cmd.env(k, v);
        }

        // Capture output
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        // Execute
        let output = cmd.output().map_err(|e| HookError::ExecutionFailed {
            stage: stage.as_str().to_string(),
            command: hook.command.clone(),
            message: e.to_string(),
        })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(HookResult {
            stage,
            command: hook.command.clone(),
            success: output.status.success(),
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            duration_ms,
        })
    }
}

/// Configuration for hooks
#[derive(Debug, Clone, Default)]
pub struct HooksConfig {
    /// Hooks organized by stage
    pub hooks: HashMap<String, Vec<HookConfig>>,
}

/// Single hook configuration
#[derive(Debug, Clone)]
pub struct HookConfig {
    /// Command to run
    pub command: String,
    /// Working directory
    pub cwd: Option<String>,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// Whether to fail on error (default true)
    pub fail_on_error: bool,
    /// Timeout in seconds
    pub timeout: Option<u64>,
    /// Description
    pub description: Option<String>,
}

impl From<HookConfig> for Hook {
    fn from(config: HookConfig) -> Self {
        Hook {
            command: config.command,
            cwd: config.cwd,
            env: config.env,
            fail_on_error: config.fail_on_error,
            timeout: config.timeout,
            description: config.description,
        }
    }
}

impl From<String> for HookConfig {
    fn from(command: String) -> Self {
        HookConfig {
            command,
            cwd: None,
            env: HashMap::new(),
            fail_on_error: true,
            timeout: None,
            description: None,
        }
    }
}

impl From<&str> for HookConfig {
    fn from(command: &str) -> Self {
        HookConfig::from(command.to_string())
    }
}

/// Build a HookRunner from a HooksConfig
pub fn build_hook_runner(config: &HooksConfig, base_dir: Option<&Path>) -> HookRunner {
    let mut runner = HookRunner::new();

    if let Some(dir) = base_dir {
        runner = runner.with_base_dir(dir.to_string_lossy().to_string());
    }

    for (stage_name, hook_configs) in &config.hooks {
        if let Some(stage) = HookStage::from_str(stage_name) {
            for hook_config in hook_configs {
                runner.register(stage, hook_config.clone().into());
            }
        }
    }

    runner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_stage_roundtrip() {
        for stage in HookStage::all() {
            let s = stage.as_str();
            let parsed = HookStage::from_str(s);
            assert_eq!(parsed, Some(*stage));
        }
    }

    #[test]
    fn test_hook_creation() {
        let hook = Hook::new("echo hello")
            .with_cwd("/tmp")
            .with_env("FOO", "bar")
            .with_fail_on_error(false)
            .with_timeout(30)
            .with_description("Test hook");

        assert_eq!(hook.command, "echo hello");
        assert_eq!(hook.cwd, Some("/tmp".to_string()));
        assert_eq!(hook.env.get("FOO"), Some(&"bar".to_string()));
        assert!(!hook.fail_on_error);
        assert_eq!(hook.timeout, Some(30));
        assert_eq!(hook.description, Some("Test hook".to_string()));
    }

    #[test]
    fn test_hook_context_to_env() {
        let ctx = HookContext::new()
            .with_version("1.0.0")
            .with_previous_version("0.9.0")
            .with_package_name("my-package")
            .with_release_type("minor")
            .with_tag("v1.0.0")
            .with_dry_run(true)
            .with_custom("build_id", "123");

        let env = ctx.to_env();

        assert_eq!(env.get("CANAVERAL_VERSION"), Some(&"1.0.0".to_string()));
        assert_eq!(
            env.get("CANAVERAL_PREVIOUS_VERSION"),
            Some(&"0.9.0".to_string())
        );
        assert_eq!(env.get("CANAVERAL_PACKAGE"), Some(&"my-package".to_string()));
        assert_eq!(env.get("CANAVERAL_RELEASE_TYPE"), Some(&"minor".to_string()));
        assert_eq!(env.get("CANAVERAL_TAG"), Some(&"v1.0.0".to_string()));
        assert_eq!(env.get("CANAVERAL_DRY_RUN"), Some(&"true".to_string()));
        assert_eq!(env.get("CANAVERAL_BUILD_ID"), Some(&"123".to_string()));
    }

    #[test]
    fn test_hook_runner_register() {
        let mut runner = HookRunner::new();
        runner.register(HookStage::PreVersion, Hook::new("echo pre"));
        runner.register(HookStage::PostVersion, Hook::new("echo post"));

        assert!(runner.has_hooks(HookStage::PreVersion));
        assert!(runner.has_hooks(HookStage::PostVersion));
        assert!(!runner.has_hooks(HookStage::PreCommit));
    }

    #[test]
    fn test_hook_execution() {
        let mut runner = HookRunner::new();
        runner.register(HookStage::PreVersion, Hook::new("echo hello"));

        let ctx = HookContext::new().with_version("1.0.0");
        let results = runner.run(HookStage::PreVersion, &ctx).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(results[0].stdout.contains("hello"));
    }

    #[test]
    fn test_hook_with_env_from_context() {
        let mut runner = HookRunner::new();
        runner.register(
            HookStage::PreVersion,
            Hook::new("echo $CANAVERAL_VERSION"),
        );

        let ctx = HookContext::new().with_version("2.0.0");
        let results = runner.run(HookStage::PreVersion, &ctx).unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].success);
        assert!(results[0].stdout.contains("2.0.0"));
    }

    #[test]
    fn test_hook_failure_handling() {
        let mut runner = HookRunner::new();
        runner.register(
            HookStage::PreVersion,
            Hook::new("exit 1").with_fail_on_error(false),
        );

        let ctx = HookContext::new();
        let results = runner.run(HookStage::PreVersion, &ctx).unwrap();

        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
    }

    #[test]
    fn test_hook_failure_stops_execution() {
        let mut runner = HookRunner::new();
        runner.register(HookStage::PreVersion, Hook::new("exit 1"));
        runner.register(HookStage::PreVersion, Hook::new("echo second"));

        let ctx = HookContext::new();
        let result = runner.run(HookStage::PreVersion, &ctx);

        assert!(result.is_err());
    }

    #[test]
    fn test_build_hook_runner_from_config() {
        let mut config = HooksConfig::default();
        config.hooks.insert(
            "pre-version".to_string(),
            vec![HookConfig::from("echo test")],
        );

        let runner = build_hook_runner(&config, None);
        assert!(runner.has_hooks(HookStage::PreVersion));
    }
}
