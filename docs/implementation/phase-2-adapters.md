# Phase 2: Core Adapters ✅

**Status:** Complete

**Goal**: Support publishing to npm, crates.io, and PyPI with proper credential management.

## Tasks

### 2.1 Adapter Trait

- [x] Define abstract PackageAdapter trait
- [x] Create adapter registry
- [x] Implement adapter discovery
- [x] Add adapter configuration types

**Adapters crate structure:**
```
crates/canaveral-adapters/src/
├── lib.rs
├── traits.rs          # PackageAdapter trait
├── registry.rs        # Adapter registration
├── detector.rs        # Auto-detection logic
├── npm/
│   ├── mod.rs
│   ├── manifest.rs    # package.json operations
│   ├── publish.rs     # npm publish
│   └── auth.rs        # Token handling
├── cargo/
│   ├── mod.rs
│   ├── manifest.rs    # Cargo.toml operations
│   ├── publish.rs     # cargo publish
│   └── auth.rs
└── python/
    ├── mod.rs
    ├── manifest.rs    # pyproject.toml operations
    ├── build.rs       # Build wheel/sdist
    ├── publish.rs     # twine upload
    └── auth.rs
```

**Trait definition:**
```rust
// crates/canaveral-adapters/src/traits.rs
use std::path::Path;
use canaveral_core::error::Result;
use canaveral_core::types::PackageInfo;

/// Trait for package adapters (synchronous)
pub trait PackageAdapter: Send + Sync {
    /// Get the adapter name (e.g., "npm", "cargo")
    fn name(&self) -> &'static str;

    /// Get the default registry URL for this adapter
    fn default_registry(&self) -> &'static str;

    /// Check if this adapter applies to the given path
    fn detect(&self, path: &Path) -> bool;

    /// Get package information from manifest
    fn get_info(&self, path: &Path) -> Result<PackageInfo>;

    /// Get current version
    fn get_version(&self, path: &Path) -> Result<String>;

    /// Set version in manifest
    fn set_version(&self, path: &Path, version: &str) -> Result<()>;

    /// Publish package (simple version)
    fn publish(&self, path: &Path, dry_run: bool) -> Result<()>;

    /// Publish package with detailed options
    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()>;

    /// Validate that the package can be published
    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult>;

    /// Check if authentication is configured
    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool>;

    /// Get the manifest filename(s) this adapter handles
    fn manifest_names(&self) -> &[&str];

    /// Build the package (if applicable)
    fn build(&self, path: &Path) -> Result<()>;

    /// Run tests (if applicable)
    fn test(&self, path: &Path) -> Result<()>;

    /// Clean build artifacts (if applicable)
    fn clean(&self, path: &Path) -> Result<()>;

    /// Pack for publishing without actually publishing
    fn pack(&self, path: &Path) -> Result<Option<PathBuf>>;
}
```

Note: The `PackageAdapter` trait is synchronous (not async). The `StoreAdapter` trait in `canaveral-stores` is async for store uploaders (App Store Connect, Google Play, etc.).

### 2.2 npm Adapter

- [x] Parse and modify package.json
- [x] Handle scoped packages (@org/name)
- [x] Execute npm publish via command
- [x] Support custom registries
- [x] Handle access levels (public/restricted)
- [x] Support npm tags (latest, next, etc.)

**npm adapter (simplified):**
```rust
// crates/canaveral-adapters/src/npm/mod.rs
pub struct NpmAdapter;

impl PackageAdapter for NpmAdapter {
    fn name(&self) -> &'static str { "npm" }
    fn default_registry(&self) -> &'static str { "https://registry.npmjs.org" }
    fn manifest_names(&self) -> &[&str] { &["package.json"] }

    fn detect(&self, path: &Path) -> bool {
        path.join("package.json").exists()
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path.join("package.json"))?;
        let pkg: serde_json::Value = serde_json::from_str(&content)?;
        Ok(pkg["version"].as_str().unwrap_or("0.0.0").to_string())
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        // Read, update version field, write back preserving formatting
        // ...
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        // Execute npm publish with appropriate flags
        // ...
    }
    // ... other methods
}
```

