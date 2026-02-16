//! Go module adapter
//!
//! Go uses git tags for versioning. The go.mod file contains the module path,
//! but versions are determined by git tags like `v1.2.3` or `module/v1.2.3`.

mod gomod;

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;

pub use gomod::GoMod;

/// Go module adapter
pub struct GoAdapter;

impl GoAdapter {
    /// Create a new Go adapter
    pub fn new() -> Self {
        Self
    }

    /// Get the go.mod path
    fn manifest_path(&self, path: &Path) -> PathBuf {
        path.join("go.mod")
    }

    /// Get the latest git tag for this module
    fn get_latest_tag(&self, path: &Path, module_path: &str) -> Result<Option<String>> {
        // Try module-prefixed tags first (for monorepos)
        let prefix = if module_path.contains('/') {
            // Extract the last part as potential prefix
            module_path.rsplit('/').next().unwrap_or("")
        } else {
            ""
        };

        // List tags matching the module prefix
        let output = Command::new("git")
            .args(["tag", "-l", "--sort=-v:refname"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "git tag".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let tags: Vec<&str> = stdout.lines().collect();

        // Look for tags matching this module
        for tag in &tags {
            // Check for prefixed tags (e.g., "module/v1.0.0")
            if !prefix.is_empty() && tag.starts_with(&format!("{}/v", prefix)) {
                return Ok(Some(tag.to_string()));
            }
            // Check for simple version tags
            if tag.starts_with('v') && semver::Version::parse(&tag[1..]).is_ok() {
                return Ok(Some(tag.to_string()));
            }
        }

        Ok(None)
    }

    /// Create a git tag for this module version
    fn create_tag(&self, path: &Path, module_path: &str, version: &str) -> Result<String> {
        // Determine tag format
        let tag = if module_path.contains('/') {
            // Use prefixed tag for submodules
            let prefix = module_path.rsplit('/').next().unwrap_or("");
            if prefix.is_empty() || prefix == module_path {
                format!("v{}", version)
            } else {
                format!("{}/v{}", prefix, version)
            }
        } else {
            format!("v{}", version)
        };

        // Create the tag
        let output = Command::new("git")
            .args(["tag", "-a", &tag, "-m", &format!("Release {}", tag)])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "git tag".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "git tag".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(tag)
    }
}

impl Default for GoAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for GoAdapter {
    fn name(&self) -> &'static str {
        "go"
    }

    fn default_registry(&self) -> &'static str {
        "https://proxy.golang.org"
    }

    fn detect(&self, path: &Path) -> bool {
        let found = self.manifest_path(path).exists();
        debug!(adapter = "go", path = %path.display(), found, "detecting package");
        found
    }

