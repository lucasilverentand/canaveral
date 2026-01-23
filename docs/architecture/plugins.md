# Plugin System

The plugin system provides extensibility through well-defined trait interfaces, allowing the community to extend Canaveral's functionality without modifying core code.

## Plugin Architecture

Canaveral supports two plugin mechanisms:

1. **Native Plugins** - Rust shared libraries (.so, .dylib, .dll) for maximum performance
2. **WASM Plugins** - WebAssembly modules for sandboxed, portable plugins

## Plugin Types

### 1. Version Strategies

Custom version calculation logic beyond built-in SemVer/CalVer.

```rust
use canaveral_core::{Commit, VersionStrategy, VersionComponents, StrategyOptions};
use async_trait::async_trait;

#[async_trait]
pub trait VersionStrategy: Send + Sync {
    /// Strategy name for configuration
    fn name(&self) -> &str;

    /// Calculate next version based on commits
    async fn calculate(
        &self,
        current_version: &str,
        commits: &[Commit],
        options: &StrategyOptions,
    ) -> anyhow::Result<String>;

    /// Parse version string into components
    fn parse(&self, version: &str) -> anyhow::Result<VersionComponents>;

    /// Compare two versions (-1, 0, 1)
    fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering;

    /// Validate version string
    fn validate(&self, version: &str) -> bool;
}
```

### 2. Package Adapters

Support for additional package managers and registries.

```rust
use canaveral_core::{PublishOptions, PublishResult, Dependency, ValidationResult};
use async_trait::async_trait;
use std::path::Path;

#[async_trait]
pub trait PackageAdapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;

    /// Ecosystem identifier (npm, cargo, python, etc.)
    fn ecosystem(&self) -> &str;

    /// Manifest filename (package.json, Cargo.toml, etc.)
    fn manifest_file(&self) -> &str;

    /// Detect if this adapter applies to the project
    async fn detect(&self, project_path: &Path) -> anyhow::Result<bool>;

    /// Read current version from manifest
    async fn read_version(&self, manifest_path: &Path) -> anyhow::Result<String>;

    /// Write new version to manifest
    async fn write_version(&self, manifest_path: &Path, version: &str) -> anyhow::Result<()>;

    /// Publish package to registry
    async fn publish(&self, options: &PublishOptions) -> anyhow::Result<PublishResult>;

    /// Unpublish a version (optional, not all registries support)
    async fn unpublish(&self, _version: &str) -> anyhow::Result<()> {
        Err(anyhow::anyhow!("Unpublish not supported"))
    }

    /// Validate manifest file
    async fn validate_manifest(&self, manifest_path: &Path) -> anyhow::Result<ValidationResult>;

    /// Validate registry credentials
    async fn validate_credentials(&self) -> anyhow::Result<bool>;

    /// Read dependencies (for monorepo support)
    async fn read_dependencies(&self, manifest_path: &Path) -> anyhow::Result<Vec<Dependency>> {
        Ok(vec![])
    }

    /// Update dependency versions
    async fn write_dependencies(
        &self,
        manifest_path: &Path,
        deps: &[Dependency],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
```

### 3. Changelog Generators

Alternative changelog formats and output targets.

```rust
use canaveral_core::{Commit, ChangelogOptions};
use async_trait::async_trait;

#[async_trait]
pub trait ChangelogGenerator: Send + Sync {
    /// Generator name
    fn name(&self) -> &str;

    /// Generate changelog content from commits
    async fn generate(
        &self,
        commits: &[Commit],
        version: &str,
        options: &ChangelogOptions,
    ) -> anyhow::Result<String>;

    /// Insert new entry into existing changelog
    async fn prepend(
        &self,
        existing_content: &str,
        new_entry: &str,
    ) -> anyhow::Result<String>;
}
```

### 4. Commit Parsers

Custom commit message parsing beyond Conventional Commits.

```rust
use canaveral_core::{ParsedCommit, ReleaseType};

pub trait CommitParser: Send + Sync {
    /// Parser name
    fn name(&self) -> &str;

    /// Parse commit message into structured data
    fn parse(&self, message: &str) -> Option<ParsedCommit>;

    /// Determine release type from commits
    fn get_release_type(&self, commits: &[ParsedCommit]) -> ReleaseType;
}
```

### 5. Lifecycle Hooks

Custom actions at various stages of the release process.

```rust
use canaveral_core::HookContext;
use async_trait::async_trait;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStage {
    PreVersion,
    PostVersion,
    PreChangelog,
    PostChangelog,
    PreCommit,
    PostCommit,
    PrePublish,
    PostPublish,
}

#[async_trait]
pub trait LifecycleHook: Send + Sync {
    /// Hook name
    fn name(&self) -> &str;

    /// Stage this hook runs at
    fn stage(&self) -> HookStage;

    /// Execute the hook
    async fn execute(&self, context: &mut HookContext) -> anyhow::Result<()>;
}
```

## Native Plugin Development

### Creating a Plugin Crate

