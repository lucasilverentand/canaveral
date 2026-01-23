# Phase 3: Monorepo Support

**Goal**: Handle multi-package repositories with coordinated or independent versioning.

## Tasks

### 3.1 Package Discovery

- [ ] Detect monorepo structure from config
- [ ] Support glob patterns for package paths
- [ ] Handle npm workspaces
- [ ] Handle Cargo workspaces
- [ ] Handle Python monorepo patterns

**Monorepo module:**
```
crates/canaveral-core/src/monorepo/
├── mod.rs
├── discovery.rs       # Find packages
├── workspace.rs       # Workspace config parsing
├── changes.rs         # Change detection
├── graph.rs           # Dependency graph
└── types.rs
```

**Discovery implementation:**
```rust
// crates/canaveral-core/src/monorepo/discovery.rs
use glob::glob;
use std::path::{Path, PathBuf};
use anyhow::Result;

#[derive(Debug, Clone)]
pub struct DiscoveredPackage {
    pub name: String,
    pub path: PathBuf,
    pub adapter: String,
    pub version: String,
    pub dependencies: Vec<String>,
}

pub async fn discover_packages(
    root: &Path,
    config: &MonorepoConfig,
) -> Result<Vec<DiscoveredPackage>> {
    let mut packages = Vec::new();

    // From explicit config patterns
    for pattern in &config.packages {
        let full_pattern = root.join(pattern).to_string_lossy().to_string();
        for entry in glob(&full_pattern)? {
            let path = entry?;
            if let Some(pkg) = detect_package(&path).await? {
                packages.push(pkg);
            }
        }
    }

    // Auto-detect workspace configs
    packages.extend(detect_npm_workspace(root).await?);
    packages.extend(detect_cargo_workspace(root).await?);

    // Deduplicate by path
    packages.sort_by(|a, b| a.path.cmp(&b.path));
    packages.dedup_by(|a, b| a.path == b.path);

    Ok(packages)
}

async fn detect_cargo_workspace(root: &Path) -> Result<Vec<DiscoveredPackage>> {
    let cargo_toml = root.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Ok(vec![]);
    }

    let content = tokio::fs::read_to_string(&cargo_toml).await?;
    let doc: toml_edit::Document = content.parse()?;

    let members = doc.get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
        .map(|arr| arr.iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>())
        .unwrap_or_default();

    let mut packages = Vec::new();
    for pattern in members {
        let full_pattern = root.join(pattern).to_string_lossy().to_string();
        for entry in glob(&full_pattern)? {
            let path = entry?;
            if path.join("Cargo.toml").exists() {
                if let Some(pkg) = detect_package(&path).await? {
                    packages.push(pkg);
                }
            }
        }
    }

    Ok(packages)
}
```

### 3.2 Change Detection

- [ ] Get changed files since last release
- [ ] Map files to packages
- [ ] Handle shared code changes
- [ ] Support ignore patterns
- [ ] Detect transitive changes

**Change detection:**
```rust
// crates/canaveral-core/src/monorepo/changes.rs
use crate::git::GitRepo;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct PackageChange {
    pub package: DiscoveredPackage,
    pub change_type: ChangeType,
    pub files: Vec<PathBuf>,
    pub commits: Vec<ParsedCommit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeType {
    Direct,      // Files in package changed
    Dependency,  // Internal dependency changed
    Shared,      // Shared code changed
}

pub async fn detect_changes(
    git: &GitRepo,
    packages: &[DiscoveredPackage],
    since: &str,
    ignore_patterns: &[String],
) -> Result<Vec<PackageChange>> {
    // Get all changed files since last tag/commit
    let changed_files = git.diff_files(since, "HEAD")?;

    // Filter out ignored patterns
    let changed_files: Vec<_> = changed_files
        .into_iter()
        .filter(|f| !matches_any_pattern(f, ignore_patterns))
        .collect();

    let mut changes = Vec::new();
    let mut changed_package_names = HashSet::new();

    // Direct changes
    for pkg in packages {
        let pkg_files: Vec<_> = changed_files
            .iter()
            .filter(|f| f.starts_with(&pkg.path))
            .cloned()
            .collect();

        if !pkg_files.is_empty() {
            let commits = git.commits_for_files(&pkg_files, since)?;
            changes.push(PackageChange {
                package: pkg.clone(),
                change_type: ChangeType::Direct,
                files: pkg_files,
                commits,
            });
            changed_package_names.insert(pkg.name.clone());
        }
    }

    // Dependency changes (packages that depend on changed packages)
    for pkg in packages {
        if changed_package_names.contains(&pkg.name) {
            continue;
        }

        let has_changed_dep = pkg.dependencies.iter()
            .any(|dep| changed_package_names.contains(dep));

        if has_changed_dep {
            changes.push(PackageChange {
                package: pkg.clone(),
                change_type: ChangeType::Dependency,
                files: vec![],
                commits: vec![],
            });
        }
    }

    Ok(changes)
}
```

### 3.3 Independent Versioning

- [ ] Track version per package
- [ ] Generate per-package changelogs
- [ ] Create package-specific tags
- [ ] Support selective releasing
- [ ] Handle package filtering

