//! Canaveral Core - Core library for release management
//!
//! This crate provides the foundational types, error handling, configuration,
//! and workflow orchestration for the Canaveral release management tool.

pub mod config;
pub mod error;
pub mod hooks;
pub mod migration;
pub mod monorepo;
pub mod plugins;
pub mod templates;
pub mod types;
pub mod workflow;

pub use error::{CanaveralError, HookError, Result};
pub use hooks::{Hook, HookContext, HookRunner, HookStage, HooksConfig};
pub use migration::{
    auto_migrate, detect_tool, MigrationResult, MigrationSource, Migrator, MigratorRegistry,
    ReleasePleaseMigrator, SemanticReleaseMigrator,
};
pub use monorepo::detector::{WorkspaceDetector, WorkspaceDetectorRegistry};
pub use monorepo::publishing::PublishCallbackRegistry;
pub use plugins::{ExternalPlugin, PluginConfig, PluginInfo, PluginRegistry, PluginType};
pub use templates::{
    CITemplate, CITemplateRegistry, GitHubActionsTemplate, GitLabCITemplate, TemplateOptions,
};
pub use types::{ReleaseResult, ReleaseType};