### 2.3 Cargo Adapter

- [x] Parse and modify Cargo.toml (TOML)
- [x] Handle workspace members
- [x] Execute cargo publish
- [x] Support crates.io authentication
- [x] Handle publish restrictions

**Cargo adapter (simplified):**
```rust
// crates/canaveral-adapters/src/cargo/mod.rs
pub struct CargoAdapter;

impl PackageAdapter for CargoAdapter {
    fn name(&self) -> &'static str { "cargo" }
    fn default_registry(&self) -> &'static str { "https://crates.io" }
    fn manifest_names(&self) -> &[&str] { &["Cargo.toml"] }

    fn detect(&self, path: &Path) -> bool {
        let cargo_toml = path.join("Cargo.toml");
        if !cargo_toml.exists() { return false; }
        // Check for [package] section (not just workspace)
        let content = std::fs::read_to_string(&cargo_toml).unwrap_or_default();
        content.contains("[package]")
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let content = std::fs::read_to_string(path.join("Cargo.toml"))?;
        // Parse TOML and extract [package].version
        // ...
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        // Update [package].version using toml_edit to preserve formatting
        // ...
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        // Execute cargo publish with appropriate flags
        // ...
    }
    // ... other methods
}
```

### 2.4 Python Adapter

- [x] Parse pyproject.toml (PEP 621)
- [x] Support setup.py/setup.cfg fallback
- [x] Build package (wheel, sdist)
- [x] Publish via twine
- [x] Support PyPI and custom indexes

**Python adapter (simplified):**
```rust
// crates/canaveral-adapters/src/python/mod.rs
pub struct PythonAdapter;

impl PackageAdapter for PythonAdapter {
    fn name(&self) -> &'static str { "python" }
    fn default_registry(&self) -> &'static str { "https://upload.pypi.org/legacy/" }
    fn manifest_names(&self) -> &[&str] { &["pyproject.toml", "setup.py"] }

    fn detect(&self, path: &Path) -> bool {
        path.join("pyproject.toml").exists() || path.join("setup.py").exists()
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        // Try [project].version (PEP 621), then [tool.poetry].version
        // ...
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        // Update version in pyproject.toml using toml_edit
        // ...
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        // Build with python -m build, upload with twine
        // ...
    }
    // ... other methods
}
```

### 2.5 Credential Management

- [x] Environment variable support
- [x] System keychain integration (keyring crate)
- [x] Config file credentials (.npmrc, etc.)
- [x] Secure token handling (no logging)
- [x] Credential validation before publish

**Credentials module:**
```rust
// crates/canaveral-adapters/src/credentials.rs

pub struct CredentialProvider {
    // Manages credentials from multiple sources
}

impl CredentialProvider {
    /// Check if credentials exist for a given adapter
    pub fn has_credentials(&self, adapter_name: &str) -> bool {
        // Check environment variables, config files, etc.
        // ...
    }
}
```

Adapters check authentication via the `check_auth` method on the `PackageAdapter` trait.

### 2.6 Dry-Run Mode

- [x] Preview version changes
- [x] Show changelog that would be generated
- [x] Display files that would be modified
- [x] Show git operations that would occur
- [x] Simulate publish without execution

