//! Version strategy types

use serde::{Deserialize, Serialize};

/// Version components
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionComponents {
    /// Major version
    pub major: u64,
    /// Minor version
    pub minor: u64,
    /// Patch version
    pub patch: u64,
    /// Pre-release identifier
    pub prerelease: Option<String>,
    /// Build metadata
    pub build: Option<String>,
}

impl VersionComponents {
    /// Create new version components
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
            prerelease: None,
            build: None,
        }
    }

    /// Set prerelease
    pub fn with_prerelease(mut self, prerelease: impl Into<String>) -> Self {
        self.prerelease = Some(prerelease.into());
        self
    }

    /// Set build metadata
    pub fn with_build(mut self, build: impl Into<String>) -> Self {
        self.build = Some(build.into());
        self
    }

    /// Convert to string representation
    pub fn to_version_string(&self) -> String {
        let mut v = format!("{}.{}.{}", self.major, self.minor, self.patch);

        if let Some(pre) = &self.prerelease {
            v.push('-');
            v.push_str(pre);
        }

        if let Some(build) = &self.build {
            v.push('+');
            v.push_str(build);
        }

        v
    }
}

impl std::fmt::Display for VersionComponents {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_version_string())
    }
}

impl TryFrom<&str> for VersionComponents {
    type Error = String;

    fn try_from(s: &str) -> Result<Self, Self::Error> {
        let version = semver::Version::parse(s).map_err(|e| e.to_string())?;

        Ok(Self {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
            prerelease: if version.pre.is_empty() {
                None
            } else {
                Some(version.pre.to_string())
            },
            build: if version.build.is_empty() {
                None
            } else {
                Some(version.build.to_string())
            },
        })
    }
}

/// Type of version bump
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BumpType {
    /// Major version bump (breaking changes)
    Major,
    /// Minor version bump (new features)
    Minor,
    /// Patch version bump (bug fixes)
    Patch,
    /// Pre-release bump
    Prerelease,
    /// No bump needed
    None,
}

impl BumpType {
    /// Get the higher priority bump type
    pub fn max(self, other: Self) -> Self {
        use BumpType::*;
        match (self, other) {
            (Major, _) | (_, Major) => Major,
            (Minor, _) | (_, Minor) => Minor,
            (Patch, _) | (_, Patch) => Patch,
            (Prerelease, _) | (_, Prerelease) => Prerelease,
            (None, None) => None,
        }
    }
}

impl std::fmt::Display for BumpType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Patch => write!(f, "patch"),
            Self::Prerelease => write!(f, "prerelease"),
            Self::None => write!(f, "none"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_components() {
        let v = VersionComponents::new(1, 2, 3);
        assert_eq!(v.to_version_string(), "1.2.3");

        let v = v.with_prerelease("alpha.1");
        assert_eq!(v.to_version_string(), "1.2.3-alpha.1");

        let v = v.with_build("build.123");
        assert_eq!(v.to_version_string(), "1.2.3-alpha.1+build.123");
    }

    #[test]
    fn test_version_parse() {
        let v = VersionComponents::try_from("1.2.3").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);

        let v = VersionComponents::try_from("2.0.0-beta.1").unwrap();
        assert_eq!(v.prerelease, Some("beta.1".to_string()));
    }

    #[test]
    fn test_bump_type_max() {
        assert_eq!(BumpType::Patch.max(BumpType::Minor), BumpType::Minor);
        assert_eq!(BumpType::Minor.max(BumpType::Major), BumpType::Major);
        assert_eq!(BumpType::None.max(BumpType::Patch), BumpType::Patch);
    }
}
