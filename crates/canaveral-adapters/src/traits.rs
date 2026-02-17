//! Package adapter traits

use std::path::Path;

use canaveral_core::error::Result;
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};

/// Trait for package adapters
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
    fn publish(&self, path: &Path, dry_run: bool) -> Result<()> {
        let options = PublishOptions::new().dry_run(dry_run);
        self.publish_with_options(path, &options)
    }

    /// Publish package with detailed options
    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()>;

    /// Validate that the package can be published
    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        let mut result = ValidationResult::pass();

        // Check manifest exists and is valid
        if let Err(e) = self.get_info(path) {
            result.add_error(format!("Invalid manifest: {}", e));
            return Ok(result);
        }

        // Check version is set
        match self.get_version(path) {
            Ok(version) if version.is_empty() => {
                result.add_error("Version is not set");
            }
            Err(e) => {
                result.add_error(format!("Cannot read version: {}", e));
            }
            _ => {}
        }

        Ok(result)
    }

    /// Check if authentication is configured for publishing
    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        Ok(credentials.has_credentials(self.name()))
    }

    /// Get the manifest filename(s) this adapter handles
    fn manifest_names(&self) -> &[&str];

    /// Format source code (if applicable)
    /// When `check` is true, verify formatting without applying changes.
    fn fmt(&self, _path: &Path, _check: bool) -> Result<()> {
        Ok(())
    }

    /// Run linter (if applicable)
    fn lint(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    /// Build the package (if applicable)
    fn build(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    /// Run tests (if applicable)
    fn test(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    /// Clean build artifacts (if applicable)
    fn clean(&self, _path: &Path) -> Result<()> {
        Ok(())
    }

    /// Pack the package for publishing without actually publishing
    fn pack(&self, _path: &Path) -> Result<Option<std::path::PathBuf>> {
        Ok(None)
    }
}