```toml
# Cargo.toml
[package]
name = "canaveral-plugin-example"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
canaveral-core = "0.1"
async-trait = "0.1"
anyhow = "1"
```

```rust
// src/lib.rs
use canaveral_core::{VersionStrategy, VersionComponents, Commit, StrategyOptions};
use async_trait::async_trait;

pub struct GitFlowStrategy;

#[async_trait]
impl VersionStrategy for GitFlowStrategy {
    fn name(&self) -> &str {
        "git-flow"
    }

    async fn calculate(
        &self,
        current: &str,
        commits: &[Commit],
        options: &StrategyOptions,
    ) -> anyhow::Result<String> {
        let branch = &options.branch;

        if branch == "main" || branch == "master" {
            // Production release
            self.bump_major_minor(current, commits)
        } else if branch.starts_with("release/") {
            // Release candidate
            self.bump_rc(current)
        } else if branch == "develop" {
            // Development snapshot
            Ok(format!("{}-SNAPSHOT", current))
        } else {
            Ok(current.to_string())
        }
    }

    fn parse(&self, version: &str) -> anyhow::Result<VersionComponents> {
        // Parse version string
        todo!()
    }

    fn compare(&self, a: &str, b: &str) -> std::cmp::Ordering {
        // Compare versions
        todo!()
    }

    fn validate(&self, version: &str) -> bool {
        // Validate version format
        todo!()
    }
}

// Export plugin entry point
canaveral_core::export_plugin!(GitFlowStrategy);
```

### Plugin Manifest

Each plugin includes a manifest file:

```toml
# plugin.toml
[plugin]
name = "canaveral-plugin-example"
version = "0.1.0"
type = "strategy"  # strategy, adapter, changelog, hook
description = "Git Flow versioning strategy"

[plugin.config]
# JSON Schema for plugin configuration
schema = "schema.json"
```

## WASM Plugin Development

For sandboxed plugins that run in a WebAssembly runtime:

```rust
// Build with: cargo build --target wasm32-wasi

use canaveral_wasm::{VersionStrategy, Commit};

#[canaveral_wasm::plugin]
struct MyStrategy;

impl VersionStrategy for MyStrategy {
    fn name(&self) -> &str {
        "my-strategy"
    }

    fn calculate(&self, current: &str, commits: &[Commit]) -> String {
        // WASM plugins have limited async support
        format!("{}.1", current)
    }
}
```

## Plugin Discovery

Plugins are discovered through multiple mechanisms:

### 1. Built-in Plugins
Compiled into the binary, always available.

### 2. Local Plugins
Defined in project's `canaveral.yaml`:

```yaml
plugins:
  - path: ./plugins/my-strategy.so
  - path: ./plugins/custom-adapter.wasm
```

### 3. Installed Plugins
Plugins installed to `~/.canaveral/plugins/`:

```bash
canaveral plugin install canaveral-plugin-slack
```

### 4. Convention-based
Shared libraries in plugin directories matching `canaveral-plugin-*`.

## Plugin Configuration

Plugins receive configuration from `canaveral.yaml`:

```yaml
plugins:
  - name: canaveral-plugin-slack
    config:
      webhook: ${SLACK_WEBHOOK_URL}
      channel: '#releases'
```

Configuration is passed to the plugin during initialization:

```rust
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Initialize plugin with configuration
    async fn init(&mut self, config: serde_json::Value) -> anyhow::Result<()>;

    /// Cleanup resources
    async fn cleanup(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
```

## Plugin Lifecycle

```
┌──────────┐     ┌──────────┐     ┌──────────┐
│  Discover│────▶│  Load    │────▶│  Validate│
│  Plugins │     │  Library │     │  Schema  │
└──────────┘     └──────────┘     └──────────┘
                                       │
     ┌─────────────────────────────────┘
     ▼
┌──────────┐     ┌──────────┐     ┌──────────┐
│Initialize│────▶│  Register│────▶│  Execute │
│  Config  │     │  Hooks   │     │ at Stage │
└──────────┘     └──────────┘     └──────────┘
                                       │
                                       ▼
                                 ┌──────────┐
                                 │  Cleanup │
                                 │ Resources│
                                 └──────────┘
```

## Security Considerations

### Native Plugins
- Run with full system permissions
- Only install plugins from trusted sources
- Review plugin code before use in CI/CD
- Use lock files to pin plugin versions

### WASM Plugins
- Run in sandboxed environment
- Limited system access (no filesystem, network by default)
- Capabilities granted explicitly in configuration
- Safer for untrusted plugins

```yaml
plugins:
  - name: untrusted-plugin
    path: ./plugin.wasm
    sandbox: true
    capabilities:
      - network  # Allow HTTP requests
      # - filesystem  # Deny filesystem access
```

## Plugin CLI Commands

```bash
# List installed plugins
canaveral plugin list

# Install a plugin
canaveral plugin install canaveral-plugin-slack

# Remove a plugin
canaveral plugin remove canaveral-plugin-slack

# Update plugins
canaveral plugin update

# Show plugin info
canaveral plugin info canaveral-plugin-slack
```
