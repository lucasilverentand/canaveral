//! Core types for Canaveral

use serde::{Deserialize, Serialize};

/// Type of release being performed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReleaseType {
    /// Major version bump (breaking changes)
    Major,
    /// Minor version bump (new features)
    Minor,
    /// Patch version bump (bug fixes)
    Patch,
    /// Pre-release version
    Prerelease,
    /// Custom release type
    Custom,
}

impl ReleaseType {
    /// Returns the string representation of the release type
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Major => "major",
            Self::Minor => "minor",
            Self::Patch => "patch",
            Self::Prerelease => "prerelease",
            Self::Custom => "custom",
        }
    }
}

impl std::fmt::Display for ReleaseType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ReleaseType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "major" => Ok(Self::Major),
            "minor" => Ok(Self::Minor),
            "patch" => Ok(Self::Patch),
            "prerelease" | "pre" => Ok(Self::Prerelease),
            "custom" => Ok(Self::Custom),
            _ => Err(format!("Unknown release type: {}", s)),
        }
    }
}

/// Result of a release operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseResult {
    /// The package name
    pub package: String,
    /// Previous version
    pub previous_version: Option<String>,
    /// New version
    pub new_version: String,
    /// Release type that was applied
    pub release_type: ReleaseType,
    /// Tag that was created
    pub tag: String,
    /// Whether the release was published
    pub published: bool,
    /// Changelog content generated
    pub changelog: Option<String>,
    /// Any notes or warnings
    pub notes: Vec<String>,
}

impl ReleaseResult {
    /// Create a new release result
    pub fn new(package: impl Into<String>, new_version: impl Into<String>) -> Self {
        let new_version = new_version.into();
        let package = package.into();
        let tag = format!("v{}", new_version);

        Self {
            package,
            previous_version: None,
            new_version,
            release_type: ReleaseType::Patch,
            tag,
            published: false,
            changelog: None,
            notes: Vec::new(),
        }
    }

    /// Set the previous version
    pub fn with_previous_version(mut self, version: impl Into<String>) -> Self {
        self.previous_version = Some(version.into());
        self
    }

    /// Set the release type
    pub fn with_release_type(mut self, release_type: ReleaseType) -> Self {
        self.release_type = release_type;
        self
    }

    /// Set the tag
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tag = tag.into();
        self
    }

    /// Set whether published
    pub fn with_published(mut self, published: bool) -> Self {
        self.published = published;
        self
    }

    /// Set the changelog
    pub fn with_changelog(mut self, changelog: impl Into<String>) -> Self {
        self.changelog = Some(changelog.into());
        self
    }

    /// Add a note
    pub fn add_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

/// Package information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    /// Package name
    pub name: String,
    /// Current version
    pub version: String,
    /// Package type (npm, cargo, python, etc.)
    pub package_type: String,
    /// Path to the package manifest
    pub manifest_path: std::path::PathBuf,
    /// Whether this is a private package
    pub private: bool,
}

impl PackageInfo {
    /// Create new package info
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        package_type: impl Into<String>,
        manifest_path: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            package_type: package_type.into(),
            manifest_path: manifest_path.into(),
            private: false,
        }
    }

    /// Set whether private
    pub fn with_private(mut self, private: bool) -> Self {
        self.private = private;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_release_type_from_str() {
        assert_eq!(ReleaseType::from_str("major").unwrap(), ReleaseType::Major);
        assert_eq!(ReleaseType::from_str("MINOR").unwrap(), ReleaseType::Minor);
        assert_eq!(ReleaseType::from_str("patch").unwrap(), ReleaseType::Patch);
        assert!(ReleaseType::from_str("invalid").is_err());
    }

    #[test]
    fn test_release_result_builder() {
        let result = ReleaseResult::new("my-package", "1.0.0")
            .with_previous_version("0.9.0")
            .with_release_type(ReleaseType::Minor)
            .with_published(true)
            .add_note("Release successful");

        assert_eq!(result.package, "my-package");
        assert_eq!(result.new_version, "1.0.0");
        assert_eq!(result.previous_version, Some("0.9.0".to_string()));
        assert_eq!(result.release_type, ReleaseType::Minor);
        assert!(result.published);
        assert_eq!(result.notes.len(), 1);
    }
}
