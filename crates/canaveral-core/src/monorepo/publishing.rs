//! Coordinated publishing for monorepos

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

use crate::error::Result;

use super::discovery::DiscoveredPackage;
use super::graph::DependencyGraph;
use super::versioning::VersionBump;

/// Result of publishing a single package
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackagePublishResult {
    /// Package name
    pub package: String,
    /// Whether publishing succeeded
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
    /// Time taken to publish
    pub duration: Duration,
    /// Registry URL if available
    pub registry_url: Option<String>,
}

/// Overall result of a coordinated publish
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishResult {
    /// Results for each package
    pub packages: Vec<PackagePublishResult>,
    /// Total duration
    pub total_duration: Duration,
    /// Whether all packages were published successfully
    pub success: bool,
    /// Packages that were skipped due to failures
    pub skipped: Vec<String>,
}

impl PublishResult {
    /// Get successfully published packages
    pub fn successful(&self) -> Vec<&PackagePublishResult> {
        self.packages.iter().filter(|p| p.success).collect()
    }

    /// Get failed packages
    pub fn failed(&self) -> Vec<&PackagePublishResult> {
        self.packages.iter().filter(|p| !p.success).collect()
    }
}

/// A plan for publishing packages in order
#[derive(Debug, Clone)]
pub struct PublishPlan {
    /// Packages to publish in order
    pub packages: Vec<PlannedPublish>,
    /// Total number of packages
    pub total_count: usize,
    /// Packages that will be skipped (already published, private, etc.)
    pub skipped: Vec<SkippedPackage>,
}

/// A package planned for publishing
#[derive(Debug, Clone)]
pub struct PlannedPublish {
    /// Package name
    pub name: String,
    /// Package path
    pub path: std::path::PathBuf,
    /// Version to publish
    pub version: String,
    /// Dependencies that must be published first
    pub dependencies: Vec<String>,
    /// Publish order (0 = first)
    pub order: usize,
}

/// A package that will be skipped
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkippedPackage {
    /// Package name
    pub name: String,
    /// Reason for skipping
    pub reason: SkipReason,
}

/// Reason why a package is skipped
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkipReason {
    /// Package is marked as private
    Private,
    /// Package is already published at this version
    AlreadyPublished,
    /// No version bump required
    NoChanges,
    /// Dependency failed to publish
    DependencyFailed(String),
    /// Explicitly excluded
    Excluded,
}

impl std::fmt::Display for SkipReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Private => write!(f, "package is private"),
            Self::AlreadyPublished => write!(f, "already published"),
            Self::NoChanges => write!(f, "no changes"),
            Self::DependencyFailed(dep) => write!(f, "dependency '{}' failed", dep),
            Self::Excluded => write!(f, "explicitly excluded"),
        }
    }
}

/// Strategy for handling publish failures
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailureStrategy {
    /// Stop on first failure
    #[default]
    StopOnFailure,
    /// Continue publishing packages that don't depend on failed packages
    SkipDependents,
    /// Continue publishing all packages regardless of failures
    ContinueAll,
}

/// Options for coordinated publishing
#[derive(Debug, Clone)]
pub struct PublishOptions {
    /// Strategy for handling failures
    pub failure_strategy: FailureStrategy,
    /// Dry run (don't actually publish)
    pub dry_run: bool,
    /// Packages to exclude from publishing
    pub exclude: HashSet<String>,
    /// Only publish these packages (if not empty)
    pub only: HashSet<String>,
    /// Registry to publish to (if different from default)
    pub registry: Option<String>,
    /// Retry failed publishes
    pub retry_count: usize,
    /// Delay between retries
    pub retry_delay: Duration,
    /// Delay between publishing packages (for rate limiting)
    pub publish_delay: Duration,
}

impl Default for PublishOptions {
    fn default() -> Self {
        Self {
            failure_strategy: FailureStrategy::default(),
            dry_run: false,
            exclude: HashSet::new(),
            only: HashSet::new(),
            registry: None,
            retry_count: 0,
            retry_delay: Duration::from_secs(5),
            publish_delay: Duration::from_secs(0),
        }
    }
}

/// Callback for publish events
pub trait PublishCallback: Send + Sync {
    /// Called before publishing a package
    fn on_publish_start(&self, package: &str, version: &str);

