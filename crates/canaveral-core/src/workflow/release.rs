//! Release workflow orchestration

use tracing::{debug, info};

use crate::config::Config;
use crate::error::Result;
use crate::types::{ReleaseResult, ReleaseType};

/// Options for a release
#[derive(Debug, Clone, Default)]
pub struct ReleaseOptions {
    /// Release type (major, minor, patch, etc.)
    pub release_type: Option<ReleaseType>,
    /// Explicit version to set
    pub version: Option<String>,
    /// Pre-release identifier
    pub prerelease: Option<String>,
    /// Whether this is a dry run
    pub dry_run: bool,
    /// Skip changelog generation
    pub skip_changelog: bool,
    /// Skip publishing
    pub skip_publish: bool,
    /// Skip git operations
    pub skip_git: bool,
    /// Allow release from non-release branch
    pub allow_branch: bool,
    /// Package to release (for monorepos)
    pub package: Option<String>,
}

impl ReleaseOptions {
    /// Create options for a dry run
    pub fn dry_run() -> Self {
        Self {
            dry_run: true,
            ..Default::default()
        }
    }

    /// Set release type
    pub fn with_release_type(mut self, release_type: ReleaseType) -> Self {
        self.release_type = Some(release_type);
        self
    }

    /// Set explicit version
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }
}

/// Execute a release workflow
pub struct ReleaseWorkflow<'a> {
    config: &'a Config,
    options: ReleaseOptions,
}

impl<'a> ReleaseWorkflow<'a> {
    /// Create a new release workflow
    pub fn new(config: &'a Config, options: ReleaseOptions) -> Self {
        Self { config, options }
    }

    /// Execute the release
    pub fn execute(&self) -> Result<ReleaseResult> {
        let version = self
            .options
            .version
            .clone()
            .unwrap_or_else(|| "0.1.0".to_string());

        let release_type = self.options.release_type.unwrap_or(ReleaseType::Patch);

        info!(
            version = %version,
            release_type = ?release_type,
            dry_run = self.is_dry_run(),
            "executing release workflow"
        );

        let result = ReleaseResult::new(
            self.config
                .name
                .clone()
                .unwrap_or_else(|| "package".to_string()),
            &version,
        )
        .with_release_type(release_type)
        .with_published(!self.options.skip_publish && !self.options.dry_run);

        debug!(version = %result.new_version, "release workflow complete");
        Ok(result)
    }

    /// Check if this is a dry run
    pub fn is_dry_run(&self) -> bool {
        self.options.dry_run || self.config.publish.dry_run
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_release_options_default() {
        let opts = ReleaseOptions::default();
        assert!(!opts.dry_run);
        assert!(!opts.skip_changelog);
        assert!(opts.release_type.is_none());
    }

    #[test]
    fn test_release_workflow() {
        let config = Config::default();
        let opts = ReleaseOptions::default().with_version("1.0.0");
        let workflow = ReleaseWorkflow::new(&config, opts);

        let result = workflow.execute().unwrap();
        assert_eq!(result.new_version, "1.0.0");
    }
}
