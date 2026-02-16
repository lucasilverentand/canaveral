//! Version workflow operations

use tracing::{debug, info};

use crate::config::Config;
use crate::error::Result;
use crate::types::ReleaseType;

/// Options for version calculation
#[derive(Debug, Clone)]
pub struct VersionOptions {
    /// Force a specific release type
    pub release_type: Option<ReleaseType>,
    /// Pre-release identifier
    pub prerelease: Option<String>,
    /// Package name (for monorepos)
    pub package: Option<String>,
}

impl Default for VersionOptions {
    fn default() -> Self {
        Self {
            release_type: None,
            prerelease: None,
            package: None,
        }
    }
}

/// Result of version calculation
#[derive(Debug, Clone)]
pub struct VersionResult {
    /// Current version
    pub current: String,
    /// Next version
    pub next: String,
    /// Release type applied
    pub release_type: ReleaseType,
}

/// Calculate the next version based on commits and configuration
pub fn calculate_next_version(
    _config: &Config,
    current_version: &str,
    release_type: ReleaseType,
) -> Result<String> {
    debug!(current = current_version, release_type = ?release_type, "calculating next version");
    use crate::error::VersionError;
    let version = semver::Version::parse(current_version)
        .map_err(|e| VersionError::ParseFailed(current_version.to_string(), e.to_string()))?;

    let next = match release_type {
        ReleaseType::Major => semver::Version::new(version.major + 1, 0, 0),
        ReleaseType::Minor => semver::Version::new(version.major, version.minor + 1, 0),
        ReleaseType::Patch => semver::Version::new(version.major, version.minor, version.patch + 1),
        ReleaseType::Prerelease => {
            let mut v = version.clone();
            v.pre = semver::Prerelease::new("alpha.1").unwrap_or_default();
            v
        }
        ReleaseType::Custom => version,
    };

    let next_str = next.to_string();
    info!(current = current_version, next = %next_str, release_type = ?release_type, "version calculated");
    Ok(next_str)
}

/// Format a version tag based on the configuration
pub fn format_tag(config: &Config, version: &str, package: Option<&str>) -> String {
    let tag_format = if let Some(pkg) = package {
        // Check for package-specific tag format
        config
            .packages
            .iter()
            .find(|p| p.name == pkg)
            .and_then(|p| p.tag_format.clone())
            .unwrap_or_else(|| {
                if config.versioning.independent {
                    format!("{}@{{version}}", pkg)
                } else {
                    config.versioning.tag_format.clone()
                }
            })
    } else {
        config.versioning.tag_format.clone()
    };

    tag_format.replace("{version}", version)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_next_version_major() {
        let config = Config::default();
        let next = calculate_next_version(&config, "1.2.3", ReleaseType::Major).unwrap();
        assert_eq!(next, "2.0.0");
    }

    #[test]
    fn test_calculate_next_version_minor() {
        let config = Config::default();
        let next = calculate_next_version(&config, "1.2.3", ReleaseType::Minor).unwrap();
        assert_eq!(next, "1.3.0");
    }

    #[test]
    fn test_calculate_next_version_patch() {
        let config = Config::default();
        let next = calculate_next_version(&config, "1.2.3", ReleaseType::Patch).unwrap();
        assert_eq!(next, "1.2.4");
    }

    #[test]
    fn test_format_tag() {
        let config = Config::default();
        let tag = format_tag(&config, "1.0.0", None);
        assert_eq!(tag, "v1.0.0");
    }
}
