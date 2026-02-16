//! Change detection for monorepos

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::Result;

use super::discovery::DiscoveredPackage;
use super::graph::DependencyGraph;

/// A package that has changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedPackage {
    /// Package name
    pub name: String,
    /// Package path
    pub path: PathBuf,
    /// Files that changed in this package
    pub changed_files: Vec<PathBuf>,
    /// Reason for inclusion in changes
    pub change_reason: ChangeReason,
    /// Commits that affected this package
    pub commits: Vec<String>,
}

/// Reason why a package is considered changed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeReason {
    /// Direct file changes in the package
    DirectChanges,
    /// Dependency on a changed package
    DependencyChanged(String),
    /// Forced inclusion (e.g., for fixed versioning)
    Forced,
    /// Version bump required due to conventional commit
    ConventionalCommit,
}

impl std::fmt::Display for ChangeReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DirectChanges => write!(f, "direct changes"),
            Self::DependencyChanged(dep) => write!(f, "dependency '{}' changed", dep),
            Self::Forced => write!(f, "forced"),
            Self::ConventionalCommit => write!(f, "conventional commit"),
        }
    }
}

/// Change detector for monorepos
pub struct ChangeDetector {
    /// Root path of the workspace
    root: PathBuf,
    /// Include changes from dependencies (transitive)
    include_transitive: bool,
}

impl ChangeDetector {
    /// Create a new change detector
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            include_transitive: true,
        }
    }

    /// Set whether to include transitive dependency changes
    pub fn with_transitive(mut self, include: bool) -> Self {
        self.include_transitive = include;
        self
    }

    /// Detect changed packages since a reference (tag, commit, branch)
    pub fn detect_changes(
        &self,
        packages: &[DiscoveredPackage],
        changed_files: &[PathBuf],
        graph: Option<&DependencyGraph>,
    ) -> Result<Vec<ChangedPackage>> {
        debug!(
            packages = packages.len(),
            changed_files = changed_files.len(),
            transitive = self.include_transitive,
            "detecting changed packages"
        );
        // Map files to packages
        let file_to_package = self.map_files_to_packages(packages, changed_files);

        // Build initial set of directly changed packages
        let mut changed: HashMap<String, ChangedPackage> = HashMap::new();

        for (file, pkg_name) in &file_to_package {
            let pkg = packages.iter().find(|p| &p.name == pkg_name).unwrap();

            changed
                .entry(pkg_name.clone())
                .or_insert_with(|| ChangedPackage {
                    name: pkg_name.clone(),
                    path: pkg.path.clone(),
                    changed_files: Vec::new(),
                    change_reason: ChangeReason::DirectChanges,
                    commits: Vec::new(),
                })
                .changed_files
                .push(file.clone());
        }

        // Include transitively affected packages
        if self.include_transitive {
            if let Some(graph) = graph {
                let directly_changed: HashSet<String> = changed.keys().cloned().collect();

                for pkg in packages {
                    if changed.contains_key(&pkg.name) {
                        continue;
                    }

                    // Check if any of this package's dependencies changed
                    let dependents = graph.get_dependents(&pkg.name);
                    for dep in &directly_changed {
                        if dependents.contains(dep) {
                            // This package depends on a changed package
                            let affected_pkg = packages.iter().find(|p| p.name == pkg.name).unwrap();
                            changed.insert(
                                pkg.name.clone(),
                                ChangedPackage {
                                    name: pkg.name.clone(),
                                    path: affected_pkg.path.clone(),
                                    changed_files: Vec::new(),
                                    change_reason: ChangeReason::DependencyChanged(dep.clone()),
                                    commits: Vec::new(),
                                },
                            );
                            break;
                        }
                    }
                }
            }
        }

        let result: Vec<ChangedPackage> = changed.into_values().collect();
        info!(changed_packages = result.len(), "change detection complete");
        Ok(result)
    }

    /// Map changed files to their containing packages
    fn map_files_to_packages(
        &self,
        packages: &[DiscoveredPackage],
        changed_files: &[PathBuf],
    ) -> Vec<(PathBuf, String)> {
        let mut mappings = Vec::new();

        for file in changed_files {
            // Make the file path relative to root if it's absolute
            let relative_file = if file.is_absolute() {
                file.strip_prefix(&self.root).unwrap_or(file).to_path_buf()
            } else {
                file.clone()
            };

            // Find the package that contains this file
            for pkg in packages {
                let pkg_relative_path = pkg
                    .path
                    .strip_prefix(&self.root)
                    .unwrap_or(&pkg.path);

                if relative_file.starts_with(pkg_relative_path) {
                    mappings.push((relative_file.clone(), pkg.name.clone()));
                    break;
                }
            }
        }

        mappings
    }

    /// Get files changed between two git references
    pub fn get_changed_files_git(
        &self,
        from_ref: Option<&str>,
        to_ref: &str,
    ) -> Result<Vec<PathBuf>> {
        use std::process::Command;

        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root);

        if let Some(from) = from_ref {
            cmd.args(["diff", "--name-only", from, to_ref]);
        } else {
            // All files tracked by git
            cmd.args(["ls-files"]);
        }

        let output = cmd.output().map_err(|e| {
            crate::error::GitError::OpenFailed(format!("Failed to run git: {}", e))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(crate::error::GitError::OpenFailed(stderr.to_string()).into());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<PathBuf> = stdout
            .lines()
            .filter(|line| !line.is_empty())
            .map(PathBuf::from)
            .collect();

        Ok(files)
    }

    /// Detect changes since the last tag
    pub fn detect_changes_since_tag(
        &self,
        packages: &[DiscoveredPackage],
        tag_pattern: Option<&str>,
        graph: Option<&DependencyGraph>,
    ) -> Result<Vec<ChangedPackage>> {
        use std::process::Command;

        // Find the latest tag
        let mut cmd = Command::new("git");
        cmd.current_dir(&self.root);
        cmd.args(["describe", "--tags", "--abbrev=0"]);

        if let Some(pattern) = tag_pattern {
            cmd.arg(format!("--match={}", pattern));
        }

        let output = cmd.output().map_err(|e| {
            crate::error::GitError::OpenFailed(format!("Failed to run git: {}", e))
        })?;

        let from_ref = if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Some(tag)
        } else {
            // No tags found, compare against all commits
            None
        };

        let changed_files = self.get_changed_files_git(from_ref.as_deref(), "HEAD")?;
        self.detect_changes(packages, &changed_files, graph)
    }
}

