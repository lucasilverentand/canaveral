//! Build Number versioning strategy
//!
//! Uses sequential build numbers, optionally combined with a base version.
//! Common formats:
//! - Simple: 123, 124, 125...
//! - Prefixed: 1.0.123, 1.0.124...
//! - With timestamp: 1.0.20240115.1

use chrono::{Datelike, Local};

use canaveral_core::error::{Result, VersionError};
use tracing::instrument;

use crate::traits::VersionStrategy;
use crate::types::{BumpType, VersionComponents};

/// Build number format variants
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildNumberFormat {
    /// Simple sequential number: 1, 2, 3...
    Simple,
    /// With base version: BASE.BUILD (e.g., 1.0.123)
    WithBase { major: u64, minor: u64 },
    /// With date prefix: YYYYMMDD.BUILD (e.g., 20240115.1)
    DateBuild,
    /// Full with date: MAJOR.MINOR.YYYYMMDD.BUILD
    FullDate { major: u64, minor: u64 },
}

impl Default for BuildNumberFormat {
    fn default() -> Self {
        Self::Simple
    }
}

/// Build number strategy
#[derive(Debug, Clone)]
pub struct BuildNumberStrategy {
    /// Version format
    format: BuildNumberFormat,
}

impl BuildNumberStrategy {
    /// Create a new build number strategy with simple format
    pub fn new() -> Self {
        Self {
            format: BuildNumberFormat::default(),
        }
    }

    /// Create with a base version (e.g., 1.0)
    pub fn with_base(major: u64, minor: u64) -> Self {
        Self {
            format: BuildNumberFormat::WithBase { major, minor },
        }
    }

    /// Create with date-based builds
    pub fn with_date_build() -> Self {
        Self {
            format: BuildNumberFormat::DateBuild,
        }
    }

    /// Create with full date format
    pub fn with_full_date(major: u64, minor: u64) -> Self {
        Self {
            format: BuildNumberFormat::FullDate { major, minor },
        }
    }

    /// Get today's date as a number (YYYYMMDD)
    fn date_number() -> u64 {
        let now = Local::now();
        (now.year() as u64 * 10000) + (now.month() as u64 * 100) + now.day() as u64
    }

}