    /// Called after publishing a package
    fn on_publish_complete(&self, package: &str, result: &PackagePublishResult);

    /// Called when a package is skipped
    fn on_skip(&self, package: &str, reason: &SkipReason);
}

/// Default no-op callback
pub struct NoOpCallback;

impl PublishCallback for NoOpCallback {
    fn on_publish_start(&self, _package: &str, _version: &str) {}
    fn on_publish_complete(&self, _package: &str, _result: &PackagePublishResult) {}
    fn on_skip(&self, _package: &str, _reason: &SkipReason) {}
}

/// Registry that broadcasts publish events to multiple callbacks
pub struct PublishCallbackRegistry {
    callbacks: Vec<Arc<dyn PublishCallback>>,
}

impl PublishCallbackRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            callbacks: Vec::new(),
        }
    }

    /// Register a callback
    pub fn register<C: PublishCallback + 'static>(&mut self, callback: C) {
        self.callbacks.push(Arc::new(callback));
    }

    /// Notify all callbacks that a publish is starting
    pub fn on_publish_start(&self, package: &str, version: &str) {
        for cb in &self.callbacks {
            cb.on_publish_start(package, version);
        }
    }

    /// Notify all callbacks that a publish completed
    pub fn on_publish_complete(&self, package: &str, result: &PackagePublishResult) {
        for cb in &self.callbacks {
            cb.on_publish_complete(package, result);
        }
    }

    /// Notify all callbacks that a package was skipped
    pub fn on_skip(&self, package: &str, reason: &SkipReason) {
        for cb in &self.callbacks {
            cb.on_skip(package, reason);
        }
    }

    /// Get all registered callbacks
    pub fn all(&self) -> &[Arc<dyn PublishCallback>] {
        &self.callbacks
    }

    /// Check if the registry has no callbacks
    pub fn is_empty(&self) -> bool {
        self.callbacks.is_empty()
    }
}

impl Default for PublishCallbackRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl PublishCallback for PublishCallbackRegistry {
    fn on_publish_start(&self, package: &str, version: &str) {
        self.on_publish_start(package, version);
    }

    fn on_publish_complete(&self, package: &str, result: &PackagePublishResult) {
        self.on_publish_complete(package, result);
    }

    fn on_skip(&self, package: &str, reason: &SkipReason) {
        self.on_skip(package, reason);
    }
}

/// Publisher function type
pub type PublishFn = Box<dyn Fn(&Path, &str, &PublishOptions) -> Result<Option<String>> + Send + Sync>;

/// Coordinator for publishing packages in a monorepo
pub struct PublishCoordinator {
    /// Publish options
    options: PublishOptions,
    /// Publisher function
    publisher: Option<PublishFn>,
}

impl PublishCoordinator {
    /// Create a new publish coordinator
    pub fn new(options: PublishOptions) -> Self {
        Self {
            options,
            publisher: None,
        }
    }

    /// Set the publisher function
    pub fn with_publisher(mut self, publisher: PublishFn) -> Self {
        self.publisher = Some(publisher);
        self
    }

    /// Create a publish plan
    pub fn create_plan(
        &self,
        packages: &[DiscoveredPackage],
        bumps: &[VersionBump],
        graph: &DependencyGraph,
    ) -> Result<PublishPlan> {
        debug!(packages = packages.len(), bumps = bumps.len(), "creating publish plan");
        let bump_map: HashMap<&str, &VersionBump> =
            bumps.iter().map(|b| (b.package.as_str(), b)).collect();

        let mut planned = Vec::new();
        let mut skipped = Vec::new();
        let mut order = 0;

        // Use topological order to ensure dependencies are published first
        for name in graph.sorted() {
            let pkg = match packages.iter().find(|p| &p.name == name) {
                Some(p) => p,
                None => continue,
            };

            // Check if package should be excluded
            if self.options.exclude.contains(&pkg.name) {
                skipped.push(SkippedPackage {
                    name: pkg.name.clone(),
                    reason: SkipReason::Excluded,
                });
                continue;
            }

            // Check if we're limiting to specific packages
            if !self.options.only.is_empty() && !self.options.only.contains(&pkg.name) {
                continue;
            }

            // Check if package is private
            if pkg.private {
                skipped.push(SkippedPackage {
                    name: pkg.name.clone(),
                    reason: SkipReason::Private,
                });
                continue;
            }

            // Check if package has a version bump
            let bump = match bump_map.get(pkg.name.as_str()) {
                Some(b) => b,
                None => {
                    skipped.push(SkippedPackage {
                        name: pkg.name.clone(),
                        reason: SkipReason::NoChanges,
                    });
                    continue;
                }
            };

            // Get dependencies that are also being published
            let deps: Vec<String> = pkg
                .workspace_dependencies
                .iter()
                .filter(|d| bump_map.contains_key(d.as_str()))
                .cloned()
                .collect();

            planned.push(PlannedPublish {
                name: pkg.name.clone(),
                path: pkg.path.clone(),
                version: bump.new_version.clone(),
                dependencies: deps,
                order,
            });

            order += 1;
        }

        info!(
            to_publish = planned.len(),
            skipped = skipped.len(),
            "publish plan created"
        );
        Ok(PublishPlan {
            total_count: planned.len(),
            packages: planned,
            skipped,
        })
    }

