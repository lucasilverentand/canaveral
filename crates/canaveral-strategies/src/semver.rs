//! SemVer version strategy

use std::cmp::Ordering;

use canaveral_core::error::{Result, VersionError};

use crate::traits::VersionStrategy;
use crate::types::{BumpType, VersionComponents};

/// Semantic Versioning strategy
///
/// Follows the SemVer 2.0.0 specification: https://semver.org/
pub struct SemVerStrategy {
    /// Default prerelease identifier
    pub default_prerelease: String,
    /// Whether to allow versions starting with 0.x
    pub allow_zero_major: bool,
}

impl SemVerStrategy {
    /// Create a new SemVer strategy
    pub fn new() -> Self {
        Self {
            default_prerelease: "alpha".to_string(),
            allow_zero_major: true,
        }
    }

    /// Set the default prerelease identifier
    pub fn with_default_prerelease(mut self, prerelease: impl Into<String>) -> Self {
        self.default_prerelease = prerelease.into();
        self
    }

    /// Increment prerelease version
    fn increment_prerelease(&self, current: Option<&str>) -> String {
        match current {
            Some(pre) => {
                // Try to parse as "identifier.number"
                if let Some(dot_pos) = pre.rfind('.') {
                    let identifier = &pre[..dot_pos];
                    let number = &pre[dot_pos + 1..];

                    if let Ok(n) = number.parse::<u64>() {
                        return format!("{}.{}", identifier, n + 1);
                    }
                }

                // If we can't parse, just append .1
                format!("{}.1", pre)
            }
            None => format!("{}.1", self.default_prerelease),
        }
    }
}

impl Default for SemVerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionStrategy for SemVerStrategy {
    fn name(&self) -> &'static str {
        "semver"
    }

    fn parse(&self, version: &str) -> Result<VersionComponents> {
        // Strip leading 'v' if present
        let version = version.strip_prefix('v').unwrap_or(version);

        let v = semver::Version::parse(version)
            .map_err(|e| VersionError::ParseFailed(version.to_string(), e.to_string()))?;

        Ok(VersionComponents {
            major: v.major,
            minor: v.minor,
            patch: v.patch,
            prerelease: if v.pre.is_empty() {
                None
            } else {
                Some(v.pre.to_string())
            },
            build: if v.build.is_empty() {
                None
            } else {
                Some(v.build.to_string())
            },
        })
    }

    fn format(&self, components: &VersionComponents) -> String {
        components.to_version_string()
    }

    fn bump(&self, current: &VersionComponents, bump_type: BumpType) -> Result<VersionComponents> {
        let mut result = current.clone();

        match bump_type {
            BumpType::Major => {
                result.major += 1;
                result.minor = 0;
                result.patch = 0;
                result.prerelease = None;
            }
            BumpType::Minor => {
                result.minor += 1;
                result.patch = 0;
                result.prerelease = None;
            }
            BumpType::Patch => {
                // If currently a prerelease, just remove prerelease to release
                if result.prerelease.is_some() {
                    result.prerelease = None;
                } else {
                    result.patch += 1;
                }
            }
            BumpType::Prerelease => {
                // If not already a prerelease, bump patch and add prerelease
                if result.prerelease.is_none() {
                    result.patch += 1;
                }
                result.prerelease = Some(self.increment_prerelease(result.prerelease.as_deref()));
            }
            BumpType::None => {}
        }

        // Clear build metadata on bump
        result.build = None;

        Ok(result)
    }

    fn compare(&self, a: &str, b: &str) -> Result<Ordering> {
        let a = a.strip_prefix('v').unwrap_or(a);
        let b = b.strip_prefix('v').unwrap_or(b);

        let va = semver::Version::parse(a)
            .map_err(|e| VersionError::ParseFailed(a.to_string(), e.to_string()))?;
        let vb = semver::Version::parse(b)
            .map_err(|e| VersionError::ParseFailed(b.to_string(), e.to_string()))?;

        Ok(va.cmp(&vb))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let strategy = SemVerStrategy::new();
        let v = strategy.parse("1.2.3").unwrap();

        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
        assert!(v.prerelease.is_none());
    }

    #[test]
    fn test_parse_with_v_prefix() {
        let strategy = SemVerStrategy::new();
        let v = strategy.parse("v1.2.3").unwrap();

        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_parse_with_prerelease() {
        let strategy = SemVerStrategy::new();
        let v = strategy.parse("1.0.0-alpha.1").unwrap();

        assert_eq!(v.prerelease, Some("alpha.1".to_string()));
    }

    #[test]
    fn test_bump_major() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3);
        let next = strategy.bump(&current, BumpType::Major).unwrap();

        assert_eq!(next.major, 2);
        assert_eq!(next.minor, 0);
        assert_eq!(next.patch, 0);
    }

    #[test]
    fn test_bump_minor() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3);
        let next = strategy.bump(&current, BumpType::Minor).unwrap();

        assert_eq!(next.major, 1);
        assert_eq!(next.minor, 3);
        assert_eq!(next.patch, 0);
    }

    #[test]
    fn test_bump_patch() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3);
        let next = strategy.bump(&current, BumpType::Patch).unwrap();

        assert_eq!(next.major, 1);
        assert_eq!(next.minor, 2);
        assert_eq!(next.patch, 4);
    }

    #[test]
    fn test_bump_prerelease_from_release() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3);
        let next = strategy.bump(&current, BumpType::Prerelease).unwrap();

        assert_eq!(next.patch, 4);
        assert_eq!(next.prerelease, Some("alpha.1".to_string()));
    }

    #[test]
    fn test_bump_prerelease_increment() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3).with_prerelease("alpha.1");
        let next = strategy.bump(&current, BumpType::Prerelease).unwrap();

        assert_eq!(next.prerelease, Some("alpha.2".to_string()));
    }

    #[test]
    fn test_release_from_prerelease() {
        let strategy = SemVerStrategy::new();
        let current = VersionComponents::new(1, 2, 3).with_prerelease("alpha.1");
        let next = strategy.bump(&current, BumpType::Patch).unwrap();

        assert_eq!(next.patch, 3);
        assert!(next.prerelease.is_none());
    }

    #[test]
    fn test_compare() {
        let strategy = SemVerStrategy::new();

        assert_eq!(
            strategy.compare("1.0.0", "1.0.1").unwrap(),
            Ordering::Less
        );
        assert_eq!(
            strategy.compare("1.1.0", "1.0.1").unwrap(),
            Ordering::Greater
        );
        assert_eq!(
            strategy.compare("1.0.0", "1.0.0").unwrap(),
            Ordering::Equal
        );
        assert_eq!(
            strategy.compare("1.0.0-alpha", "1.0.0").unwrap(),
            Ordering::Less
        );
    }

    #[test]
    fn test_format() {
        let strategy = SemVerStrategy::new();
        let v = VersionComponents::new(1, 2, 3).with_prerelease("beta.1");

        assert_eq!(strategy.format(&v), "1.2.3-beta.1");
    }
}
