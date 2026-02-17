//! Publish options and configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Options for publishing a package
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PublishOptions {
    /// Perform a dry run without actually publishing
    pub dry_run: bool,

    /// Registry URL to publish to (adapter-specific default if None)
    pub registry: Option<String>,

    /// Access level for the package
    pub access: Option<PublishAccess>,

    /// Tag to use (e.g., "latest", "next", "beta")
    pub tag: Option<String>,

    /// OTP/2FA code if required
    pub otp: Option<String>,

    /// Additional adapter-specific options
    pub extra: HashMap<String, String>,
}

impl PublishOptions {
    /// Create new publish options
    pub fn new() -> Self {
        Self::default()
    }

    /// Set dry run mode
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    /// Set the registry
    pub fn registry(mut self, registry: impl Into<String>) -> Self {
        self.registry = Some(registry.into());
        self
    }

    /// Set access level
    pub fn access(mut self, access: PublishAccess) -> Self {
        self.access = Some(access);
        self
    }

    /// Set the tag
    pub fn tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Set OTP code
    pub fn otp(mut self, otp: impl Into<String>) -> Self {
        self.otp = Some(otp.into());
        self
    }

    /// Add extra option
    pub fn with_extra(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.extra.insert(key.into(), value.into());
        self
    }
}

/// Access level for published packages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PublishAccess {
    /// Public package (anyone can install)
    Public,
    /// Restricted/private package
    Restricted,
}

impl Default for PublishAccess {
    fn default() -> Self {
        Self::Public
    }
}

impl std::fmt::Display for PublishAccess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Restricted => write!(f, "restricted"),
        }
    }
}

/// Result of a pre-publish validation check
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// Whether the validation passed
    pub passed: bool,
    /// Error messages (if any)
    pub errors: Vec<String>,
    /// Warning messages (if any)
    pub warnings: Vec<String>,
}

impl ValidationResult {
    /// Create a passing validation result
    pub fn pass() -> Self {
        Self {
            passed: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Create a failing validation result
    pub fn fail(error: impl Into<String>) -> Self {
        Self {
            passed: false,
            errors: vec![error.into()],
            warnings: Vec::new(),
        }
    }

    /// Add an error
    pub fn add_error(&mut self, error: impl Into<String>) {
        self.errors.push(error.into());
        self.passed = false;
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: impl Into<String>) {
        self.warnings.push(warning.into());
    }

    /// Merge another validation result into this one
    pub fn merge(&mut self, other: ValidationResult) {
        if !other.passed {
            self.passed = false;
        }
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::pass()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_publish_options_builder() {
        let opts = PublishOptions::new()
            .dry_run(true)
            .registry("https://custom.registry.com")
            .access(PublishAccess::Restricted)
            .tag("beta")
            .otp("123456");

        assert!(opts.dry_run);
        assert_eq!(
            opts.registry,
            Some("https://custom.registry.com".to_string())
        );
        assert_eq!(opts.access, Some(PublishAccess::Restricted));
        assert_eq!(opts.tag, Some("beta".to_string()));
        assert_eq!(opts.otp, Some("123456".to_string()));
    }

    #[test]
    fn test_validation_result() {
        let mut result = ValidationResult::pass();
        assert!(result.passed);

        result.add_warning("Minor issue");
        assert!(result.passed);

        result.add_error("Major issue");
        assert!(!result.passed);
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.warnings.len(), 1);
    }

    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult::pass();
        result1.add_warning("Warning 1");

        let mut result2 = ValidationResult::pass();
        result2.add_error("Error 1");

        result1.merge(result2);
        assert!(!result1.passed);
        assert_eq!(result1.errors.len(), 1);
        assert_eq!(result1.warnings.len(), 1);
    }
}