    /// Execute a publish plan
    pub fn execute(
        &self,
        plan: &PublishPlan,
        callback: &dyn PublishCallback,
    ) -> Result<PublishResult> {
        info!(packages = plan.packages.len(), dry_run = self.options.dry_run, "executing publish plan");
        let start = Instant::now();
        let mut results = Vec::new();
        let mut failed_packages: HashSet<String> = HashSet::new();
        let mut skipped_packages: Vec<String> = Vec::new();

        for planned in &plan.packages {
            // Check if any dependency failed
            let failed_dep = planned
                .dependencies
                .iter()
                .find(|d| failed_packages.contains(*d));

            if let Some(dep) = failed_dep {
                match self.options.failure_strategy {
                    FailureStrategy::StopOnFailure => {
                        // Already stopped by previous iteration
                        skipped_packages.push(planned.name.clone());
                        callback.on_skip(
                            &planned.name,
                            &SkipReason::DependencyFailed(dep.clone()),
                        );
                        continue;
                    }
                    FailureStrategy::SkipDependents => {
                        skipped_packages.push(planned.name.clone());
                        callback.on_skip(
                            &planned.name,
                            &SkipReason::DependencyFailed(dep.clone()),
                        );
                        continue;
                    }
                    FailureStrategy::ContinueAll => {
                        // Continue anyway
                    }
                }
            }

            // Check if we should stop due to previous failure
            if !failed_packages.is_empty()
                && self.options.failure_strategy == FailureStrategy::StopOnFailure
            {
                skipped_packages.push(planned.name.clone());
                continue;
            }

            // Publish the package
            info!(package = %planned.name, version = %planned.version, "publishing package");
            callback.on_publish_start(&planned.name, &planned.version);

            let result = self.publish_package(planned);

            callback.on_publish_complete(&planned.name, &result);

            if result.success {
                debug!(package = %planned.name, duration_ms = result.duration.as_millis(), "package published");
            } else {
                warn!(
                    package = %planned.name,
                    error = result.error.as_deref().unwrap_or("unknown"),
                    "package publish failed"
                );
                failed_packages.insert(planned.name.clone());
            }

            results.push(result);

            // Delay between packages
            if !self.options.publish_delay.is_zero() && !self.options.dry_run {
                std::thread::sleep(self.options.publish_delay);
            }
        }

        let total_duration = start.elapsed();
        let success = failed_packages.is_empty();

        info!(
            success,
            published = results.iter().filter(|r| r.success).count(),
            failed = failed_packages.len(),
            skipped = skipped_packages.len(),
            duration_ms = total_duration.as_millis(),
            "publish complete"
        );

        Ok(PublishResult {
            packages: results,
            total_duration,
            success,
            skipped: skipped_packages,
        })
    }

