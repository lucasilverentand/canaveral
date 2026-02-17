//! Python package adapter

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;
use toml_edit::{value, DocumentMut};

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;

/// Python package adapter (using pyproject.toml)
pub struct PythonAdapter;

impl PythonAdapter {
    /// Create a new Python adapter
    pub fn new() -> Self {
        Self
    }

    /// Get the pyproject.toml path
    fn manifest_path(&self, path: &Path) -> PathBuf {
        path.join("pyproject.toml")
    }

    /// Load pyproject.toml
    fn load_manifest(&self, path: &Path) -> Result<DocumentMut> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        content.parse().map_err(|e: toml_edit::TomlError| {
            AdapterError::ManifestParseError(e.to_string()).into()
        })
    }

    /// Get project name from pyproject.toml
    fn get_name(&self, doc: &DocumentMut) -> Option<String> {
        doc.get("project")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .map(|s| s.to_string())
    }

    /// Get project version from pyproject.toml
    fn get_version_from_doc(&self, doc: &DocumentMut) -> Option<String> {
        doc.get("project")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    /// Get project description from pyproject.toml
    fn get_description(&self, doc: &DocumentMut) -> Option<String> {
        doc.get("project")
            .and_then(|p| p.get("description"))
            .and_then(|d| d.as_str())
            .map(|s| s.to_string())
    }

    /// Check if project has readme
    fn has_readme(&self, doc: &DocumentMut) -> bool {
        doc.get("project").and_then(|p| p.get("readme")).is_some()
    }

    /// Check if project has license
    fn has_license(&self, doc: &DocumentMut) -> bool {
        doc.get("project").and_then(|p| p.get("license")).is_some()
    }

    /// Get the dist directory
    fn dist_path(&self, path: &Path) -> PathBuf {
        path.join("dist")
    }
}

