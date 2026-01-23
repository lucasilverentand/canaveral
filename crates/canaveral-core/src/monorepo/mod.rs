//! Monorepo support for multi-package repositories
//!
//! This module provides functionality for managing releases in monorepos:
//! - Workspace detection (Cargo, npm, pnpm, yarn, lerna, nx, turbo)
//! - Package discovery with glob patterns
//! - Change detection based on git diffs
//! - Dependency graph analysis with topological sorting
//! - Versioning modes (independent, fixed, grouped)
//! - Coordinated publishing with failure handling

pub mod changes;
pub mod discovery;
pub mod graph;
pub mod publishing;
pub mod versioning;
pub mod workspace;

pub use changes::{ChangeDetector, ChangeFilter, ChangeReason, ChangedPackage};
pub use discovery::{DiscoveredPackage, PackageDiscovery};
pub use graph::{DependencyGraph, PackageNode};
pub use publishing::{
    FailureStrategy, PackagePublishResult, PlannedPublish, PublishCallback, PublishCoordinator,
    PublishCoordinatorBuilder, PublishOptions, PublishPlan, PublishResult, SkipReason,
    SkippedPackage,
};
pub use versioning::{VersionBump, VersioningMode, VersioningStrategy};
pub use workspace::{Workspace, WorkspaceType};
