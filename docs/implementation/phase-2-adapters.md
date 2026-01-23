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
use async_trait::async_trait;
use std::path::Path;
use anyhow::Result;

#[async_trait]
pub trait PackageAdapter: Send + Sync {
    /// Adapter name
    fn name(&self) -> &str;

    /// Ecosystem identifier (npm, cargo, python, etc.)
    fn ecosystem(&self) -> &str;

    /// Manifest filename (package.json, Cargo.toml, etc.)
    fn manifest_file(&self) -> &str;

    /// Detect if this adapter applies to the project
    async fn detect(&self, project_path: &Path) -> Result<bool>;

    /// Read current version from manifest
    async fn read_version(&self, manifest_path: &Path) -> Result<String>;

    /// Write new version to manifest
    async fn write_version(&self, manifest_path: &Path, version: &str) -> Result<()>;

    /// Publish package to registry
    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult>;

    /// Unpublish a version (optional)
    async fn unpublish(&self, _version: &str) -> Result<()> {
        Err(anyhow::anyhow!("Unpublish not supported"))
    }

    /// Validate manifest file
    async fn validate_manifest(&self, manifest_path: &Path) -> Result<ValidationResult>;

    /// Validate registry credentials
    async fn validate_credentials(&self) -> Result<bool>;

    /// Read dependencies (for monorepo support)
    async fn read_dependencies(&self, manifest_path: &Path) -> Result<Vec<Dependency>> {
        Ok(vec![])
    }

    /// Update dependency versions
    async fn write_dependencies(
        &self,
        manifest_path: &Path,
        deps: &[Dependency],
    ) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PublishOptions {
    pub path: PathBuf,
    pub version: String,
    pub dry_run: bool,
    pub registry: Option<String>,
    pub tag: Option<String>,
    pub access: Option<Access>,
}

#[derive(Debug, Clone)]
pub struct PublishResult {
    pub success: bool,
    pub package_name: String,
    pub version: String,
    pub registry: String,
    pub url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum Access {
    Public,
    Restricted,
}
```

### 2.2 npm Adapter

- [x] Parse and modify package.json
- [x] Handle scoped packages (@org/name)
- [x] Execute npm publish via command
- [x] Support custom registries
- [x] Handle access levels (public/restricted)
- [x] Support npm tags (latest, next, etc.)

**npm adapter:**
```rust
// crates/canaveral-adapters/src/npm/mod.rs
use crate::traits::{PackageAdapter, PublishOptions, PublishResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::process::Command;

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageJson {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub private: bool,
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: HashMap<String, String>,
    #[serde(flatten)]
    pub other: serde_json::Value,
}

pub struct NpmAdapter {
    registry: String,
}

#[async_trait]
impl PackageAdapter for NpmAdapter {
    fn name(&self) -> &str { "npm" }
    fn ecosystem(&self) -> &str { "npm" }
    fn manifest_file(&self) -> &str { "package.json" }

    async fn detect(&self, project_path: &Path) -> Result<bool> {
        Ok(project_path.join("package.json").exists())
    }

    async fn read_version(&self, manifest_path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let pkg: PackageJson = serde_json::from_str(&content)?;
        Ok(pkg.version)
    }

    async fn write_version(&self, manifest_path: &Path, version: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let mut pkg: serde_json::Value = serde_json::from_str(&content)?;

        pkg["version"] = serde_json::Value::String(version.to_string());

        // Preserve formatting by using pretty print with 2 spaces
        let output = serde_json::to_string_pretty(&pkg)?;
        tokio::fs::write(manifest_path, output).await?;
        Ok(())
    }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        let mut cmd = Command::new("npm");
        cmd.arg("publish");
        cmd.current_dir(&options.path);

        if options.dry_run {
            cmd.arg("--dry-run");
        }

        if let Some(ref registry) = options.registry {
            cmd.arg("--registry").arg(registry);
        }

        if let Some(ref tag) = options.tag {
            cmd.arg("--tag").arg(tag);
        }

        if let Some(access) = options.access {
            cmd.arg("--access").arg(match access {
                Access::Public => "public",
                Access::Restricted => "restricted",
            });
        }

        let output = cmd.output().await?;

        Ok(PublishResult {
            success: output.status.success(),
            package_name: self.read_package_name(&options.path).await?,
            version: options.version.clone(),
            registry: options.registry.clone().unwrap_or_else(|| self.registry.clone()),
            url: None,
            error: if !output.status.success() {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            } else {
                None
            },
        })
    }

    async fn validate_credentials(&self) -> Result<bool> {
        // Check for NPM_TOKEN or npm whoami
        if std::env::var("NPM_TOKEN").is_ok() {
            return Ok(true);
        }

        let output = Command::new("npm")
            .args(["whoami", "--registry", &self.registry])
            .output()
            .await?;

        Ok(output.status.success())
    }
}
```

### 2.3 Cargo Adapter

- [x] Parse and modify Cargo.toml (TOML)
- [x] Handle workspace members
- [x] Execute cargo publish
- [x] Support crates.io authentication
- [x] Handle publish restrictions

**Cargo adapter:**
```rust
// crates/canaveral-adapters/src/cargo/mod.rs
use crate::traits::{PackageAdapter, PublishOptions, PublishResult};
use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;
use toml_edit::{Document, value};

pub struct CargoAdapter;

#[async_trait]
impl PackageAdapter for CargoAdapter {
    fn name(&self) -> &str { "cargo" }
    fn ecosystem(&self) -> &str { "cargo" }
    fn manifest_file(&self) -> &str { "Cargo.toml" }

