//! Pre-release validation

use crate::config::Config;
use crate::error::{Result, WorkflowError};

/// Validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether validation passed
    pub passed: bool,
    /// List of errors
    pub errors: Vec<String>,
    /// List of warnings
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a passing result
    pub fn pass() -> Self {
        Self {
            passed: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failing result with an error
    pub fn fail(error: impl Into<String>) -> Self {
        Self {
            passed: false,
            errors: vec![error.into()],
            warnings: Vec::new(),
        }
    }

    /// Add an error
    pub fn add_error(&mut self, error: impl Into<String>) {
        self.passed = false;
        self.errors.push(error.into());
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }
}

/// Validate that the release can proceed
pub fn validate_release(
    config: &Config,
    is_clean: bool,
    on_correct_branch: bool,
) -> Result<ValidationResult> {
    let mut result = ValidationResult::pass();

    // Check working directory
    if config.git.require_clean && !is_clean {
        result.add_error("Working directory has uncommitted changes");
    }

    // Check branch
    if !on_correct_branch {
        result.add_error(format!(
            "Not on release branch '{}'. Use --allow-branch to override",
            config.git.branch
        ));
    }

    if !result.passed {
        return Err(WorkflowError::ValidationFailed(result.errors.join("; ")).into());
    }

    Ok(result)
}

/// Validate configuration
pub fn validate_config_for_release(config: &Config) -> Result<ValidationResult> {
    let mut result = ValidationResult::pass();

    // Warn about disabled features
    if !config.changelog.enabled {
        result.add_warning("Changelog generation is disabled");
    }

    if !config.publish.enabled {
        result.add_warning("Publishing is disabled");
    }

    Ok(result)
}
