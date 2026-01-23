//! CalVer (Calendar Versioning) strategy
//!
//! CalVer uses date-based version numbers. Common formats:
//! - YYYY.MM.DD (full date)
//! - YYYY.MM.MICRO (year.month with micro version)
//! - YY.MM.MICRO (short year.month with micro version)
//! - YYYY.0M.MICRO (zero-padded month)

use chrono::{Datelike, Local};

use canaveral_core::error::{Result, VersionError};

use crate::traits::VersionStrategy;
use crate::types::{BumpType, VersionComponents};

/// CalVer format variants
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalVerFormat {
    /// YYYY.MM.MICRO (e.g., 2024.1.0)
    YearMonth,
    /// YYYY.0M.MICRO (e.g., 2024.01.0) - zero-padded month
    YearMonthPadded,
    /// YY.MM.MICRO (e.g., 24.1.0) - short year
    ShortYearMonth,
    /// YYYY.MM.DD (e.g., 2024.1.15)
    YearMonthDay,
    /// YYYY.WW.MICRO (e.g., 2024.5.0) - ISO week number
    YearWeek,
    /// YYYY.MICRO (e.g., 2024.1)
    YearMicro,
}

impl Default for CalVerFormat {
    fn default() -> Self {
        Self::YearMonth
    }
}

/// CalVer strategy for calendar-based versioning
#[derive(Debug, Clone)]
pub struct CalVerStrategy {
    /// Version format
    format: CalVerFormat,
}

impl CalVerStrategy {
    /// Create a new CalVer strategy with default format (YYYY.MM.MICRO)
    pub fn new() -> Self {
        Self {
            format: CalVerFormat::default(),
        }
    }

    /// Create with a specific format
    pub fn with_format(format: CalVerFormat) -> Self {
        Self { format }
    }

    /// Get the current date components based on format
    fn current_date_components(&self) -> (u64, u64) {
        let now = Local::now();
        let year = now.year() as u64;
        let short_year = (year % 100) as u64;

        match self.format {
            CalVerFormat::YearMonth | CalVerFormat::YearMonthPadded => {
                (year, now.month() as u64)
            }
            CalVerFormat::ShortYearMonth => {
                (short_year, now.month() as u64)
            }
            CalVerFormat::YearMonthDay => {
                (year, now.month() as u64)
            }
            CalVerFormat::YearWeek => {
                (year, now.iso_week().week() as u64)
            }
            CalVerFormat::YearMicro => {
                (year, 0)
            }
        }
    }

    /// Check if a version is from the current date period
    fn is_current_period(&self, components: &VersionComponents) -> bool {
        let (current_major, current_minor) = self.current_date_components();

        match self.format {
            CalVerFormat::YearMicro => components.major == current_major,
            _ => components.major == current_major && components.minor == current_minor,
        }
    }

    /// Parse version string based on format
    fn parse_internal(&self, version: &str) -> Result<VersionComponents> {
        let version = version.strip_prefix('v').unwrap_or(version);

        // Split on dots
        let parts: Vec<&str> = version.split('.').collect();

        match self.format {
            CalVerFormat::YearMonth | CalVerFormat::YearMonthPadded | CalVerFormat::ShortYearMonth => {
                if parts.len() < 2 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected format YYYY.MM.MICRO or similar, got: {}", version)
                    ).into());
                }

                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid year".to_string())
                })?;

                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid month".to_string())
                })?;

                let patch: u64 = parts.get(2).unwrap_or(&"0").parse().unwrap_or(0);

                Ok(VersionComponents::new(major, minor, patch))
            }
            CalVerFormat::YearMonthDay => {
                if parts.len() < 3 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected format YYYY.MM.DD, got: {}", version)
                    ).into());
                }

                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid year".to_string())
                })?;

                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid month".to_string())
                })?;

                let patch: u64 = parts[2].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid day".to_string())
                })?;

                Ok(VersionComponents::new(major, minor, patch))
            }
            CalVerFormat::YearWeek => {
                if parts.len() < 2 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected format YYYY.WW.MICRO, got: {}", version)
                    ).into());
                }

                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid year".to_string())
                })?;

                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid week".to_string())
                })?;

                let patch: u64 = parts.get(2).unwrap_or(&"0").parse().unwrap_or(0);

                Ok(VersionComponents::new(major, minor, patch))
            }
            CalVerFormat::YearMicro => {
                if parts.len() < 2 {
                    return Err(VersionError::InvalidFormat(
                        format!("Expected format YYYY.MICRO, got: {}", version)
                    ).into());
                }

                let major: u64 = parts[0].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid year".to_string())
                })?;

                let minor: u64 = parts[1].parse().map_err(|_| {
                    VersionError::ParseFailed(version.to_string(), "Invalid micro".to_string())
                })?;

                Ok(VersionComponents::new(major, minor, 0))
            }
        }
    }
}