    async fn detect(&self, project_path: &Path) -> Result<bool> {
        let cargo_toml = project_path.join("Cargo.toml");
        if !cargo_toml.exists() {
            return Ok(false);
        }

        // Check if it has a [package] section (not just workspace)
        let content = tokio::fs::read_to_string(&cargo_toml).await?;
        let doc: Document = content.parse()?;
        Ok(doc.get("package").is_some())
    }

    async fn read_version(&self, manifest_path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let doc: Document = content.parse()?;

        doc.get("package")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow::anyhow!("No version found in Cargo.toml"))
    }

    async fn write_version(&self, manifest_path: &Path, version: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let mut doc: Document = content.parse()?;

        doc["package"]["version"] = value(version);

        tokio::fs::write(manifest_path, doc.to_string()).await?;
        Ok(())
    }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        let mut cmd = Command::new("cargo");
        cmd.arg("publish");
        cmd.current_dir(&options.path);

        if options.dry_run {
            cmd.arg("--dry-run");
        }

        // Allow dirty for CI environments where git state may vary
        cmd.arg("--allow-dirty");

        let output = cmd.output().await?;

        let package_name = self.read_package_name(&options.path).await?;

        Ok(PublishResult {
            success: output.status.success(),
            package_name: package_name.clone(),
            version: options.version.clone(),
            registry: "crates.io".to_string(),
            url: Some(format!("https://crates.io/crates/{}", package_name)),
            error: if !output.status.success() {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            } else {
                None
            },
        })
    }

    async fn validate_credentials(&self) -> Result<bool> {
        // Check for CARGO_REGISTRY_TOKEN
        if std::env::var("CARGO_REGISTRY_TOKEN").is_ok() {
            return Ok(true);
        }

        // Check credentials file
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        let creds = home.join(".cargo/credentials.toml");

        if creds.exists() {
            let content = tokio::fs::read_to_string(&creds).await?;
            let doc: Document = content.parse()?;
            return Ok(doc.get("registry").and_then(|r| r.get("token")).is_some());
        }

        Ok(false)
    }
}
```

### 2.4 Python Adapter

- [x] Parse pyproject.toml (PEP 621)
- [x] Support setup.py/setup.cfg fallback
- [x] Build package (wheel, sdist)
- [x] Publish via twine
- [x] Support PyPI and custom indexes

**Python adapter:**
```rust
// crates/canaveral-adapters/src/python/mod.rs
use crate::traits::{PackageAdapter, PublishOptions, PublishResult};
use async_trait::async_trait;
use std::path::Path;
use tokio::process::Command;

pub struct PythonAdapter {
    registry: String,
}

#[async_trait]
impl PackageAdapter for PythonAdapter {
    fn name(&self) -> &str { "python" }
    fn ecosystem(&self) -> &str { "python" }
    fn manifest_file(&self) -> &str { "pyproject.toml" }

    async fn detect(&self, project_path: &Path) -> Result<bool> {
        // Check for pyproject.toml first
        if project_path.join("pyproject.toml").exists() {
            return Ok(true);
        }
        // Fallback to setup.py
        Ok(project_path.join("setup.py").exists())
    }