**Tag format:** `@scope/package@1.2.3` or `package@1.2.3`

```rust
// crates/canaveral-core/src/monorepo/versioning.rs

pub struct IndependentVersioning;

impl IndependentVersioning {
    pub fn tag_name(&self, package: &str, version: &str) -> String {
        format!("{}@{}", package, version)
    }

    pub fn parse_tag(&self, tag: &str) -> Option<(String, String)> {
        let parts: Vec<_> = tag.rsplitn(2, '@').collect();
        if parts.len() == 2 {
            Some((parts[1].to_string(), parts[0].to_string()))
        } else {
            None
        }
    }

    pub async fn release_package(
        &self,
        ctx: &ReleaseContext,
        package: &DiscoveredPackage,
        changes: &PackageChange,
    ) -> Result<ReleaseResult> {
        let release_type = determine_release_type(&changes.commits);
        let current = ctx.adapter.read_version(&package.path).await?;
        let current_version: semver::Version = current.parse()?;
        let new_version = ctx.strategy.bump(&current_version, release_type);

        // Update version
        ctx.adapter.write_version(&package.path, &new_version.to_string()).await?;

        // Generate changelog for this package
        let changelog_path = package.path.join("CHANGELOG.md");
        let changelog_entry = ctx.changelog.generate(&changes.commits, &new_version.to_string());
        prepend_changelog(&changelog_path, &changelog_entry).await?;

        // Commit
        let message = format!("chore(release): {}@{}", package.name, new_version);
        ctx.git.commit(&message, &[&package.path])?;

        // Tag
        let tag = self.tag_name(&package.name, &new_version.to_string());
        ctx.git.create_tag(&tag, &format!("Release {} {}", package.name, new_version))?;

        Ok(ReleaseResult {
            package: package.name.clone(),
            previous_version: current,
            new_version: new_version.to_string(),
            tag,
        })
    }
}
```

### 3.4 Fixed Versioning

- [ ] Single version for all packages
- [ ] Unified changelog at root
- [ ] Single version tag
- [ ] All-or-nothing release

```rust
// crates/canaveral-core/src/monorepo/versioning.rs

pub struct FixedVersioning {
    version_file: PathBuf,
}

impl FixedVersioning {
    pub fn tag_name(&self, version: &str, prefix: &str) -> String {
        format!("{}{}", prefix, version)
    }

    pub async fn sync_versions(
        &self,
        packages: &[DiscoveredPackage],
        version: &str,
        adapters: &AdapterRegistry,
    ) -> Result<()> {
        for pkg in packages {
            let adapter = adapters.get(&pkg.adapter)?;
            adapter.write_version(&pkg.path, version).await?;

            // Update internal dependency versions
            for dep_name in &pkg.dependencies {
                if let Some(dep_pkg) = packages.iter().find(|p| &p.name == dep_name) {
                    adapter.write_dependency_version(&pkg.path, dep_name, version).await?;
                }
            }
        }
        Ok(())
    }
}
```

### 3.5 Dependency Graph

- [ ] Build internal dependency graph
- [ ] Detect circular dependencies
- [ ] Topological sort for publish order
- [ ] Auto-bump dependent packages

```rust
// crates/canaveral-core/src/monorepo/graph.rs
use std::collections::{HashMap, HashSet, VecDeque};

pub struct DependencyGraph {
    nodes: HashMap<String, DiscoveredPackage>,
    edges: HashMap<String, Vec<String>>,  // package -> dependencies
}

impl DependencyGraph {
    pub fn build(packages: &[DiscoveredPackage]) -> Self {
        let mut graph = Self {
            nodes: HashMap::new(),
            edges: HashMap::new(),
        };

        for pkg in packages {
            graph.nodes.insert(pkg.name.clone(), pkg.clone());
            graph.edges.insert(pkg.name.clone(), pkg.dependencies.clone());
        }

        graph
    }

    /// Topological sort for publish order (dependencies first)
    pub fn topological_sort(&self) -> Result<Vec<String>, GraphError> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut reverse_edges: HashMap<String, Vec<String>> = HashMap::new();

        // Initialize
        for name in self.nodes.keys() {
            in_degree.insert(name.clone(), 0);
            reverse_edges.insert(name.clone(), vec![]);
        }

        // Calculate in-degrees
        for (name, deps) in &self.edges {
            for dep in deps {
                if self.nodes.contains_key(dep) {
                    *in_degree.get_mut(name).unwrap() += 1;
                    reverse_edges.get_mut(dep).unwrap().push(name.clone());
                }
            }
        }

        // Kahn's algorithm
        let mut queue: VecDeque<String> = in_degree
            .iter()
            .filter(|(_, &degree)| degree == 0)
            .map(|(name, _)| name.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(node) = queue.pop_front() {
            result.push(node.clone());

            for dependent in &reverse_edges[&node] {
                let degree = in_degree.get_mut(dependent).unwrap();
                *degree -= 1;
                if *degree == 0 {
                    queue.push_back(dependent.clone());
                }
            }
        }

        if result.len() != self.nodes.len() {
            return Err(GraphError::CircularDependency);
        }

        Ok(result)
    }

    /// Get packages that depend on the given package
    pub fn dependents(&self, package: &str) -> Vec<&DiscoveredPackage> {
        self.nodes
            .values()
            .filter(|pkg| pkg.dependencies.contains(&package.to_string()))
            .collect()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GraphError {
    #[error("Circular dependency detected in package graph")]
    CircularDependency,
}
```