    /// Publish a single package with retries
    fn publish_package(&self, planned: &PlannedPublish) -> PackagePublishResult {
        let start = Instant::now();

        if self.options.dry_run {
            return PackagePublishResult {
                package: planned.name.clone(),
                success: true,
                error: None,
                duration: start.elapsed(),
                registry_url: None,
            };
        }

        let publisher = match &self.publisher {
            Some(p) => p,
            None => {
                return PackagePublishResult {
                    package: planned.name.clone(),
                    success: false,
                    error: Some("No publisher configured".to_string()),
                    duration: start.elapsed(),
                    registry_url: None,
                };
            }
        };

        let mut last_error: Option<String> = None;

        for attempt in 0..=self.options.retry_count {
            match publisher(&planned.path, &planned.version, &self.options) {
                Ok(registry_url) => {
                    return PackagePublishResult {
                        package: planned.name.clone(),
                        success: true,
                        error: None,
                        duration: start.elapsed(),
                        registry_url,
                    };
                }
                Err(e) => {
                    last_error = Some(e.to_string());

                    if attempt < self.options.retry_count {
                        std::thread::sleep(self.options.retry_delay);
                    }
                }
            }
        }

        PackagePublishResult {
            package: planned.name.clone(),
            success: false,
            error: last_error,
            duration: start.elapsed(),
            registry_url: None,
        }
    }

    /// Validate that all packages can be published
    pub fn validate_plan(&self, plan: &PublishPlan) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for empty plan
        if plan.packages.is_empty() {
            warnings.push("No packages to publish".to_string());
        }

        // Check for packages with unmet dependencies
        let planned_names: HashSet<_> = plan.packages.iter().map(|p| p.name.as_str()).collect();

        for planned in &plan.packages {
            for dep in &planned.dependencies {
                if !planned_names.contains(dep.as_str()) {
                    warnings.push(format!(
                        "Package '{}' depends on '{}' which is not in the publish plan",
                        planned.name, dep
                    ));
                }
            }
        }

        Ok(warnings)
    }
}

/// Builder for creating a publish coordinator
pub struct PublishCoordinatorBuilder {
    options: PublishOptions,
    publisher: Option<PublishFn>,
}

impl PublishCoordinatorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            options: PublishOptions::default(),
            publisher: None,
        }
    }

    /// Set failure strategy
    pub fn failure_strategy(mut self, strategy: FailureStrategy) -> Self {
        self.options.failure_strategy = strategy;
        self
    }

    /// Set dry run mode
    pub fn dry_run(mut self, dry_run: bool) -> Self {
        self.options.dry_run = dry_run;
        self
    }

    /// Add packages to exclude
    pub fn exclude(mut self, packages: impl IntoIterator<Item = String>) -> Self {
        self.options.exclude.extend(packages);
        self
    }

    /// Set packages to publish (empty = all)
    pub fn only(mut self, packages: impl IntoIterator<Item = String>) -> Self {
        self.options.only.extend(packages);
        self
    }

    /// Set registry
    pub fn registry(mut self, registry: impl Into<String>) -> Self {
        self.options.registry = Some(registry.into());
        self
    }

    /// Set retry count
    pub fn retry(mut self, count: usize, delay: Duration) -> Self {
        self.options.retry_count = count;
        self.options.retry_delay = delay;
        self
    }

    /// Set delay between packages
    pub fn publish_delay(mut self, delay: Duration) -> Self {
        self.options.publish_delay = delay;
        self
    }

    /// Set publisher function
    pub fn publisher(mut self, publisher: PublishFn) -> Self {
        self.publisher = Some(publisher);
        self
    }

    /// Build the coordinator
    pub fn build(self) -> PublishCoordinator {
        let mut coordinator = PublishCoordinator::new(self.options);
        coordinator.publisher = self.publisher;
        coordinator
    }
}