    async fn read_version(&self, manifest_path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let doc: Document = content.parse()?;

        // Try [project].version first (PEP 621)
        if let Some(version) = doc.get("project")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
        {
            return Ok(version.to_string());
        }

        // Try [tool.poetry].version
        if let Some(version) = doc.get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
        {
            return Ok(version.to_string());
        }

        Err(anyhow::anyhow!("No version found in pyproject.toml"))
    }

    async fn write_version(&self, manifest_path: &Path, version: &str) -> Result<()> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let mut doc: Document = content.parse()?;

        // Update [project].version if it exists
        if doc.get("project").and_then(|p| p.get("version")).is_some() {
            doc["project"]["version"] = value(version);
        }
        // Or [tool.poetry].version
        else if doc.get("tool")
            .and_then(|t| t.get("poetry"))
            .and_then(|p| p.get("version"))
            .is_some()
        {
            doc["tool"]["poetry"]["version"] = value(version);
        }

        tokio::fs::write(manifest_path, doc.to_string()).await?;
        Ok(())
    }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        // Build first
        let build_output = Command::new("python")
            .args(["-m", "build"])
            .current_dir(&options.path)
            .output()
            .await?;

        if !build_output.status.success() {
            return Ok(PublishResult {
                success: false,
                package_name: String::new(),
                version: options.version.clone(),
                registry: self.registry.clone(),
                url: None,
                error: Some(String::from_utf8_lossy(&build_output.stderr).to_string()),
            });
        }

        // Upload with twine
        let mut cmd = Command::new("twine");
        cmd.arg("upload");
        cmd.arg("dist/*");
        cmd.current_dir(&options.path);

        if let Some(ref registry) = options.registry {
            cmd.arg("--repository-url").arg(registry);
        }

        if options.dry_run {
            // twine doesn't have dry-run, use check instead
            cmd = Command::new("twine");
            cmd.arg("check").arg("dist/*");
            cmd.current_dir(&options.path);
        }

        let output = cmd.output().await?;

        Ok(PublishResult {
            success: output.status.success(),
            package_name: self.read_package_name(&options.path).await?,
            version: options.version.clone(),
            registry: options.registry.clone().unwrap_or_else(|| self.registry.clone()),
            url: None,
            error: if !output.status.success() {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            } else {
                None
            },
        })
    }

    async fn validate_credentials(&self) -> Result<bool> {
        // Check for TWINE_USERNAME/TWINE_PASSWORD or TWINE_TOKEN
        if std::env::var("TWINE_TOKEN").is_ok() {
            return Ok(true);
        }
        if std::env::var("TWINE_USERNAME").is_ok() && std::env::var("TWINE_PASSWORD").is_ok() {
            return Ok(true);
        }

        // Check .pypirc
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("No home directory"))?;
        let pypirc = home.join(".pypirc");

        Ok(pypirc.exists())
    }
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
// crates/canaveral-core/src/credentials.rs
use keyring::Entry;
use anyhow::Result;

pub struct CredentialManager;

impl CredentialManager {
    /// Get credential from various sources (priority order)
    pub fn get(&self, service: &str, key: &str) -> Result<Option<String>> {
        // 1. Environment variable
        let env_key = format!("{}_{}", service.to_uppercase(), key.to_uppercase());
        if let Ok(value) = std::env::var(&env_key) {
            return Ok(Some(value));
        }

        // 2. System keychain
        if let Ok(entry) = Entry::new(service, key) {
            if let Ok(password) = entry.get_password() {
                return Ok(Some(password));
            }
        }

        Ok(None)
    }

    /// Store credential in system keychain
    pub fn store(&self, service: &str, key: &str, value: &str) -> Result<()> {
        let entry = Entry::new(service, key)?;
        entry.set_password(value)?;
        Ok(())
    }

    /// Delete credential from system keychain
    pub fn delete(&self, service: &str, key: &str) -> Result<()> {
        let entry = Entry::new(service, key)?;
        entry.delete_password()?;
        Ok(())
    }
}

/// Mask credential for logging
pub fn mask_token(token: &str) -> String {
    if token.len() <= 8 {
        return "*".repeat(token.len());
    }
    format!("{}...{}", &token[..4], &token[token.len()-4..])
}
```

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

pub async fn validate_release(ctx: &ReleaseContext) -> ValidationResult {
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
        if !adapter.validate_credentials().await.unwrap_or(false) {
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
