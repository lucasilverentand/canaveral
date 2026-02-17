//! Versioning modes for monorepos

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;
use crate::types::ReleaseType;

use super::changes::ChangedPackage;
use super::discovery::DiscoveredPackage;
use super::graph::DependencyGraph;

/// Versioning mode for the monorepo
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersioningMode {
    /// Each package has its own version
    Independent,
    /// All packages share the same version
    Fixed,
    /// Hybrid: groups of packages share versions
    Grouped,
}

impl Default for VersioningMode {
    fn default() -> Self {
        Self::Independent
    }
}

impl std::fmt::Display for VersioningMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Independent => write!(f, "independent"),
            Self::Fixed => write!(f, "fixed"),
            Self::Grouped => write!(f, "grouped"),
        }
    }
}

/// A version bump to be applied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionBump {
    /// Package name
    pub package: String,
    /// Current version
    pub current_version: String,
    /// New version
    pub new_version: String,
    /// Type of release
    pub release_type: ReleaseType,
    /// Reason for the bump
    pub reason: String,
}

/// Strategy for versioning packages
pub struct VersioningStrategy {
    /// Versioning mode
    mode: VersioningMode,
    /// Version groups (for grouped mode)
    groups: HashMap<String, Vec<String>>,
    /// Whether to sync peer dependencies
    sync_peer_deps: bool,
    /// Whether to bump dependents when a dependency changes
    bump_dependents: bool,
}

impl VersioningStrategy {
    /// Create a new versioning strategy
    pub fn new(mode: VersioningMode) -> Self {
        Self {
            mode,
            groups: HashMap::new(),
            sync_peer_deps: true,
            bump_dependents: false,
        }
    }

    /// Set version groups for grouped mode
    pub fn with_groups(mut self, groups: HashMap<String, Vec<String>>) -> Self {
        self.groups = groups;
        self
    }

    /// Set whether to sync peer dependencies
    pub fn sync_peer_deps(mut self, sync: bool) -> Self {
        self.sync_peer_deps = sync;
        self
    }

    /// Set whether to bump dependents
    pub fn bump_dependents(mut self, bump: bool) -> Self {
        self.bump_dependents = bump;
        self
    }

    /// Calculate version bumps for changed packages
    pub fn calculate_bumps(
        &self,
        packages: &[DiscoveredPackage],
        changes: &[ChangedPackage],
        release_type: ReleaseType,
        graph: Option<&DependencyGraph>,
    ) -> Result<Vec<VersionBump>> {
        info!(
            mode = %self.mode,
            changes = changes.len(),
            release_type = ?release_type,
            "calculating version bumps"
        );
        let result = match self.mode {
            VersioningMode::Independent => {
                self.calculate_independent_bumps(packages, changes, release_type, graph)
            }
            VersioningMode::Fixed => self.calculate_fixed_bumps(packages, changes, release_type),
            VersioningMode::Grouped => {
                self.calculate_grouped_bumps(packages, changes, release_type, graph)
            }
        };
        if let Ok(ref bumps) = result {
            for bump in bumps {
                debug!(
                    package = %bump.package,
                    from = %bump.current_version,
                    to = %bump.new_version,
                    release_type = ?bump.release_type,
                    reason = %bump.reason,
                    "version bump"
                );
            }
            info!(count = bumps.len(), "version bumps calculated");
        }
        result
    }

    /// Calculate bumps for independent versioning
    fn calculate_independent_bumps(
        &self,
        packages: &[DiscoveredPackage],
        changes: &[ChangedPackage],
        release_type: ReleaseType,
        graph: Option<&DependencyGraph>,
    ) -> Result<Vec<VersionBump>> {
        let mut bumps = Vec::new();
        let changed_names: Vec<_> = changes.iter().map(|c| c.name.clone()).collect();

        for change in changes {
            if let Some(pkg) = packages.iter().find(|p| p.name == change.name) {
                let new_version = self.bump_version(&pkg.version, release_type)?;
                bumps.push(VersionBump {
                    package: pkg.name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    release_type,
                    reason: format!("{}", change.change_reason),
                });
            }
        }

        // Optionally bump dependents
        if self.bump_dependents {
            if let Some(graph) = graph {
                for pkg in packages {
                    if changed_names.contains(&pkg.name) {
                        continue; // Already bumped
                    }

                    // Check if any dependency was bumped
                    let deps = graph.get_dependencies(&pkg.name);
                    let has_bumped_dep = deps.iter().any(|d| changed_names.contains(d));

                    if has_bumped_dep {
                        let new_version = self.bump_version(&pkg.version, ReleaseType::Patch)?;
                        bumps.push(VersionBump {
                            package: pkg.name.clone(),
                            current_version: pkg.version.clone(),
                            new_version,
                            release_type: ReleaseType::Patch,
                            reason: "dependency updated".to_string(),
                        });
                    }
                }
            }
        }

        Ok(bumps)
    }