impl Default for BuildNumberStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionStrategy for BuildNumberStrategy {
    fn name(&self) -> &'static str {
        "buildnum"
    }

    #[instrument(skip(self), fields(strategy = "buildnum"))]
    fn parse(&self, version: &str) -> Result<VersionComponents> {
        let parts: Vec<&str> = version.split('.').collect();

        match &self.format {
            BuildNumberFormat::Simple => {
                let build: u64 = version.parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid build number".to_string())
                })?;
                Ok(VersionComponents::new(0, 0, build))
            }
            BuildNumberFormat::WithBase { .. } => {
                if parts.len() < 3 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected MAJOR.MINOR.BUILD, got: {}", version)
                    ).into());
                }
                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid major".to_string())
                })?;
                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid minor".to_string())
                })?;
                let patch: u64 = parts[2].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid build".to_string())
                })?;
                Ok(VersionComponents::new(major, minor, patch))
            }
            BuildNumberFormat::DateBuild => {
                if parts.len() < 2 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected YYYYMMDD.BUILD, got: {}", version)
                    ).into());
                }
                let date: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid date".to_string())
                })?;
                let build: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid build".to_string())
                })?;
                // Store date in major, build in patch
                Ok(VersionComponents::new(date, 0, build))
            }
            BuildNumberFormat::FullDate { .. } => {
                if parts.len() < 4 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected MAJOR.MINOR.YYYYMMDD.BUILD, got: {}", version)
                    ).into());
                }
                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid major".to_string())
                })?;
                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid minor".to_string())
                })?;
                let date: u64 = parts[2].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid date".to_string())
                })?;
                let build: u64 = parts[3].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid build".to_string())
                })?;
                // Store date in prerelease metadata, build in patch
                let mut v = VersionComponents::new(major, minor, build);
                v.build = Some(date.to_string());
                Ok(v)
            }
        }
    }

    fn format(&self, components: &VersionComponents) -> String {
        match &self.format {
            BuildNumberFormat::Simple => {
                components.patch.to_string()
            }
            BuildNumberFormat::WithBase { major, minor } => {
                format!("{}.{}.{}", major, minor, components.patch)
            }
            BuildNumberFormat::DateBuild => {
                format!("{}.{}", components.major, components.patch)
            }
            BuildNumberFormat::FullDate { major, minor } => {
                let date = components.build.as_deref()
                    .and_then(|d| d.parse::<u64>().ok())
                    .unwrap_or_else(Self::date_number);
                format!("{}.{}.{}.{}", major, minor, date, components.patch)
            }
        }
    }

    #[instrument(skip(self), fields(strategy = "buildnum", current_build = current.patch))]
    fn bump(&self, current: &VersionComponents, _bump_type: BumpType) -> Result<VersionComponents> {
        match &self.format {
            BuildNumberFormat::Simple => {
                Ok(VersionComponents::new(0, 0, current.patch + 1))
            }
            BuildNumberFormat::WithBase { major, minor } => {
                Ok(VersionComponents::new(*major, *minor, current.patch + 1))
            }
            BuildNumberFormat::DateBuild => {
                let today = Self::date_number();
                if current.major == today {
                    // Same day, increment build
                    Ok(VersionComponents::new(today, 0, current.patch + 1))
                } else {
                    // New day, reset build to 1
                    Ok(VersionComponents::new(today, 0, 1))
                }
            }
            BuildNumberFormat::FullDate { major, minor } => {
                let today = Self::date_number();
                let current_date = current.build.as_deref()
                    .and_then(|d| d.parse::<u64>().ok())
                    .unwrap_or(0);

                let (date, build) = if current_date == today {
                    (today, current.patch + 1)
                } else {
                    (today, 1)
                };

                let mut v = VersionComponents::new(*major, *minor, build);
                v.build = Some(date.to_string());
                Ok(v)
            }
        }
    }

    fn determine_bump_type(&self, _is_breaking: bool, _is_feature: bool, _is_fix: bool) -> BumpType {
        // Build numbers always increment
        BumpType::Patch
    }

    fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering> {
        match &self.format {
            BuildNumberFormat::Simple => {
                let na: u64 = a.parse().map_err(|_| {
                    VersionError::ParseFailed(a.to_string(), "Invalid build number".to_string())
                })?;
                let nb: u64 = b.parse().map_err(|_| {
                    VersionError::ParseFailed(b.to_string(), "Invalid build number".to_string())
                })?;
                Ok(na.cmp(&nb))
            }
            BuildNumberFormat::WithBase { .. } => {
                let va = self.parse(a)?;
                let vb = self.parse(b)?;
                Ok(va.major.cmp(&vb.major)
                    .then(va.minor.cmp(&vb.minor))
                    .then(va.patch.cmp(&vb.patch)))
            }
            BuildNumberFormat::DateBuild => {
                let va = self.parse(a)?;
                let vb = self.parse(b)?;
                Ok(va.major.cmp(&vb.major) // date
                    .then(va.patch.cmp(&vb.patch))) // build
            }
            BuildNumberFormat::FullDate { .. } => {
                let va = self.parse(a)?;
                let vb = self.parse(b)?;
                let date_a = va.build.as_deref().and_then(|d| d.parse::<u64>().ok()).unwrap_or(0);
                let date_b = vb.build.as_deref().and_then(|d| d.parse::<u64>().ok()).unwrap_or(0);

                Ok(va.major.cmp(&vb.major)
                    .then(va.minor.cmp(&vb.minor))
                    .then(date_a.cmp(&date_b))
                    .then(va.patch.cmp(&vb.patch)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_format() {
        let strategy = BuildNumberStrategy::new();

        let v = strategy.parse("123").unwrap();
        assert_eq!(v.patch, 123);

        assert_eq!(strategy.format(&v), "123");

        let bumped = strategy.bump(&v, BumpType::None).unwrap();
        assert_eq!(bumped.patch, 124);
    }

    #[test]
    fn test_with_base() {
        let strategy = BuildNumberStrategy::with_base(1, 0);

        let v = strategy.parse("1.0.50").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 50);

        assert_eq!(strategy.format(&v), "1.0.50");

        let bumped = strategy.bump(&v, BumpType::Patch).unwrap();
        assert_eq!(strategy.format(&bumped), "1.0.51");
    }

    #[test]
    fn test_date_build() {
        let strategy = BuildNumberStrategy::with_date_build();
        let today = BuildNumberStrategy::date_number();

        // Parse today's version
        let version = format!("{}.5", today);
        let v = strategy.parse(&version).unwrap();
        assert_eq!(v.major, today);
        assert_eq!(v.patch, 5);

        // Bump should increment build
        let bumped = strategy.bump(&v, BumpType::Patch).unwrap();
        assert_eq!(bumped.patch, 6);

        // Parse old version
        let old_v = strategy.parse("20200101.99").unwrap();
        let bumped_old = strategy.bump(&old_v, BumpType::Patch).unwrap();
        // Should reset to today with build 1
        assert_eq!(bumped_old.major, today);
        assert_eq!(bumped_old.patch, 1);
    }

    #[test]
    fn test_compare_simple() {
        let strategy = BuildNumberStrategy::new();

        assert_eq!(
            strategy.compare("10", "20").unwrap(),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            strategy.compare("100", "99").unwrap(),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_compare_with_base() {
        let strategy = BuildNumberStrategy::with_base(1, 0);

        assert_eq!(
            strategy.compare("1.0.10", "1.0.20").unwrap(),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_full_date_format() {
        let strategy = BuildNumberStrategy::with_full_date(2, 0);
        let today = BuildNumberStrategy::date_number();

        let version = format!("2.0.{}.3", today);
        let v = strategy.parse(&version).unwrap();
        assert_eq!(v.major, 2);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 3);

        let formatted = strategy.format(&v);
        assert!(formatted.starts_with("2.0."));
        assert!(formatted.ends_with(".3"));
    }
}