**Dry-run output:**
```rust
// crates/canaveral/src/cli/output.rs
use console::{style, Emoji};

static LOOKING: Emoji = Emoji("🔍", "");
static PACKAGE: Emoji = Emoji("📦", "");
static MEMO: Emoji = Emoji("📝", "");
static FILE: Emoji = Emoji("📄", "");
static TAG: Emoji = Emoji("🔖", "");
static UPLOAD: Emoji = Emoji("📤", "");

pub fn print_dry_run(result: &DryRunResult) {
    println!("{} {} Dry Run Mode - No changes will be made\n",
        LOOKING, style("Dry Run Mode").bold().yellow());

    println!("{} Package: {}", PACKAGE, style(&result.package_name).cyan());
    println!("   Current version: {}", result.current_version);
    println!("   New version: {}\n", style(&result.new_version).green());

    println!("{} Changelog Preview:", MEMO);
    for line in result.changelog_preview.lines().take(10) {
        println!("   {}", line);
    }
    println!();

    println!("{} Files to modify:", FILE);
    for file in &result.modified_files {
        println!("   - {}", file.display());
    }
    println!();

    println!("{} Git operations:", TAG);
    println!("   - Commit: \"{}\"", result.commit_message);
    println!("   - Tag: {}", result.tag_name);
    if result.will_push {
        println!("   - Push: origin/{}", result.branch);
    }
    println!();

    println!("{} Publish:", UPLOAD);
    println!("   - Registry: {}", result.registry);
    println!("   - Package: {}@{}", result.package_name, result.new_version);
}
```

### 2.7 Validation & Error Handling

- [x] Pre-flight validation checks
- [x] Clear error messages
- [x] Suggestions for fixing issues
- [x] Structured error types

**Validation:**
```rust
// crates/canaveral-core/src/validation.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Working directory has uncommitted changes")]
    DirtyWorkingDirectory,

    #[error("Not on release branch. Expected '{expected}', got '{actual}'")]
    WrongBranch { expected: String, actual: String },

    #[error("No credentials found for {registry}")]
    MissingCredentials { registry: String },

    #[error("Version {version} already exists on {registry}")]
    VersionExists { version: String, registry: String },

    #[error("Invalid manifest: {message}")]
    InvalidManifest { message: String },

    #[error("No changes to release")]
    NoChanges,
}

pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<String>,
}

pub fn validate_release(ctx: &ReleaseContext) -> ValidationResult {
    let mut result = ValidationResult {
        valid: true,
        errors: vec![],
        warnings: vec![],
    };

    // Check git is clean
    if !ctx.git.is_clean().unwrap_or(false) {
        result.errors.push(ValidationError::DirtyWorkingDirectory);
        result.valid = false;
    }

    // Check branch
    if let Ok(branch) = ctx.git.current_branch() {
        if branch != ctx.config.git.branch {
            result.errors.push(ValidationError::WrongBranch {
                expected: ctx.config.git.branch.clone(),
                actual: branch,
            });
            result.valid = false;
        }
    }

    // Check credentials
    for adapter in &ctx.adapters {
        if !adapter.check_auth(&mut ctx.credentials).unwrap_or(false) {
            result.errors.push(ValidationError::MissingCredentials {
                registry: adapter.ecosystem().to_string(),
            });
            result.valid = false;
        }
    }

    result
}
```

## Testing Strategy

### Unit Tests
- Manifest parsing (package.json, Cargo.toml, pyproject.toml)
- Credential resolution from various sources
- Version string formatting

### Integration Tests
- npm publish to local Verdaccio
- Cargo publish dry-run
- Python build and twine check

### Test setup
```rust
// tests/integration/adapters_test.rs
use tempfile::TempDir;

#[tokio::test]
async fn test_npm_adapter_read_version() {
    let dir = TempDir::new().unwrap();
    let package_json = dir.path().join("package.json");

    std::fs::write(&package_json, r#"{"name": "test", "version": "1.2.3"}"#).unwrap();

    let adapter = NpmAdapter::new();
    let version = adapter.read_version(&package_json).await.unwrap();

    assert_eq!(version, "1.2.3");
}
```

## Definition of Done

Phase 2 is complete when:

1. [x] npm adapter reads/writes package.json correctly
2. [x] npm adapter publishes to npmjs.com
3. [x] Cargo adapter reads/writes Cargo.toml correctly
4. [x] Cargo adapter publishes to crates.io
5. [x] Python adapter supports pyproject.toml
6. [x] Python adapter builds and publishes to PyPI
7. [x] Credentials are loaded from env vars and keychain
8. [x] Dry-run mode shows accurate preview
9. [x] Validation catches common issues
10. [x] Error messages are helpful and actionable
11. [x] All adapters have >80% test coverage