impl Default for CalVerStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl VersionStrategy for CalVerStrategy {
    fn name(&self) -> &'static str {
        "calver"
    }

    fn parse(&self, version: &str) -> Result<VersionComponents> {
        self.parse_internal(version)
    }

    fn format(&self, components: &VersionComponents) -> String {
        match self.format {
            CalVerFormat::YearMonth => {
                format!("{}.{}.{}", components.major, components.minor, components.patch)
            }
            CalVerFormat::YearMonthPadded => {
                format!("{}.{:02}.{}", components.major, components.minor, components.patch)
            }
            CalVerFormat::ShortYearMonth => {
                format!("{}.{}.{}", components.major, components.minor, components.patch)
            }
            CalVerFormat::YearMonthDay => {
                format!("{}.{}.{}", components.major, components.minor, components.patch)
            }
            CalVerFormat::YearWeek => {
                format!("{}.{}.{}", components.major, components.minor, components.patch)
            }
            CalVerFormat::YearMicro => {
                format!("{}.{}", components.major, components.minor)
            }
        }
    }

    fn bump(&self, current: &VersionComponents, bump_type: BumpType) -> Result<VersionComponents> {
        let (date_major, date_minor) = self.current_date_components();

        match self.format {
            CalVerFormat::YearMonthDay => {
                // For YYYY.MM.DD, just return today's date
                let now = Local::now();
                Ok(VersionComponents::new(
                    now.year() as u64,
                    now.month() as u64,
                    now.day() as u64,
                ))
            }
            CalVerFormat::YearMicro => {
                // For YYYY.MICRO, increment micro or reset if year changed
                if current.major == date_major {
                    Ok(VersionComponents::new(date_major, current.minor + 1, 0))
                } else {
                    Ok(VersionComponents::new(date_major, 0, 0))
                }
            }
            _ => {
                // For other formats, check if we're in a new period
                if self.is_current_period(current) {
                    // Same period, increment micro version
                    match bump_type {
                        BumpType::Major | BumpType::Minor => {
                            // Major/minor bumps in CalVer just increment micro
                            Ok(VersionComponents::new(
                                current.major,
                                current.minor,
                                current.patch + 1,
                            ))
                        }
                        _ => {
                            Ok(VersionComponents::new(
                                current.major,
                                current.minor,
                                current.patch + 1,
                            ))
                        }
                    }
                } else {
                    // New period, reset micro to 0
                    Ok(VersionComponents::new(date_major, date_minor, 0))
                }
            }
        }
    }

    fn determine_bump_type(&self, _is_breaking: bool, _is_feature: bool, _is_fix: bool) -> BumpType {
        // CalVer doesn't use semantic bump types
        // Always return Patch to trigger a version increment
        BumpType::Patch
    }

    fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering> {
        let va = self.parse(a)?;
        let vb = self.parse(b)?;

        Ok(va.major.cmp(&vb.major)
            .then(va.minor.cmp(&vb.minor))
            .then(va.patch.cmp(&vb.patch)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_year_month() {
        let strategy = CalVerStrategy::new();

        let v = strategy.parse("2024.1.0").unwrap();
        assert_eq!(v.major, 2024);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 0);

        let v = strategy.parse("2024.12.5").unwrap();
        assert_eq!(v.minor, 12);
        assert_eq!(v.patch, 5);
    }

    #[test]
    fn test_parse_short_year() {
        let strategy = CalVerStrategy::with_format(CalVerFormat::ShortYearMonth);

        let v = strategy.parse("24.1.0").unwrap();
        assert_eq!(v.major, 24);
        assert_eq!(v.minor, 1);
    }

    #[test]
    fn test_parse_year_month_day() {
        let strategy = CalVerStrategy::with_format(CalVerFormat::YearMonthDay);

        let v = strategy.parse("2024.1.15").unwrap();
        assert_eq!(v.major, 2024);
        assert_eq!(v.minor, 1);
        assert_eq!(v.patch, 15);
    }

    #[test]
    fn test_format() {
        let strategy = CalVerStrategy::new();
        let v = VersionComponents::new(2024, 1, 5);
        assert_eq!(strategy.format(&v), "2024.1.5");

        let padded_strategy = CalVerStrategy::with_format(CalVerFormat::YearMonthPadded);
        assert_eq!(padded_strategy.format(&v), "2024.01.5");
    }

    #[test]
    fn test_bump_same_period() {
        let strategy = CalVerStrategy::new();
        let (year, month) = strategy.current_date_components();

        let current = VersionComponents::new(year, month, 5);
        let bumped = strategy.bump(&current, BumpType::Patch).unwrap();

        assert_eq!(bumped.major, year);
        assert_eq!(bumped.minor, month);
        assert_eq!(bumped.patch, 6);
    }

    #[test]
    fn test_bump_new_period() {
        let strategy = CalVerStrategy::new();

        // Old version from a previous month
        let current = VersionComponents::new(2020, 1, 99);
        let bumped = strategy.bump(&current, BumpType::Patch).unwrap();

        // Should reset to current date with micro 0
        let (year, month) = strategy.current_date_components();
        assert_eq!(bumped.major, year);
        assert_eq!(bumped.minor, month);
        assert_eq!(bumped.patch, 0);
    }

    #[test]
    fn test_compare() {
        let strategy = CalVerStrategy::new();

        assert_eq!(
            strategy.compare("2024.1.0", "2024.1.1").unwrap(),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            strategy.compare("2024.2.0", "2024.1.5").unwrap(),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            strategy.compare("2024.1.0", "2024.1.0").unwrap(),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_year_micro_format() {
        let strategy = CalVerStrategy::with_format(CalVerFormat::YearMicro);

        let v = strategy.parse("2024.5").unwrap();
        assert_eq!(v.major, 2024);
        assert_eq!(v.minor, 5);

        let formatted = strategy.format(&v);
        assert_eq!(formatted, "2024.5");
    }
}
