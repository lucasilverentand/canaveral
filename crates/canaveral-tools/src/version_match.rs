//! Shared version matching logic for tool providers
//!
//! Uses prefix matching: `"1.2"` matches `"1.2.0"`, `"1.2.5"`, etc.

/// Check if `installed` satisfies `requested`.
///
/// Rules:
/// - `"1"` matches any `1.x.y`
/// - `"1.2"` matches any `1.2.x`
/// - `"1.2.3"` matches only `"1.2.3"` exactly
///
/// Both strings are trimmed of leading `v` prefixes before comparison.
pub fn version_satisfies(installed: &str, requested: &str) -> bool {
    let installed = installed.trim_start_matches('v');
    let requested = requested.trim_start_matches('v');

    // Exact match
    if installed == requested {
        return true;
    }

    // Prefix match: requested must be a component-aligned prefix of installed
    let prefix = format!("{}.", requested);
    installed.starts_with(&prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match() {
        assert!(version_satisfies("1.2.3", "1.2.3"));
    }

    #[test]
    fn minor_prefix_match() {
        assert!(version_satisfies("1.2.0", "1.2"));
        assert!(version_satisfies("1.2.5", "1.2"));
        assert!(version_satisfies("1.2.15", "1.2"));
    }

    #[test]
    fn major_prefix_match() {
        assert!(version_satisfies("1.0.0", "1"));
        assert!(version_satisfies("1.5.3", "1"));
        assert!(version_satisfies("1.99.99", "1"));
    }

    #[test]
    fn no_false_prefix_match() {
        // "1.2" should not match "1.20.0" via naive string prefix
        assert!(!version_satisfies("1.20.0", "1.2"));
        assert!(!version_satisfies("1.20.5", "1.2"));
    }

    #[test]
    fn no_cross_major_match() {
        assert!(!version_satisfies("2.0.0", "1"));
        assert!(!version_satisfies("2.1.3", "1.2"));
    }

    #[test]
    fn v_prefix_stripped() {
        assert!(version_satisfies("v1.2.3", "1.2.3"));
        assert!(version_satisfies("1.2.3", "v1.2.3"));
        assert!(version_satisfies("v1.2.3", "v1.2.3"));
        assert!(version_satisfies("v1.2.5", "1.2"));
    }

    #[test]
    fn different_patch_no_match() {
        assert!(!version_satisfies("1.2.4", "1.2.3"));
    }

    #[test]
    fn empty_strings() {
        // Both empty: exact match returns true
        assert!(version_satisfies("", ""));
        // Non-empty installed, empty requested: prefix "." is not a match
        assert!(!version_satisfies("1.2.3", ""));
        // Empty installed, non-empty requested: no match
        assert!(!version_satisfies("", "1"));
    }

    #[test]
    fn zero_versions() {
        assert!(version_satisfies("0.1.0", "0"));
        assert!(version_satisfies("0.1.0", "0.1"));
        assert!(version_satisfies("0.0.0", "0.0.0"));
        assert!(!version_satisfies("0.1.0", "0.2"));
    }

    #[test]
    fn large_version_numbers() {
        assert!(version_satisfies("100.200.300", "100"));
        assert!(version_satisfies("100.200.300", "100.200"));
        assert!(version_satisfies("100.200.300", "100.200.300"));
        assert!(!version_satisfies("100.200.300", "100.201"));
    }

    #[test]
    fn pre_release_not_matched_by_prefix() {
        // "1.2.3-beta" should not be matched by "1.2.3" since they differ
        assert!(!version_satisfies("1.2.3-beta", "1.2.3"));
        // but "1.2" prefix should match "1.2.3-beta" since it starts with "1.2."
        assert!(version_satisfies("1.2.3-beta", "1.2"));
    }

    #[test]
    fn single_digit_no_false_match() {
        // "1" should not match "10.0.0"
        assert!(!version_satisfies("10.0.0", "1"));
        assert!(!version_satisfies("11.0.0", "1"));
        assert!(version_satisfies("1.0.0", "1"));
    }
}