impl Default for PublishCoordinatorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CanaveralError;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

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
                version: "1.0.0".to_string(),
                path: "packages/utils".into(),
                manifest_path: "packages/utils/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string()],
            },
            DiscoveredPackage {
                name: "cli".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/cli".into(),
                manifest_path: "packages/cli/package.json".into(),
                package_type: "npm".to_string(),
                private: false,
                workspace_dependencies: vec!["core".to_string(), "utils".to_string()],
            },
            DiscoveredPackage {
                name: "internal".to_string(),
                version: "1.0.0".to_string(),
                path: "packages/internal".into(),
                manifest_path: "packages/internal/package.json".into(),
                package_type: "npm".to_string(),
                private: true,
                workspace_dependencies: vec![],
            },
        ]
    }

    fn create_bumps() -> Vec<VersionBump> {
        use crate::types::ReleaseType;

        vec![
            VersionBump {
                package: "core".to_string(),
                current_version: "1.0.0".to_string(),
                new_version: "1.1.0".to_string(),
                release_type: ReleaseType::Minor,
                reason: "direct changes".to_string(),
            },
            VersionBump {
                package: "utils".to_string(),
                current_version: "1.0.0".to_string(),
                new_version: "1.1.0".to_string(),
                release_type: ReleaseType::Minor,
                reason: "dependency updated".to_string(),
            },
            VersionBump {
                package: "cli".to_string(),
                current_version: "1.0.0".to_string(),
                new_version: "1.1.0".to_string(),
                release_type: ReleaseType::Minor,
                reason: "dependency updated".to_string(),
            },
        ]
    }

    #[test]
    fn test_create_publish_plan() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let coordinator = PublishCoordinator::new(PublishOptions::default());
        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();

        // Should have 3 packages (internal is private)
        assert_eq!(plan.packages.len(), 3);
        assert_eq!(plan.skipped.len(), 1);
        assert_eq!(plan.skipped[0].name, "internal");
        assert_eq!(plan.skipped[0].reason, SkipReason::Private);

        // Check order: core should be first
        assert_eq!(plan.packages[0].name, "core");
        assert!(plan.packages[0].dependencies.is_empty());

        // utils should come after core
        let utils_idx = plan.packages.iter().position(|p| p.name == "utils").unwrap();
        let core_idx = plan.packages.iter().position(|p| p.name == "core").unwrap();
        assert!(utils_idx > core_idx);

        // cli should come after utils
        let cli_idx = plan.packages.iter().position(|p| p.name == "cli").unwrap();
        assert!(cli_idx > utils_idx);
    }

    #[test]
    fn test_dry_run_publish() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let mut options = PublishOptions::default();
        options.dry_run = true;

        let coordinator = PublishCoordinator::new(options);
        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();
        let result = coordinator.execute(&plan, &NoOpCallback).unwrap();

        assert!(result.success);
        assert_eq!(result.packages.len(), 3);
        assert!(result.packages.iter().all(|p| p.success));
    }

    #[test]
    fn test_exclude_packages() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let mut options = PublishOptions::default();
        options.exclude.insert("utils".to_string());

        let coordinator = PublishCoordinator::new(options);
        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();

        assert_eq!(plan.packages.len(), 2);
        assert!(!plan.packages.iter().any(|p| p.name == "utils"));

        let excluded = plan.skipped.iter().find(|s| s.name == "utils").unwrap();
        assert_eq!(excluded.reason, SkipReason::Excluded);
    }

    #[test]
    fn test_only_packages() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let mut options = PublishOptions::default();
        options.only.insert("core".to_string());

        let coordinator = PublishCoordinator::new(options);
        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();

        assert_eq!(plan.packages.len(), 1);
        assert_eq!(plan.packages[0].name, "core");
    }

    #[test]
    fn test_failure_strategy_stop() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let call_count = Arc::new(AtomicUsize::new(0));
        let call_count_clone = call_count.clone();

        let mut options = PublishOptions::default();
        options.failure_strategy = FailureStrategy::StopOnFailure;

        let coordinator = PublishCoordinatorBuilder::new()
            .failure_strategy(FailureStrategy::StopOnFailure)
            .publisher(Box::new(move |_path, _version, _opts| {
                let count = call_count_clone.fetch_add(1, Ordering::SeqCst);
                if count == 0 {
                    // First package (core) fails
                    Err(CanaveralError::other("publish failed"))
                } else {
                    Ok(None)
                }
            }))
            .build();

        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();
        let result = coordinator.execute(&plan, &NoOpCallback).unwrap();

        assert!(!result.success);
        assert_eq!(result.failed().len(), 1);
        // Other packages should be skipped
        assert!(!result.skipped.is_empty());
    }

    #[test]
    fn test_builder() {
        let coordinator = PublishCoordinatorBuilder::new()
            .dry_run(true)
            .failure_strategy(FailureStrategy::SkipDependents)
            .exclude(vec!["test".to_string()])
            .registry("https://custom.registry")
            .retry(3, Duration::from_secs(1))
            .publish_delay(Duration::from_millis(100))
            .build();

        assert!(coordinator.options.dry_run);
        assert_eq!(coordinator.options.failure_strategy, FailureStrategy::SkipDependents);
        assert!(coordinator.options.exclude.contains("test"));
        assert_eq!(coordinator.options.registry, Some("https://custom.registry".to_string()));
        assert_eq!(coordinator.options.retry_count, 3);
    }

    #[test]
    fn test_validate_plan() {
        let packages = create_packages();
        let bumps = create_bumps();
        let graph = DependencyGraph::build(&packages).unwrap();

        let coordinator = PublishCoordinator::new(PublishOptions::default());
        let plan = coordinator.create_plan(&packages, &bumps, &graph).unwrap();
        let warnings = coordinator.validate_plan(&plan).unwrap();

        assert!(warnings.is_empty());
    }

    #[test]
    fn test_publish_result_helpers() {
        let result = PublishResult {
            packages: vec![
                PackagePublishResult {
                    package: "a".to_string(),
                    success: true,
                    error: None,
                    duration: Duration::from_secs(1),
                    registry_url: None,
                },
                PackagePublishResult {
                    package: "b".to_string(),
                    success: false,
                    error: Some("failed".to_string()),
                    duration: Duration::from_secs(1),
                    registry_url: None,
                },
            ],
            total_duration: Duration::from_secs(2),
            success: false,
            skipped: vec![],
        };

        assert_eq!(result.successful().len(), 1);
        assert_eq!(result.failed().len(), 1);
        assert_eq!(result.successful()[0].package, "a");
        assert_eq!(result.failed()[0].package, "b");
    }

    #[test]
    fn test_empty_registry() {
        let registry = PublishCallbackRegistry::new();
        assert!(registry.is_empty());
        assert!(registry.all().is_empty());

        // Should not panic with no callbacks
        registry.on_publish_start("pkg", "1.0.0");
        registry.on_publish_complete(
            "pkg",
            &PackagePublishResult {
                package: "pkg".to_string(),
                success: true,
                error: None,
                duration: Duration::from_secs(1),
                registry_url: None,
            },
        );
        registry.on_skip("pkg", &SkipReason::Private);
    }

    #[test]
    fn test_register_and_broadcast() {
        use std::sync::Mutex;

        // Shared state across callbacks
        let starts: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
        let completes: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let skips: Arc<Mutex<Vec<(String, SkipReason)>>> = Arc::new(Mutex::new(Vec::new()));

        struct SharedCallback {
            starts: Arc<Mutex<Vec<(String, String)>>>,
            completes: Arc<Mutex<Vec<String>>>,
            skips: Arc<Mutex<Vec<(String, SkipReason)>>>,
        }

        impl PublishCallback for SharedCallback {
            fn on_publish_start(&self, package: &str, version: &str) {
                self.starts
                    .lock()
                    .unwrap()
                    .push((package.to_string(), version.to_string()));
            }
            fn on_publish_complete(&self, package: &str, _result: &PackagePublishResult) {
                self.completes.lock().unwrap().push(package.to_string());
            }
            fn on_skip(&self, package: &str, reason: &SkipReason) {
                self.skips
                    .lock()
                    .unwrap()
                    .push((package.to_string(), reason.clone()));
            }
        }

        let mut registry = PublishCallbackRegistry::new();

        // Two callbacks sharing the same state — each event increments counts twice
        registry.register(SharedCallback {
            starts: starts.clone(),
            completes: completes.clone(),
            skips: skips.clone(),
        });
        registry.register(SharedCallback {
            starts: starts.clone(),
            completes: completes.clone(),
            skips: skips.clone(),
        });

        assert!(!registry.is_empty());
        assert_eq!(registry.all().len(), 2);

        // Broadcast events
        registry.on_publish_start("core", "2.0.0");
        registry.on_publish_complete(
            "core",
            &PackagePublishResult {
                package: "core".to_string(),
                success: true,
                error: None,
                duration: Duration::from_secs(1),
                registry_url: None,
            },
        );
        registry.on_skip("internal", &SkipReason::Private);

        // Both callbacks received all events (2 callbacks x 1 event each = 2 entries)
        let starts = starts.lock().unwrap();
        assert_eq!(starts.len(), 2);
        assert!(starts.iter().all(|(p, v)| p == "core" && v == "2.0.0"));

        let completes = completes.lock().unwrap();
        assert_eq!(completes.len(), 2);
        assert!(completes.iter().all(|p| p == "core"));

        let skips = skips.lock().unwrap();
        assert_eq!(skips.len(), 2);
        assert!(skips
            .iter()
            .all(|(p, r)| p == "internal" && *r == SkipReason::Private));
    }
}