    /// Calculate bumps for fixed versioning (all packages same version)
    fn calculate_fixed_bumps(
        &self,
        packages: &[DiscoveredPackage],
        changes: &[ChangedPackage],
        release_type: ReleaseType,
    ) -> Result<Vec<VersionBump>> {
        if changes.is_empty() {
            return Ok(Vec::new());
        }

        // Find the highest current version
        let max_version = packages
            .iter()
            .filter_map(|p| semver::Version::parse(&p.version).ok())
            .max()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "0.0.0".to_string());

        let new_version = self.bump_version(&max_version, release_type)?;

        // Bump all packages to the same version
        let bumps = packages
            .iter()
            .map(|pkg| VersionBump {
                package: pkg.name.clone(),
                current_version: pkg.version.clone(),
                new_version: new_version.clone(),
                release_type,
                reason: if changes.iter().any(|c| c.name == pkg.name) {
                    "direct changes".to_string()
                } else {
                    "fixed versioning".to_string()
                },
            })
            .collect();

        Ok(bumps)
    }

    /// Calculate bumps for grouped versioning
    fn calculate_grouped_bumps(
        &self,
        packages: &[DiscoveredPackage],
        changes: &[ChangedPackage],
        release_type: ReleaseType,
        graph: Option<&DependencyGraph>,
    ) -> Result<Vec<VersionBump>> {
        let mut bumps = Vec::new();
        let changed_names: Vec<_> = changes.iter().map(|c| c.name.clone()).collect();

        // Track which groups have changes
        let mut group_has_changes: HashMap<String, bool> = HashMap::new();

        for (group_name, members) in &self.groups {
            let has_changes = members.iter().any(|m| changed_names.contains(m));
            group_has_changes.insert(group_name.clone(), has_changes);
        }

        // Bump packages by group
        for (group_name, members) in &self.groups {
            if !group_has_changes.get(group_name).copied().unwrap_or(false) {
                continue;
            }

            // Find max version in group
            let max_version = packages
                .iter()
                .filter(|p| members.contains(&p.name))
                .filter_map(|p| semver::Version::parse(&p.version).ok())
                .max()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "0.0.0".to_string());

            let new_version = self.bump_version(&max_version, release_type)?;

            for member in members {
                if let Some(pkg) = packages.iter().find(|p| &p.name == member) {
                    bumps.push(VersionBump {
                        package: pkg.name.clone(),
                        current_version: pkg.version.clone(),
                        new_version: new_version.clone(),
                        release_type,
                        reason: if changed_names.contains(&pkg.name) {
                            "direct changes".to_string()
                        } else {
                            format!("group '{}' updated", group_name)
                        },
                    });
                }
            }
        }

        // Handle packages not in any group (independent versioning)
        for change in changes {
            let in_group = self
                .groups
                .values()
                .any(|members| members.contains(&change.name));
            if in_group {
                continue;
            }

            if let Some(pkg) = packages.iter().find(|p| p.name == change.name) {
                let new_version = self.bump_version(&pkg.version, release_type)?;
                bumps.push(VersionBump {
                    package: pkg.name.clone(),
                    current_version: pkg.version.clone(),
                    new_version,
                    release_type,
                    reason: format!("{}", change.change_reason),
                });
            }
        }

        // Handle bump_dependents for non-grouped packages
        if self.bump_dependents {
            if let Some(graph) = graph {
                let bumped_names: Vec<_> = bumps.iter().map(|b| b.package.clone()).collect();

                for pkg in packages {
                    if bumped_names.contains(&pkg.name) {
                        continue;
                    }

                    let in_group = self
                        .groups
                        .values()
                        .any(|members| members.contains(&pkg.name));
                    if in_group {
                        continue;
                    }

                    let deps = graph.get_dependencies(&pkg.name);
                    let has_bumped_dep = deps.iter().any(|d| bumped_names.contains(d));

                    if has_bumped_dep {
                        let new_version = self.bump_version(&pkg.version, ReleaseType::Patch)?;
                        bumps.push(VersionBump {
                            package: pkg.name.clone(),
                            current_version: pkg.version.clone(),
                            new_version,
                            release_type: ReleaseType::Patch,
                            reason: "dependency updated".to_string(),
                        });
                    }
                }
            }
        }

        Ok(bumps)
    }

    /// Bump a version according to release type
    fn bump_version(&self, version: &str, release_type: ReleaseType) -> Result<String> {
        let mut v = semver::Version::parse(version).map_err(|e| {
            crate::error::VersionError::ParseFailed(version.to_string(), e.to_string())
        })?;

        match release_type {
            ReleaseType::Major => {
                v.major += 1;
                v.minor = 0;
                v.patch = 0;
                v.pre = semver::Prerelease::EMPTY;
            }
            ReleaseType::Minor => {
                v.minor += 1;
                v.patch = 0;
                v.pre = semver::Prerelease::EMPTY;
            }
            ReleaseType::Patch => {
                v.patch += 1;
                v.pre = semver::Prerelease::EMPTY;
            }
            ReleaseType::Prerelease => {
                if v.pre.is_empty() {
                    v.patch += 1;
                    v.pre = semver::Prerelease::new("alpha.0").unwrap();
                } else {
                    // Increment prerelease
                    let pre_str = v.pre.to_string();
                    if let Some((prefix, num)) = pre_str.rsplit_once('.') {
                        if let Ok(n) = num.parse::<u32>() {
                            v.pre =
                                semver::Prerelease::new(&format!("{}.{}", prefix, n + 1)).unwrap();
                        }
                    }
                }
            }
            ReleaseType::Custom => {
                // No automatic bump for custom
            }
        }

        Ok(v.to_string())
    }

    /// Generate tag name for a package version
    pub fn tag_name(&self, package: &str, version: &str) -> String {
        match self.mode {
            VersioningMode::Fixed => format!("v{}", version),
            VersioningMode::Independent | VersioningMode::Grouped => {
                format!("{}@{}", package, version)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::monorepo::changes::ChangeReason;

    fn create_packages() -> Vec<DiscoveredPackage> {
        vec![
            DiscoveredPackage {
                name: "core".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/core".into(),
                manifest_path: "packages/core/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec![],
            },
            DiscoveredPackage {
                name: "utils".to_string(),
                version: "1.2.0".to_string(),
                path: "packages/utils".into(),
                manifest_path: "packages/utils/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string()],
            },
            DiscoveredPackage {
                name: "cli".to_string(),
                version: "2.0.0".to_string(),
                path: "packages/cli".into(),
                manifest_path: "packages/cli/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string(), "utils".to_string()],
            },
        ]
    }

    fn create_changes() -> Vec<ChangedPackage> {
        vec![ChangedPackage {
            name: "core".to_string(),
            path: "packages/core".into(),
            changed_files: vec!["packages/core/src/index.ts".into()],
            change_reason: ChangeReason::DirectChanges,
            commits: vec![],
        }]
    }

    #[test]
    fn test_independent_versioning() {
        let packages = create_packages();
        let changes = create_changes();

        let strategy = VersioningStrategy::new(VersioningMode::Independent);
        let bumps = strategy
            .calculate_bumps(&packages, &changes, ReleaseType::Minor, None)
            .unwrap();

        assert_eq!(bumps.len(), 1);
        assert_eq!(bumps[0].package, "core");
        assert_eq!(bumps[0].new_version, "1.1.0");
    }

    #[test]
    fn test_fixed_versioning() {
        let packages = create_packages();
        let changes = create_changes();

        let strategy = VersioningStrategy::new(VersioningMode::Fixed);
        let bumps = strategy
            .calculate_bumps(&packages, &changes, ReleaseType::Minor, None)
            .unwrap();

        // All packages should be bumped to the same version
        assert_eq!(bumps.len(), 3);

        let new_version = &bumps[0].new_version;
        assert!(bumps.iter().all(|b| &b.new_version == new_version));
        assert_eq!(new_version, "2.1.0"); // Max was 2.0.0, minor bump = 2.1.0
    }

    #[test]
    fn test_grouped_versioning() {
        let packages = create_packages();
        let changes = create_changes();

        let mut groups = HashMap::new();
        groups.insert(
            "core-group".to_string(),
            vec!["core".to_string(), "utils".to_string()],
        );

        let strategy = VersioningStrategy::new(VersioningMode::Grouped).with_groups(groups);

        let bumps = strategy
            .calculate_bumps(&packages, &changes, ReleaseType::Minor, None)
            .unwrap();

        // core and utils should be in the same group and bumped together
        let core_bump = bumps.iter().find(|b| b.package == "core").unwrap();
        let utils_bump = bumps.iter().find(|b| b.package == "utils").unwrap();

        assert_eq!(core_bump.new_version, utils_bump.new_version);
        assert_eq!(core_bump.new_version, "1.3.0"); // Max in group was 1.2.0

        // cli should not be bumped (not in group, no direct changes)
        assert!(!bumps.iter().any(|b| b.package == "cli"));
    }

    #[test]
    fn test_tag_names() {
        let independent = VersioningStrategy::new(VersioningMode::Independent);
        let fixed = VersioningStrategy::new(VersioningMode::Fixed);

        assert_eq!(independent.tag_name("core", "1.0.0"), "core@1.0.0");
        assert_eq!(fixed.tag_name("core", "1.0.0"), "v1.0.0");
    }

    #[test]
    fn test_bump_version() {
        let strategy = VersioningStrategy::new(VersioningMode::Independent);

        assert_eq!(
            strategy.bump_version("1.0.0", ReleaseType::Major).unwrap(),
            "2.0.0"
        );
        assert_eq!(
            strategy.bump_version("1.0.0", ReleaseType::Minor).unwrap(),
            "1.1.0"
        );
        assert_eq!(
            strategy.bump_version("1.0.0", ReleaseType::Patch).unwrap(),
            "1.0.1"
        );
    }
}
