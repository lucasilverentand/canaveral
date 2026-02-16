//! PR validation workflow

use crate::config::Config;

/// Branching model for PR validation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BranchingModel {
    /// Trunk-based development (feature branches → main)
    TrunkBased,
    /// Git-flow (feature → develop → release → main)
    GitFlow,
    /// Custom branch patterns
    Custom(Vec<String>),
}

impl BranchingModel {
    /// Parse from config string
    pub fn from_config(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "trunk-based" | "trunk" => Self::TrunkBased,
            "gitflow" | "git-flow" => Self::GitFlow,
            _ => Self::Custom(vec![s.to_string()]),
        }
    }

    /// Get the main release branch for this model
    pub fn release_branch(&self) -> &str {
        match self {
            Self::TrunkBased => "main",
            Self::GitFlow => "main",
            Self::Custom(branches) => branches.first().map(|s| s.as_str()).unwrap_or("main"),
        }
    }

    /// Check if a branch can be merged to the release branch
    pub fn is_valid_source(&self, branch: &str) -> bool {
        match self {
            Self::TrunkBased => {
                // Any branch can merge to main in trunk-based
                branch != "main"
            }
            Self::GitFlow => {
                // Only release and hotfix branches merge to main
                branch.starts_with("release/")
                    || branch.starts_with("hotfix/")
                    || branch == "develop"
            }
            Self::Custom(_) => true,
        }
    }
}

/// Result of a single PR check
#[derive(Debug, Clone)]
pub struct CheckResult {
    /// Check name
    pub name: String,
    /// Whether the check passed
    pub passed: bool,
    /// Description or reason
    pub message: String,
    /// Whether this is just a warning
    pub is_warning: bool,
}

/// PR validation results
#[derive(Debug, Clone)]
pub struct PrValidationResult {
    /// Individual check results
    pub checks: Vec<CheckResult>,
}

impl PrValidationResult {
    /// Create empty results
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Add a check result
    pub fn add_check(&mut self, check: CheckResult) {
        self.checks.push(check);
    }

    /// Whether all checks passed
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed || c.is_warning)
    }

    /// Whether all checks passed (strict: warnings are failures)
    pub fn all_passed_strict(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }

    /// Get failed checks
    pub fn failures(&self) -> Vec<&CheckResult> {
        self.checks.iter().filter(|c| !c.passed && !c.is_warning).collect()
    }

    /// Get warnings
    pub fn warnings(&self) -> Vec<&CheckResult> {
        self.checks.iter().filter(|c| c.is_warning).collect()
    }
}

impl Default for PrValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Preview of what a release would look like
#[derive(Debug, Clone)]
pub struct PrPreview {
    /// Current version
    pub current_version: String,
    /// What the next version would be
    pub next_version: Option<String>,
    /// Bump type (major, minor, patch)
    pub bump_type: String,
    /// Number of relevant commits
    pub commit_count: usize,
    /// Whether there are breaking changes
    pub has_breaking_changes: bool,
    /// Summary of changes
    pub summary: Vec<String>,
}

/// PR validator — runs configured checks
pub struct PrValidator<'a> {
    config: &'a Config,
}

impl<'a> PrValidator<'a> {
    /// Create a new PR validator
    pub fn new(config: &'a Config) -> Self {
        Self { config }
    }

    /// Get the configured branching model
    pub fn branching_model(&self) -> BranchingModel {
        BranchingModel::from_config(&self.config.pr.branching_model)
    }

    /// Validate a branch name
    pub fn validate_branch(&self, branch: &str) -> CheckResult {
        let model = self.branching_model();
        let valid = model.is_valid_source(branch);

        CheckResult {
            name: "branch-model".to_string(),
            passed: valid,
            message: if valid {
                format!("Branch '{}' is valid for this branching model", branch)
            } else {
                format!(
                    "Branch '{}' cannot merge to '{}' in {} model",
                    branch,
                    model.release_branch(),
                    self.config.pr.branching_model
                )
            },
            is_warning: false,
        }
    }

    /// Check if changelog is required and present
    pub fn check_changelog(&self, has_changelog_entry: bool) -> CheckResult {
        if !self.config.pr.require_changelog {
            return CheckResult {
                name: "changelog".to_string(),
                passed: true,
                message: "Changelog not required".to_string(),
                is_warning: false,
            };
        }

        CheckResult {
            name: "changelog".to_string(),
            passed: has_changelog_entry,
            message: if has_changelog_entry {
                "Changelog entry present".to_string()
            } else {
                "Changelog entry required but not found".to_string()
            },
            is_warning: false,
        }
    }

    /// Get configured checks
    pub fn configured_checks(&self) -> &[String] {
        &self.config.pr.checks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_branching_model_trunk_based() {
        let model = BranchingModel::from_config("trunk-based");
        assert_eq!(model, BranchingModel::TrunkBased);
        assert_eq!(model.release_branch(), "main");
        assert!(model.is_valid_source("feature/foo"));
        assert!(!model.is_valid_source("main"));
    }

    #[test]
    fn test_branching_model_gitflow() {
        let model = BranchingModel::from_config("gitflow");
        assert_eq!(model, BranchingModel::GitFlow);
        assert!(model.is_valid_source("release/1.0"));
        assert!(model.is_valid_source("hotfix/urgent"));
        assert!(model.is_valid_source("develop"));
        assert!(!model.is_valid_source("feature/foo"));
    }

    #[test]
    fn test_validation_result() {
        let mut result = PrValidationResult::new();
        result.add_check(CheckResult {
            name: "test".to_string(),
            passed: true,
            message: "ok".to_string(),
            is_warning: false,
        });
        result.add_check(CheckResult {
            name: "warn".to_string(),
            passed: false,
            message: "warning".to_string(),
            is_warning: true,
        });

        assert!(result.all_passed());
        assert!(!result.all_passed_strict());
        assert!(result.failures().is_empty());
        assert_eq!(result.warnings().len(), 1);
    }

    #[test]
    fn test_pr_validator() {
        let config = Config::default();
        let validator = PrValidator::new(&config);

        let branch_check = validator.validate_branch("feature/foo");
        assert!(branch_check.passed);

        let changelog_check = validator.check_changelog(false);
        // Default config doesn't require changelog
        assert!(changelog_check.passed);
    }

    #[test]
    fn test_pr_preview() {
        let preview = PrPreview {
            current_version: "1.0.0".to_string(),
            next_version: Some("1.1.0".to_string()),
            bump_type: "minor".to_string(),
            commit_count: 5,
            has_breaking_changes: false,
            summary: vec!["Added new feature".to_string()],
        };

        assert!(!preview.has_breaking_changes);
        assert_eq!(preview.bump_type, "minor");
    }
}