/// Filter for determining which files trigger a change
#[derive(Debug, Clone)]
pub struct ChangeFilter {
    /// File patterns to include
    pub include: Vec<String>,
    /// File patterns to exclude
    pub exclude: Vec<String>,
}

impl Default for ChangeFilter {
    fn default() -> Self {
        Self {
            include: vec!["**/*".to_string()],
            exclude: vec![
                "**/*.md".to_string(),
                "**/README*".to_string(),
                "**/CHANGELOG*".to_string(),
                "**/LICENSE*".to_string(),
                "**/.gitignore".to_string(),
            ],
        }
    }
}

impl ChangeFilter {
    /// Create a new change filter
    pub fn new() -> Self {
        Self::default()
    }

    /// Add include pattern
    pub fn include(mut self, pattern: impl Into<String>) -> Self {
        self.include.push(pattern.into());
        self
    }

    /// Add exclude pattern
    pub fn exclude(mut self, pattern: impl Into<String>) -> Self {
        self.exclude.push(pattern.into());
        self
    }

    /// Check if a file matches the filter
    pub fn matches(&self, file: &Path) -> bool {
        use glob::Pattern;

        let file_str = file.to_string_lossy();

        // Check excludes first
        for pattern in &self.exclude {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&file_str) {
                    return false;
                }
            }
        }

        // Check includes
        for pattern in &self.include {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(&file_str) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_packages(temp: &TempDir) -> Vec<DiscoveredPackage> {
        vec![
            DiscoveredPackage {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                path: temp.path().join("packages/pkg-a"),
                manifest_path: temp.path().join("packages/pkg-a/package.json"),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec![],
            },
            DiscoveredPackage {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                path: temp.path().join("packages/pkg-b"),
                manifest_path: temp.path().join("packages/pkg-b/package.json"),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["pkg-a".to_string()],
            },
        ]
    }

    #[test]
    fn test_map_files_to_packages() {
        let temp = TempDir::new().unwrap();
        let packages = create_test_packages(&temp);

        std::fs::create_dir_all(temp.path().join("packages/pkg-a")).unwrap();
        std::fs::create_dir_all(temp.path().join("packages/pkg-b")).unwrap();

        let detector = ChangeDetector::new(temp.path().to_path_buf());

        let changed_files = vec![
            PathBuf::from("packages/pkg-a/src/index.js"),
            PathBuf::from("packages/pkg-b/src/utils.js"),
        ];

        let mappings = detector.map_files_to_packages(&packages, &changed_files);

        assert_eq!(mappings.len(), 2);
        assert!(mappings.iter().any(|(_, name)| name == "pkg-a"));
        assert!(mappings.iter().any(|(_, name)| name == "pkg-b"));
    }

    #[test]
    fn test_detect_direct_changes() {
        let temp = TempDir::new().unwrap();
        let packages = create_test_packages(&temp);

        std::fs::create_dir_all(temp.path().join("packages/pkg-a")).unwrap();
        std::fs::create_dir_all(temp.path().join("packages/pkg-b")).unwrap();

        let detector = ChangeDetector::new(temp.path().to_path_buf()).with_transitive(false);

        let changed_files = vec![PathBuf::from("packages/pkg-a/src/index.js")];

        let changes = detector.detect_changes(&packages, &changed_files, None).unwrap();

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "pkg-a");
        assert_eq!(changes[0].change_reason, ChangeReason::DirectChanges);
    }

    #[test]
    fn test_change_filter() {
        let filter = ChangeFilter::default();

        assert!(filter.matches(Path::new("src/index.js")));
        assert!(filter.matches(Path::new("lib/utils.rs")));
        assert!(!filter.matches(Path::new("README.md")));
        assert!(!filter.matches(Path::new("CHANGELOG.md")));
    }

    #[test]
    fn test_change_reason_display() {
        assert_eq!(ChangeReason::DirectChanges.to_string(), "direct changes");
        assert_eq!(
            ChangeReason::DependencyChanged("pkg-a".to_string()).to_string(),
            "dependency 'pkg-a' changed"
        );
    }
}
