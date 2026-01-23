//! Changelog types

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A parsed commit from conventional commit format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedCommit {
    /// Original commit hash
    pub hash: String,
    /// Commit type (feat, fix, etc.)
    pub commit_type: String,
    /// Scope (optional, in parentheses)
    pub scope: Option<String>,
    /// Whether this is a breaking change
    pub breaking: bool,
    /// Commit description
    pub description: String,
    /// Commit body
    pub body: Option<String>,
    /// Footer fields
    pub footers: Vec<Footer>,
    /// Author name
    pub author: String,
    /// Commit timestamp
    pub timestamp: DateTime<Utc>,
}

impl ParsedCommit {
    /// Check if this commit triggers a major version bump
    pub fn is_major(&self) -> bool {
        self.breaking
    }

    /// Check if this commit triggers a minor version bump
    pub fn is_minor(&self) -> bool {
        self.commit_type == "feat"
    }

    /// Check if this commit triggers a patch version bump
    pub fn is_patch(&self) -> bool {
        matches!(self.commit_type.as_str(), "fix" | "perf")
    }
}

/// A footer field from a conventional commit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Footer {
    /// Footer token (e.g., "BREAKING CHANGE", "Fixes", "Refs")
    pub token: String,
    /// Footer value
    pub value: String,
}

/// A section in a changelog
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    /// Section title
    pub title: String,
    /// Commits in this section
    pub commits: Vec<ParsedCommit>,
}

impl Section {
    /// Create a new section
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            commits: Vec::new(),
        }
    }

    /// Add a commit to the section
    pub fn add_commit(&mut self, commit: ParsedCommit) {
        self.commits.push(commit);
    }

    /// Check if section is empty
    pub fn is_empty(&self) -> bool {
        self.commits.is_empty()
    }
}

/// A changelog entry for a version
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangelogEntry {
    /// Version string
    pub version: String,
    /// Release date
    pub date: DateTime<Utc>,
    /// Sections in this entry
    pub sections: Vec<Section>,
    /// Breaking changes (highlighted separately)
    pub breaking_changes: Vec<ParsedCommit>,
    /// Any additional notes
    pub notes: Option<String>,
}

impl ChangelogEntry {
    /// Create a new changelog entry
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            date: Utc::now(),
            sections: Vec::new(),
            breaking_changes: Vec::new(),
            notes: None,
        }
    }

    /// Set the date
    pub fn with_date(mut self, date: DateTime<Utc>) -> Self {
        self.date = date;
        self
    }

    /// Add a section
    pub fn add_section(&mut self, section: Section) {
        if !section.is_empty() {
            self.sections.push(section);
        }
    }

    /// Add a breaking change
    pub fn add_breaking_change(&mut self, commit: ParsedCommit) {
        self.breaking_changes.push(commit);
    }

    /// Set notes
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Check if entry has any content
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && self.breaking_changes.is_empty()
    }
}

/// Commit type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CommitType {
    /// New feature
    Feat,
    /// Bug fix
    Fix,
    /// Documentation
    Docs,
    /// Code style (formatting, etc.)
    Style,
    /// Refactoring
    Refactor,
    /// Performance improvement
    Perf,
    /// Tests
    Test,
    /// Build system
    Build,
    /// CI configuration
    Ci,
    /// Chores (maintenance)
    Chore,
    /// Reverting changes
    Revert,
    /// Other/unknown
    Other,
}

impl CommitType {
    /// Get the default section title for this type
    pub fn default_section(&self) -> &'static str {
        match self {
            Self::Feat => "Features",
            Self::Fix => "Bug Fixes",
            Self::Docs => "Documentation",
            Self::Style => "Styles",
            Self::Refactor => "Code Refactoring",
            Self::Perf => "Performance Improvements",
            Self::Test => "Tests",
            Self::Build => "Build System",
            Self::Ci => "Continuous Integration",
            Self::Chore => "Chores",
            Self::Revert => "Reverts",
            Self::Other => "Other Changes",
        }
    }

    /// Check if this type should be hidden by default
    pub fn is_hidden_by_default(&self) -> bool {
        matches!(
            self,
            Self::Style | Self::Refactor | Self::Test | Self::Build | Self::Ci | Self::Chore
        )
    }
}

impl std::str::FromStr for CommitType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "feat" | "feature" => Ok(Self::Feat),
            "fix" | "bugfix" => Ok(Self::Fix),
            "docs" | "doc" => Ok(Self::Docs),
            "style" => Ok(Self::Style),
            "refactor" => Ok(Self::Refactor),
            "perf" | "performance" => Ok(Self::Perf),
            "test" | "tests" => Ok(Self::Test),
            "build" => Ok(Self::Build),
            "ci" => Ok(Self::Ci),
            "chore" => Ok(Self::Chore),
            "revert" => Ok(Self::Revert),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_type_from_str() {
        assert_eq!("feat".parse::<CommitType>().unwrap(), CommitType::Feat);
        assert_eq!("fix".parse::<CommitType>().unwrap(), CommitType::Fix);
        assert!("unknown".parse::<CommitType>().is_err());
    }

    #[test]
    fn test_section() {
        let mut section = Section::new("Features");
        assert!(section.is_empty());

        section.add_commit(ParsedCommit {
            hash: "abc123".to_string(),
            commit_type: "feat".to_string(),
            scope: None,
            breaking: false,
            description: "add feature".to_string(),
            body: None,
            footers: vec![],
            author: "Test".to_string(),
            timestamp: Utc::now(),
        });

        assert!(!section.is_empty());
    }
}
