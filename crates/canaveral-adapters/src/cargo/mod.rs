//! Cargo package adapter

mod manifest;

use std::path::{Path, PathBuf};
use std::process::Command;

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;
pub use manifest::CargoToml;

/// Cargo package adapter
pub struct CargoAdapter;

impl CargoAdapter {
    /// Create a new Cargo adapter
    pub fn new() -> Self {
        Self
    }

    /// Get the Cargo.toml path
    fn manifest_path(&self, path: &Path) -> PathBuf {
        path.join("Cargo.toml")
    }
}

impl Default for CargoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for CargoAdapter {
    fn name(&self) -> &'static str {
        "cargo"
    }

    fn default_registry(&self) -> &'static str {
        "https://crates.io"
    }

    fn detect(&self, path: &Path) -> bool {
        let manifest = self.manifest_path(path);
        if !manifest.exists() {
            return false;
        }

        // Check if it's a package (not just a workspace)
        if let Ok(toml) = CargoToml::load(&manifest) {
            toml.package.is_some()
        } else {
            false
        }
    }

    fn manifest_names(&self) -> &[&str] {
        &["Cargo.toml"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let manifest_path = self.manifest_path(path);
        let manifest = CargoToml::load(&manifest_path)?;

        let package = manifest.package.ok_or_else(|| {
            AdapterError::ManifestParseError("No [package] section found".to_string())
        })?;

        Ok(PackageInfo {
            name: package.name,
            version: package.version,
            package_type: "cargo".to_string(),
            manifest_path,
            private: package.publish.is_some_and(|p| !p),
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let manifest = CargoToml::load(&self.manifest_path(path))?;

        manifest
            .package
            .map(|p| p.version)
            .ok_or_else(|| {
                AdapterError::ManifestParseError("No [package] section found".to_string()).into()
            })
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        let manifest_path = self.manifest_path(path);
        CargoToml::update_version(&manifest_path, version)
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        let mut cmd = Command::new("cargo");
        cmd.arg("publish");
        cmd.current_dir(path);

        if options.dry_run {
            cmd.arg("--dry-run");
        }

        // Registry
        if let Some(ref registry) = options.registry {
            cmd.arg("--registry").arg(registry);
        }

        // Token (if provided via extra options)
        if let Some(token) = options.extra.get("token") {
            cmd.arg("--token").arg(token);
        }

        // Allow dirty (if specified)
        if options.extra.get("allow_dirty").is_some_and(|v| v == "true") {
            cmd.arg("--allow-dirty");
        }

        // No verify (if specified)
        if options.extra.get("no_verify").is_some_and(|v| v == "true") {
            cmd.arg("--no-verify");
        }

        let output = cmd
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo publish".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::PublishFailed(stderr.to_string()).into());
        }

        Ok(())
    }

    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        let mut result = ValidationResult::pass();

        // Check manifest
        let manifest = match CargoToml::load(&self.manifest_path(path)) {
            Ok(m) => m,
            Err(e) => {
                result.add_error(format!("Cannot read Cargo.toml: {}", e));
                return Ok(result);
            }
        };

        let package = match manifest.package {
            Some(p) => p,
            None => {
                result.add_error("No [package] section found");
                return Ok(result);
            }
        };

        // Check if publish is disabled
        if package.publish.is_some_and(|p| !p) {
            result.add_error("Package has publish = false");
        }

        // Check name
        if package.name.is_empty() {
            result.add_error("Package name is not set");
        }

        // Validate crate name (no uppercase, special chars)
        if !package.name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_') {
            result.add_error("Crate name must be lowercase alphanumeric with - or _");
        }

        // Check version
        if package.version.is_empty() {
            result.add_error("Package version is not set");
        }

        // Validate version is valid semver
        if semver::Version::parse(&package.version).is_err() {
            result.add_error(format!("Version '{}' is not valid semver", package.version));
        }

        // Check for required metadata
        if package.description.is_none() {
            result.add_warning("Package has no description (recommended for crates.io)");
        }

        if package.license.is_none() && package.license_file.is_none() {
            result.add_warning("Package has no license (required for crates.io)");
        }

        if package.repository.is_none() {
            result.add_warning("Package has no repository URL");
        }

        // Check Cargo.lock exists for binaries
        if package.is_binary() {
            let cargo_lock = path.join("Cargo.lock");
            if !cargo_lock.exists() {
                result.add_warning("No Cargo.lock found (recommended for binary crates)");
            }
        }

        // Run cargo check for syntax/dependency validation
        let check_output = Command::new("cargo")
            .args(["check", "--quiet"])
            .current_dir(path)
            .output();

        if let Ok(output) = check_output {
            if !output.status.success() {
                result.add_error("cargo check failed - fix compilation errors first");
            }
        }

        Ok(result)
    }

    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        // Check our credential provider first
        if credentials.has_credentials("cargo") {
            return Ok(true);
        }

        // Check if cargo credentials exist
        let cargo_home = std::env::var("CARGO_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))
            .ok_or_else(|| AdapterError::AuthenticationFailed {
                registry: "cargo".to_string(),
                reason: "Could not determine CARGO_HOME".to_string(),
            })?;

        let creds_path = cargo_home.join("credentials.toml");
        let alt_path = cargo_home.join("credentials");

        Ok(creds_path.exists() || alt_path.exists())
    }

    fn build(&self, path: &Path) -> Result<()> {
        let output = Command::new("cargo")
            .args(["build", "--release"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo build".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "cargo build".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn test(&self, path: &Path) -> Result<()> {
        let output = Command::new("cargo")
            .args(["test"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo test".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "cargo test".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn clean(&self, path: &Path) -> Result<()> {
        let output = Command::new("cargo")
            .args(["clean"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo clean".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "cargo clean".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn pack(&self, path: &Path) -> Result<Option<PathBuf>> {
        let output = Command::new("cargo")
            .args(["package", "--list"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo package".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "cargo package".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        // Actually create the package
        let package_output = Command::new("cargo")
            .args(["package"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "cargo package".to_string(),
                reason: e.to_string(),
            })?;

        if !package_output.status.success() {
            let stderr = String::from_utf8_lossy(&package_output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "cargo package".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        // Find the created .crate file
        let info = self.get_info(path)?;
        let crate_file = path
            .join("target")
            .join("package")
            .join(format!("{}-{}.crate", info.name, info.version));

        if crate_file.exists() {
            Ok(Some(crate_file))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect() {
        let adapter = CargoAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "1.0.0"
"#,
        )
        .unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_detect_workspace_only() {
        let adapter = CargoAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();

        // Workspace without [package] should not be detected as a package
        assert!(!adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_version() {
        let adapter = CargoAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "1.2.3"
"#,
        )
        .unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_set_version() {
        let adapter = CargoAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("Cargo.toml"),
            r#"
[package]
name = "test"
version = "1.0.0"
edition = "2021"
"#,
        )
        .unwrap();

        adapter.set_version(temp.path(), "2.0.0").unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version, "2.0.0");
    }
}