impl Default for PythonAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for PythonAdapter {
    fn name(&self) -> &'static str {
        "python"
    }

    fn default_registry(&self) -> &'static str {
        "https://upload.pypi.org/legacy/"
    }

    fn detect(&self, path: &Path) -> bool {
        let manifest = self.manifest_path(path);
        if !manifest.exists() {
            debug!(adapter = "python", path = %path.display(), found = false, "detecting package");
            return false;
        }

        // Check if it has [project] section
        let found = if let Ok(doc) = self.load_manifest(&manifest) {
            doc.get("project").is_some()
        } else {
            false
        };
        debug!(adapter = "python", path = %path.display(), found, "detecting package");
        found
    }

    fn manifest_names(&self) -> &[&str] {
        &["pyproject.toml"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let manifest_path = self.manifest_path(path);
        let doc = self.load_manifest(&manifest_path)?;

        let name = self
            .get_name(&doc)
            .ok_or_else(|| AdapterError::ManifestParseError("No project.name found".to_string()))?;

        let version = self.get_version_from_doc(&doc).ok_or_else(|| {
            AdapterError::ManifestParseError("No project.version found".to_string())
        })?;

        Ok(PackageInfo {
            name,
            version,
            package_type: "python".to_string(),
            manifest_path,
            private: false,
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let doc = self.load_manifest(&self.manifest_path(path))?;

        let version = self.get_version_from_doc(&doc).ok_or_else(|| {
            AdapterError::ManifestParseError("No project.version found".to_string())
        })?;
        debug!(adapter = "python", version = %version, "read version");
        Ok(version)
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        info!(adapter = "python", version, path = %path.display(), "setting version");
        let manifest_path = self.manifest_path(path);
        let content = std::fs::read_to_string(&manifest_path)
            .map_err(|_| AdapterError::ManifestNotFound(manifest_path.clone()))?;

        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e: toml_edit::TomlError| AdapterError::ManifestParseError(e.to_string()))?;

        if let Some(project) = doc.get_mut("project") {
            if let Some(table) = project.as_table_mut() {
                table["version"] = value(version);
            }
        } else {
            return Err(
                AdapterError::ManifestParseError("No [project] section found".to_string()).into(),
            );
        }

        std::fs::write(&manifest_path, doc.to_string())
            .map_err(|e| AdapterError::ManifestUpdateError(e.to_string()).into())
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        info!(adapter = "python", path = %path.display(), dry_run = options.dry_run, "publishing package");
        // Build first (unless already built)
        let dist = self.dist_path(path);
        if !dist.exists() || std::fs::read_dir(&dist).map(|d| d.count()).unwrap_or(0) == 0 {
            self.build(path)?;
        }

        if options.dry_run {
            // For dry run, just check the package with twine
            let check_output = Command::new("twine")
                .args(["check", "dist/*"])
                .current_dir(path)
                .output()
                .map_err(|e| AdapterError::CommandFailed {
                    command: "twine check".to_string(),
                    reason: e.to_string(),
                })?;

            if !check_output.status.success() {
                let stderr = String::from_utf8_lossy(&check_output.stderr);
                return Err(AdapterError::PublishFailed(stderr.to_string()).into());
            }

            return Ok(());
        }

        // Publish with twine
        let mut cmd = Command::new("twine");
        cmd.arg("upload");
        cmd.current_dir(path);

        // Repository URL
        if let Some(ref registry) = options.registry {
            cmd.arg("--repository-url").arg(registry);
        }

        // Username/password from extra options or env
        if let Some(username) = options.extra.get("username") {
            cmd.arg("--username").arg(username);
        }
        if let Some(password) = options.extra.get("password") {
            cmd.arg("--password").arg(password);
        }

        // Skip existing (useful for retries)
        if options
            .extra
            .get("skip_existing")
            .is_some_and(|v| v == "true")
        {
            cmd.arg("--skip-existing");
        }

        // Add dist files
        cmd.arg("dist/*");

        let output = cmd.output().map_err(|e| AdapterError::CommandFailed {
            command: "twine upload".to_string(),
            reason: e.to_string(),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::PublishFailed(stderr.to_string()).into());
        }

        Ok(())
    }

    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        debug!(adapter = "python", path = %path.display(), "validating publishable");
        let mut result = ValidationResult::pass();

        // Check manifest
        let manifest_path = self.manifest_path(path);
        let doc = match self.load_manifest(&manifest_path) {
            Ok(d) => d,
            Err(e) => {
                result.add_error(format!("Cannot read pyproject.toml: {}", e));
                return Ok(result);
            }
        };

        // Check name
        let name = match self.get_name(&doc) {
            Some(n) => n,
            None => {
                result.add_error("No project.name found");
                return Ok(result);
            }
        };

        // Validate package name (PEP 503 normalized)
        let normalized = name.to_lowercase().replace(['-', '.', '_'], "-");
        if name != normalized {
            result.add_warning(format!(
                "Package name '{}' will be normalized to '{}' on PyPI",
                name, normalized
            ));
        }

        // Check version
        match self.get_version_from_doc(&doc) {
            Some(v) if v.is_empty() => {
                result.add_error("Version is empty");
            }
            None => {
                result.add_error("No project.version found");
            }
            _ => {}
        }

        // Check for description
        if self.get_description(&doc).is_none() {
            result.add_warning("No description found (recommended for PyPI)");
        }

        // Check for readme
        if !self.has_readme(&doc) {
            result.add_warning("No readme specified (recommended for PyPI)");
        }

        // Check for license
        if !self.has_license(&doc) {
            result.add_warning("No license specified (recommended for PyPI)");
        }

        // Check for build system
        if doc.get("build-system").is_none() {
            result.add_warning("No [build-system] section found");
        }

        // Check that required tools are available
        let python_check = Command::new("python").args(["--version"]).output();
        if python_check.is_err() {
            result.add_error("Python is not available");
        }

        let build_check = Command::new("python")
            .args(["-m", "build", "--version"])
            .output();
        if build_check.is_err() || !build_check.unwrap().status.success() {
            result.add_warning("python-build is not installed (pip install build)");
        }

        let twine_check = Command::new("twine").args(["--version"]).output();
        if twine_check.is_err() || !twine_check.unwrap().status.success() {
            result.add_warning("twine is not installed (pip install twine)");
        }

        Ok(result)
    }

    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        debug!(adapter = "python", "checking authentication");
        // Check our credential provider
        if credentials.has_credentials("pypi") {
            return Ok(true);
        }

        // Check for .pypirc
        if let Some(home) = dirs::home_dir() {
            let pypirc = home.join(".pypirc");
            if pypirc.exists() {
                return Ok(true);
            }
        }

        // Check environment variables
        if std::env::var("TWINE_USERNAME").is_ok() && std::env::var("TWINE_PASSWORD").is_ok() {
            return Ok(true);
        }

        if std::env::var("PYPI_TOKEN").is_ok() {
            return Ok(true);
        }

        Ok(false)
    }

    fn fmt(&self, path: &Path, check: bool) -> Result<()> {
        let mut cmd = Command::new("ruff");
        cmd.arg("format").current_dir(path);
        if check {
            cmd.arg("--check");
        }
        cmd.arg(".");

        let output = cmd.output().map_err(|e| AdapterError::CommandFailed {
            command: "ruff format".to_string(),
            reason: e.to_string(),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "ruff format".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn lint(&self, path: &Path) -> Result<()> {
        let output = Command::new("ruff")
            .args(["check", "."])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "ruff check".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "ruff check".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn build(&self, path: &Path) -> Result<()> {
        let output = Command::new("python")
            .args(["-m", "build"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "python -m build".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "python -m build".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn test(&self, path: &Path) -> Result<()> {
        let output = Command::new("python")
            .args(["-m", "pytest"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "python -m pytest".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: "python -m pytest".to_string(),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn clean(&self, path: &Path) -> Result<()> {
        // Remove dist directory
        let dist = self.dist_path(path);
        if dist.exists() {
            std::fs::remove_dir_all(&dist)?;
        }

        // Remove build directory
        let build = path.join("build");
        if build.exists() {
            std::fs::remove_dir_all(&build)?;
        }

        // Remove egg-info directories
        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".egg-info") && entry.path().is_dir() {
                std::fs::remove_dir_all(entry.path())?;
            }
        }

        // Remove __pycache__ directories
        fn remove_pycache(dir: &Path) -> std::io::Result<()> {
            if dir.is_dir() {
                for entry in std::fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        if entry.file_name() == "__pycache__" {
                            std::fs::remove_dir_all(&path)?;
                        } else {
                            remove_pycache(&path)?;
                        }
                    }
                }
            }
            Ok(())
        }
        let _ = remove_pycache(path);

        Ok(())
    }

    fn pack(&self, path: &Path) -> Result<Option<PathBuf>> {
        // Build the package
        self.build(path)?;

        // Find the wheel file
        let dist = self.dist_path(path);
        if !dist.exists() {
            return Ok(None);
        }

        for entry in std::fs::read_dir(&dist)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".whl") {
                return Ok(Some(entry.path()));
            }
        }

        // Fallback to tarball
        for entry in std::fs::read_dir(&dist)? {
            let entry = entry?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.ends_with(".tar.gz") {
                return Ok(Some(entry.path()));
            }
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect() {
        let adapter = PythonAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
name = "test"
version = "1.0.0"
"#,
        )
        .unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_version() {
        let adapter = PythonAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
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
        let adapter = PythonAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
name = "test"
version = "1.0.0"
description = "A test"
"#,
        )
        .unwrap();

        adapter.set_version(temp.path(), "2.0.0").unwrap();

        let version = adapter.get_version(temp.path()).unwrap();
        assert_eq!(version, "2.0.0");

        // Check formatting preserved
        let content = std::fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
        assert!(content.contains("description"));
    }
}