    fn manifest_names(&self) -> &[&str] {
        &["go.mod"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let manifest_path = self.manifest_path(path);
        let gomod = GoMod::load(&manifest_path)?;

        // Get version from git tags
        let version = self
            .get_latest_tag(path, &gomod.module)?
            .map(|t| {
                // Strip prefix if present
                if let Some(v) = t.strip_prefix(&format!("{}/", gomod.module.rsplit('/').next().unwrap_or(""))) {
                    v.strip_prefix('v').unwrap_or(v).to_string()
                } else {
                    t.strip_prefix('v').unwrap_or(&t).to_string()
                }
            })
            .unwrap_or_else(|| "0.0.0".to_string());

        Ok(PackageInfo {
            name: gomod.module.clone(),
            version,
            package_type: "go".to_string(),
            manifest_path,
            private: false, // Go modules are public by default
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let info = self.get_info(path)?;
        debug!(adapter = "go", version = %info.version, "read version");
        Ok(info.version)
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        info!(adapter = "go", version, path = %path.display(), "setting version");
        let manifest_path = self.manifest_path(path);
        let gomod = GoMod::load(&manifest_path)?;

        // For Go, "setting version" means creating a git tag
        self.create_tag(path, &gomod.module, version)?;

        Ok(())
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        info!(adapter = "go", path = %path.display(), dry_run = options.dry_run, "publishing package");
        // Go modules are published via git tags and the Go proxy
        // The main steps are:
        // 1. Ensure go.mod is tidy
        // 2. Create and push the tag

        if options.dry_run {
            // Just verify the module
            let output = Command::new("go")
                .args(["mod", "verify"])
                .current_dir(path)
                .output()
                .map_err(|e| AdapterError::CommandFailed {
                    command: "go mod verify".to_string(),
                    reason: e.to_string(),
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(AdapterError::PublishFailed(stderr.to_string()).into());
            }

            return Ok(());
        }

        // Push the tag to trigger the Go proxy
        let tag = options.tag.as_ref().ok_or_else(|| {
            AdapterError::PublishFailed("No tag specified for Go module publish".to_string())
        })?;

        let output = Command::new("git")
            .args(["push", "origin", tag])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "git push".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::PublishFailed(stderr.to_string()).into());
        }

        // Optionally request the proxy to fetch the version
        if let Some(goproxy) = options.registry.as_ref() {
            let info = self.get_info(path)?;
            let url = format!("{}/{}/@v/{}.info", goproxy, info.name, tag);

            // Make a request to warm the proxy cache
            let _ = Command::new("curl")
                .args(["-s", &url])
                .output();
        }

        Ok(())
    }

    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        debug!(adapter = "go", path = %path.display(), "validating publishable");
        let mut result = ValidationResult::pass();

        // Check go.mod exists
        let manifest_path = self.manifest_path(path);
        if !manifest_path.exists() {
            result.add_error("go.mod not found");
            return Ok(result);
        }

        // Load and validate go.mod
        let gomod = match GoMod::load(&manifest_path) {
            Ok(m) => m,
            Err(e) => {
                result.add_error(format!("Cannot parse go.mod: {}", e));
                return Ok(result);
            }
        };

        // Check module path
        if gomod.module.is_empty() {
            result.add_error("Module path is not set");
        }

        // Check Go version
        if gomod.go_version.is_none() {
            result.add_warning("No Go version specified in go.mod");
        }

        // Verify go.mod is tidy
        let tidy_check = Command::new("go")
            .args(["mod", "tidy", "-diff"])
            .current_dir(path)
            .output();

        if let Ok(output) = tidy_check {
            if !output.stdout.is_empty() {
                result.add_warning("go.mod needs tidying (run 'go mod tidy')");
            }
        }

        // Check for go.sum
        if !path.join("go.sum").exists() {
            result.add_warning("go.sum not found (run 'go mod tidy')");
        }

        // Check this is a git repository
        let git_check = Command::new("git")
            .args(["rev-parse", "--git-dir"])
            .current_dir(path)
            .output();

        if git_check.map(|o| !o.status.success()).unwrap_or(true) {
            result.add_error("Not a git repository (Go requires git tags for versioning)");
        }

        // Run go vet
        let vet_output = Command::new("go")
            .args(["vet", "./..."])
            .current_dir(path)
            .output();

        if let Ok(output) = vet_output {
            if !output.status.success() {
                result.add_warning("go vet reported issues");
            }
        }

        Ok(result)
    }

    fn check_auth(&self, _credentials: &mut CredentialProvider) -> Result<bool> {
        debug!(adapter = "go", "checking authentication");
        // Go modules use git authentication
        // Check if git can push to origin
        let output = Command::new("git")
            .args(["remote", "get-url", "origin"])
            .output();

        Ok(output.map(|o| o.status.success()).unwrap_or(false))
    }

    fn build(&self, path: &Path) -> Result<()> {
        let output = Command::new("go")
            .args(["build", "./..."])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "go build".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "go build".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn test(&self, path: &Path) -> Result<()> {
        let output = Command::new("go")
            .args(["test", "./..."])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "go test".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "go test".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn clean(&self, path: &Path) -> Result<()> {
        let output = Command::new("go")
            .args(["clean", "-cache", "-testcache"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "go clean".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "go clean".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect() {
        let adapter = GoAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(temp.path().join("go.mod"), "module example.com/test\n\ngo 1.21\n").unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_info() {
        let adapter = GoAdapter::new();
        let temp = TempDir::new().unwrap();

        // Initialize git repo
        Command::new("git")
            .args(["init"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        std::fs::write(
            temp.path().join("go.mod"),
            "module github.com/example/test\n\ngo 1.21\n",
        )
        .unwrap();

        let info = adapter.get_info(temp.path()).unwrap();
        assert_eq!(info.name, "github.com/example/test");
        assert_eq!(info.version, "0.0.0"); // No tags yet
        assert_eq!(info.package_type, "go");
    }

    #[test]
    fn test_manifest_names() {
        let adapter = GoAdapter::new();
        assert_eq!(adapter.manifest_names(), &["go.mod"]);
    }
}
