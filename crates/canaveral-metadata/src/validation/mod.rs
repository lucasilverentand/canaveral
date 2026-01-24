//! Validation framework for app store metadata.
//!
//! This module provides validation utilities and validators for different app store platforms.
//!
//! ## Example
//!
//! ```rust
//! use canaveral_metadata::validation::{AppleValidator, ValidationResult, Severity};
//! use canaveral_metadata::AppleMetadata;
//!
//! let metadata = AppleMetadata::new("com.example.app");
//! let validator = AppleValidator::new(false);
//! let result = validator.validate(&metadata);
//!
//! if !result.is_valid() {
//!     for error in result.errors() {
//!         eprintln!("Error in {}: {}", error.field, error.message);
//!     }
//! }
//! ```

mod apple;
mod common;
mod google_play;

pub use apple::{validate_localized_screenshots, AppleValidator};
pub use common::*;
pub use google_play::{validate_localized_google_play_screenshots, GooglePlayValidator};

/// Validation issue severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    /// Must fix before upload - blocks submission.
    Error,
    /// Should fix but not blocking - may affect app quality or visibility.
    Warning,
    /// Informational - suggestions for improvement.
    Info,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => write!(f, "ERROR"),
            Severity::Warning => write!(f, "WARNING"),
            Severity::Info => write!(f, "INFO"),
        }
    }
}

/// A single validation issue found during validation.
#[derive(Debug, Clone)]
pub struct ValidationIssue {
    /// Severity level of the issue.
    pub severity: Severity,
    /// Field path where the issue was found (e.g., "en-US.name", "screenshots.iphone_6_5").
    pub field: String,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional suggestion for how to fix the issue.
    pub suggestion: Option<String>,
}

impl ValidationIssue {
    /// Creates a new validation issue.
    pub fn new(
        severity: Severity,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            field: field.into(),
            message: message.into(),
            suggestion: None,
        }
    }

    /// Creates a new validation issue with a suggestion.
    pub fn with_suggestion(
        severity: Severity,
        field: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            field: field.into(),
            message: message.into(),
            suggestion: Some(suggestion.into()),
        }
    }

    /// Creates an error issue.
    pub fn error(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Error, field, message)
    }

    /// Creates a warning issue.
    pub fn warning(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Warning, field, message)
    }

    /// Creates an info issue.
    pub fn info(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::new(Severity::Info, field, message)
    }
}

impl std::fmt::Display for ValidationIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}: {}", self.severity, self.field, self.message)?;
        if let Some(ref suggestion) = self.suggestion {
            write!(f, " (Suggestion: {})", suggestion)?;
        }
        Ok(())
    }
}

/// Result of validation containing all found issues.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    /// All validation issues found.
    pub issues: Vec<ValidationIssue>,
}

impl ValidationResult {
    /// Creates a new empty validation result.
    pub fn new() -> Self {
        Self { issues: Vec::new() }
    }

    /// Returns `true` if validation passed (no errors).
    ///
    /// Note: Warnings and info issues do not affect validity.
    pub fn is_valid(&self) -> bool {
        !self.issues.iter().any(|i| i.severity == Severity::Error)
    }

    /// Returns `true` if there are no issues at all.
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }

    /// Returns all error issues.
    pub fn errors(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .collect()
    }

    /// Returns all warning issues.
    pub fn warnings(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .collect()
    }

    /// Returns all info issues.
    pub fn infos(&self) -> Vec<&ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Info)
            .collect()
    }

    /// Counts errors.
    pub fn error_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Error)
            .count()
    }

    /// Counts warnings.
    pub fn warning_count(&self) -> usize {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
            .count()
    }

    /// Adds an issue to the result.
    pub fn add(&mut self, issue: ValidationIssue) {
        self.issues.push(issue);
    }

    /// Adds an error issue.
    pub fn add_error(&mut self, field: &str, message: &str, suggestion: Option<&str>) {
        let issue = match suggestion {
            Some(s) => ValidationIssue::with_suggestion(Severity::Error, field, message, s),
            None => ValidationIssue::error(field, message),
        };
        self.issues.push(issue);
    }

    /// Adds a warning issue.
    pub fn add_warning(&mut self, field: &str, message: &str, suggestion: Option<&str>) {
        let issue = match suggestion {
            Some(s) => ValidationIssue::with_suggestion(Severity::Warning, field, message, s),
            None => ValidationIssue::warning(field, message),
        };
        self.issues.push(issue);
    }

    /// Adds an info issue.
    pub fn add_info(&mut self, field: &str, message: &str, suggestion: Option<&str>) {
        let issue = match suggestion {
            Some(s) => ValidationIssue::with_suggestion(Severity::Info, field, message, s),
            None => ValidationIssue::info(field, message),
        };
        self.issues.push(issue);
    }

    /// Merges another validation result into this one.
    pub fn merge(&mut self, other: ValidationResult) {
        self.issues.extend(other.issues);
    }

    /// Returns an iterator over all issues.
    pub fn iter(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter()
    }
}

impl IntoIterator for ValidationResult {
    type Item = ValidationIssue;
    type IntoIter = std::vec::IntoIter<ValidationIssue>;

    fn into_iter(self) -> Self::IntoIter {
        self.issues.into_iter()
    }
}

impl<'a> IntoIterator for &'a ValidationResult {
    type Item = &'a ValidationIssue;
    type IntoIter = std::slice::Iter<'a, ValidationIssue>;

    fn into_iter(self) -> Self::IntoIter {
        self.issues.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_is_valid() {
        let mut result = ValidationResult::new();
        assert!(result.is_valid());
        assert!(result.is_clean());

        result.add_warning("field", "warning", None);
        assert!(result.is_valid()); // Warnings don't affect validity
        assert!(!result.is_clean());

        result.add_error("field", "error", None);
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validation_result_counts() {
        let mut result = ValidationResult::new();
        result.add_error("f1", "e1", None);
        result.add_error("f2", "e2", None);
        result.add_warning("f3", "w1", None);
        result.add_info("f4", "i1", None);

        assert_eq!(result.error_count(), 2);
        assert_eq!(result.warning_count(), 1);
        assert_eq!(result.errors().len(), 2);
        assert_eq!(result.warnings().len(), 1);
        assert_eq!(result.infos().len(), 1);
    }

    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult::new();
        result1.add_error("f1", "e1", None);

        let mut result2 = ValidationResult::new();
        result2.add_warning("f2", "w1", None);

        result1.merge(result2);
        assert_eq!(result1.issues.len(), 2);
    }

    #[test]
    fn test_validation_issue_display() {
        let issue = ValidationIssue::with_suggestion(
            Severity::Error,
            "en-US.name",
            "Name is too long",
            "Shorten to 30 characters",
        );
        let display = format!("{}", issue);
        assert!(display.contains("ERROR"));
        assert!(display.contains("en-US.name"));
        assert!(display.contains("Name is too long"));
        assert!(display.contains("Shorten to 30 characters"));
    }
}