### 3.6 Filtering & Selection

- [ ] Filter by package name
- [ ] Filter by glob pattern
- [ ] Filter by changed status
- [ ] Exclude specific packages

```rust
// crates/canaveral-core/src/monorepo/filter.rs
use glob::Pattern;

#[derive(Debug, Clone, Default)]
pub struct FilterOptions {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub changed_only: bool,
}

pub fn filter_packages(
    packages: &[DiscoveredPackage],
    options: &FilterOptions,
    changes: Option<&[PackageChange]>,
) -> Vec<DiscoveredPackage> {
    let mut filtered: Vec<_> = packages.to_vec();

    // Apply include filters
    if !options.include.is_empty() {
        filtered.retain(|pkg| {
            options.include.iter().any(|pattern| {
                matches_pattern(&pkg.name, pattern) ||
                matches_pattern(&pkg.path.to_string_lossy(), pattern)
            })
        });
    }

    // Apply exclude filters
    filtered.retain(|pkg| {
        !options.exclude.iter().any(|pattern| {
            matches_pattern(&pkg.name, pattern) ||
            matches_pattern(&pkg.path.to_string_lossy(), pattern)
        })
    });

    // Filter by changed status
    if options.changed_only {
        if let Some(changes) = changes {
            let changed_names: HashSet<_> = changes
                .iter()
                .map(|c| &c.package.name)
                .collect();

            filtered.retain(|pkg| changed_names.contains(&pkg.name));
        }
    }

    filtered
}

fn matches_pattern(text: &str, pattern: &str) -> bool {
    // Exact match
    if text == pattern {
        return true;
    }

    // Glob pattern
    if let Ok(glob) = Pattern::new(pattern) {
        return glob.matches(text);
    }

    false
}
```

### 3.7 Coordinated Publishing

- [ ] Publish in dependency order
- [ ] Handle partial failures
- [ ] Retry failed publishes
- [ ] Report publish status

```rust
// crates/canaveral-core/src/monorepo/publish.rs

pub async fn publish_packages(
    packages: &[DiscoveredPackage],
    graph: &DependencyGraph,
    adapters: &AdapterRegistry,
    options: &PublishOptions,
) -> Result<PublishReport> {
    let order = graph.topological_sort()?;

    let mut report = PublishReport {
        success: vec![],
        failed: vec![],
        skipped: vec![],
    };

    for package_name in order {
        let pkg = packages.iter().find(|p| p.name == package_name).unwrap();
        let adapter = adapters.get(&pkg.adapter)?;

        if options.dry_run {
            tracing::info!("Would publish: {}@{}", pkg.name, pkg.version);
            report.skipped.push(pkg.clone());
            continue;
        }

        match adapter.publish(&PublishOptions {
            path: pkg.path.clone(),
            version: pkg.version.clone(),
            dry_run: false,
            registry: options.registry.clone(),
            tag: options.tag.clone(),
            access: options.access,
        }).await {
            Ok(result) if result.success => {
                tracing::info!("Published: {}@{}", pkg.name, pkg.version);
                report.success.push((pkg.clone(), result));
            }
            Ok(result) => {
                tracing::error!("Failed to publish {}: {:?}", pkg.name, result.error);
                report.failed.push((pkg.clone(), result.error));

                if !options.force {
                    tracing::error!("Stopping due to publish failure");
                    break;
                }
            }
            Err(e) => {
                tracing::error!("Failed to publish {}: {}", pkg.name, e);
                report.failed.push((pkg.clone(), Some(e.to_string())));

                if !options.force {
                    break;
                }
            }
        }
    }

    Ok(report)
}
```

## Testing Strategy

### Unit Tests
- Package discovery from various structures
- Change detection accuracy
- Dependency graph building
- Topological sorting
- Filter logic

### Integration Tests
- npm workspaces
- Cargo workspaces
- Mixed ecosystem monorepo
- 20+ package monorepo

### Test fixtures
```
tests/fixtures/monorepos/
├── npm-workspaces/
├── cargo-workspace/
├── mixed-ecosystem/
└── large-monorepo/
```

## Definition of Done

Phase 3 is complete when:

1. [ ] Packages are discovered from config and workspaces
2. [ ] Changed packages are detected accurately
3. [ ] Independent versioning creates per-package tags
4. [ ] Fixed versioning keeps versions in sync
5. [ ] Dependency graph is built correctly
6. [ ] Publish order respects dependencies
7. [ ] `--filter` works with names, globs, and changed
8. [ ] Internal dependency versions are updated
9. [ ] Works with 10+ packages
10. [ ] Mixed npm/Cargo/Python monorepo works
