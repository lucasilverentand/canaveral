//! Version strategy traits

use canaveral_core::error::Result;

use crate::types::{BumpType, VersionComponents};

/// Trait for version strategies
pub trait VersionStrategy: Send + Sync {
    /// Get the name of this strategy
    fn name(&self) -> &'static str;

    /// Parse a version string into components
    fn parse(&self, version: &str) -> Result<VersionComponents>;

    /// Format version components into a string
    fn format(&self, components: &VersionComponents) -> String;

    /// Bump the version according to the bump type
    fn bump(&self, current: &VersionComponents, bump_type: BumpType) -> Result<VersionComponents>;

    /// Determine the bump type from commit metadata
    fn determine_bump_type(&self, is_breaking: bool, is_feature: bool, is_fix: bool) -> BumpType {
        if is_breaking {
            BumpType::Major
        } else if is_feature {
            BumpType::Minor
        } else if is_fix {
            BumpType::Patch
        } else {
            BumpType::None
        }
    }

    /// Check if a version string is valid for this strategy
    fn is_valid(&self, version: &str) -> bool {
        self.parse(version).is_ok()
    }

    /// Compare two versions (returns -1, 0, or 1)
    fn compare(&self, a: &str, b: &str) -> Result<std::cmp::Ordering>;
}
