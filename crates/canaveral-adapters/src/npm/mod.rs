//! npm package adapter

mod manifest;

use std::path::{Path, PathBuf};
use std::process::Command;

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;
pub use manifest::PackageJson;

/// npm package adapter
pub struct NpmAdapter;

impl NpmAdapter {
    /// Create a new npm adapter
    pub fn new() -> Self {
        Self
    }

    /// Get the package.json path
    fn manifest_path(&self, path: &Path) -> PathBuf {
        path.join("package.json")
    }

    /// Check if package name is scoped (@scope/name)
    fn is_scoped_package(&self, name: &str) -> bool {
        name.starts_with('@')
    }
}

impl Default for NpmAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for NpmAdapter {
    fn name(&self) -> &'static str {
        "npm"
    }

    fn default_registry(&self) -> &'static str {
        "https://registry.npmjs.org"
    }

    fn detect(&self, path: &Path) -> bool {
        self.manifest_path(path).exists()
    }

    fn manifest_names(&self) -> &[&str] {
        &["package.json"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let manifest_path = self.manifest_path(path);
        let manifest = PackageJson::load(&manifest_path)?;

        Ok(PackageInfo {
            name: manifest.name,
            version: manifest.version,
            package_type: "npm".to_string(),
            manifest_path,
            private: manifest.private.unwrap_or(false),
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let manifest = PackageJson::load(&self.manifest_path(path))?;
        Ok(manifest.version)
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        let manifest_path = self.manifest_path(path);
        let mut manifest = PackageJson::load(&manifest_path)?;
        manifest.version = version.to_string();
        manifest.save(&manifest_path)?;
        Ok(())
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        let mut cmd = Command::new("npm");
        cmd.arg("publish");
        cmd.current_dir(path);

        if options.dry_run {
            cmd.arg("--dry-run");
        }

        // Registry
        if let Some(ref registry) = options.registry {
            cmd.arg("--registry").arg(registry);
        }

        // Access level
        if let Some(ref access) = options.access {
            cmd.arg("--access").arg(access.to_string());
        } else {
            // Default scoped packages to public unless specified
            let info = self.get_info(path)?;
            if self.is_scoped_package(&info.name) {
                cmd.arg("--access").arg("public");
            }
        }

        // Tag
        if let Some(ref tag) = options.tag {
            cmd.arg("--tag").arg(tag);
        }

        // OTP
        if let Some(ref otp) = options.otp {
            cmd.arg("--otp").arg(otp);
        }

        let output = cmd
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "npm publish".to_string(),
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
        let manifest = match PackageJson::load(&self.manifest_path(path)) {
            Ok(m) => m,
            Err(e) => {
                result.add_error(format!("Cannot read package.json: {}", e));
                return Ok(result);
            }
        };

        // Check if private
        if manifest.private.unwrap_or(false) {
            result.add_error("Package is marked as private");
        }

        // Check name
        if manifest.name.is_empty() {
            result.add_error("Package name is not set");
        }

        // Check version
        if manifest.version.is_empty() {
            result.add_error("Package version is not set");
        }

        // Validate version is valid semver
        if semver::Version::parse(&manifest.version).is_err() {
            result.add_error(format!("Version '{}' is not valid semver", manifest.version));
        }

        // Check for required fields
        if manifest.description.is_none() {
            result.add_warning("Package has no description");
        }

        // Check for main/module/exports
        if manifest.main.is_none() && manifest.module.is_none() && manifest.exports.is_none() {
            result.add_warning("Package has no main, module, or exports field");
        }

        // Check files field or .npmignore
        if manifest.files.is_none() {
            let npmignore = path.join(".npmignore");
            if !npmignore.exists() {
                result.add_warning("No 'files' field or .npmignore - entire directory will be published");
            }
        }

        Ok(result)
    }

    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        // First check our credential provider
        if credentials.has_credentials("npm") {
            return Ok(true);
        }

        // Fallback: try `npm whoami` to check if logged in
        let output = Command::new("npm")
            .args(["whoami"])
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "npm whoami".to_string(),
                reason: e.to_string(),
            })?;

        Ok(output.status.success())
    }

    fn build(&self, path: &Path) -> Result<()> {
        // Check if there's a build script
        let manifest = PackageJson::load(&self.manifest_path(path))?;

        if manifest.scripts.as_ref().is_some_and(|s| s.contains_key("build")) {
            let output = Command::new("npm")
                .args(["run", "build"])
                .current_dir(path)
                .output()
                .map_err(|e| AdapterError::CommandFailed {
                    command: "npm run build".to_string(),
                    reason: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AdapterError::CommandFailed {
                    command: "npm run build".to_string(),
                    reason: stderr.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    fn test(&self, path: &Path) -> Result<()> {
        let manifest = PackageJson::load(&self.manifest_path(path))?;

        if manifest.scripts.as_ref().is_some_and(|s| s.contains_key("test")) {
            let output = Command::new("npm")
                .args(["test"])
                .current_dir(path)
                .output()
                .map_err(|e| AdapterError::CommandFailed {
                    command: "npm test".to_string(),
                    reason: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AdapterError::CommandFailed {
                    command: "npm test".to_string(),
                    reason: stderr.to_string(),
                }
                .into());
            }
        }

        Ok(())
    }

    fn clean(&self, path: &Path) -> Result<()> {
        // Remove node_modules and common build artifacts
        let node_modules = path.join("node_modules");
        if node_modules.exists() {
            std::fs::remove_dir_all(&node_modules)?;
        }

        let dist = path.join("dist");
        if dist.exists() {
            std::fs::remove_dir_all(&dist)?;
        }

        let build = path.join("build");
        if build.exists() {
            std::fs::remove_dir_all(&build)?;
        }

        Ok(())
    }

    fn pack(&self, path: &Path) -> Result<Option<PathBuf>> {
        let output = Command::new("npm")
            .args(["pack"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "npm pack".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "npm pack".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        // npm pack outputs the filename
        let stdout = String::from_utf8_lossy(&output.stdout);
        let tarball_name = stdout.trim();
        if !tarball_name.is_empty() {
            Ok(Some(path.join(tarball_name)))
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
        let adapter = NpmAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "version": "1.0.0"}"#,
        )
        .unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_version() {
        let adapter = NpmAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "version": "1.2.3"}"#,
        )
        .unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_set_version() {
        let adapter = NpmAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "test", "version": "1.0.0"}"#,
        )
        .unwrap();

        adapter.set_version(temp.path(), "2.0.0").unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version, "2.0.0");
    }
}
