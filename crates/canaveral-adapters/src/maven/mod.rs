//! Maven package adapter
//!
//! Supports Java/Kotlin projects using Maven for build and publishing.

mod pom;

use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;

pub use pom::PomXml;

/// Maven package adapter
pub struct MavenAdapter;

impl MavenAdapter {
    /// Create a new Maven adapter
    pub fn new() -> Self {
        Self
    }

    /// Get the pom.xml path
    fn manifest_path(&self, path: &Path) -> PathBuf {
        path.join("pom.xml")
    }

    /// Get the Maven command (mvn or mvnw)
    fn maven_cmd(&self, path: &Path) -> &'static str {
        let mvnw = path.join("mvnw");
        let mvnw_win = path.join("mvnw.cmd");

        if mvnw.exists() || mvnw_win.exists() {
            "./mvnw"
        } else {
            "mvn"
        }
    }
}

impl Default for MavenAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for MavenAdapter {
    fn name(&self) -> &'static str {
        "maven"
    }

    fn default_registry(&self) -> &'static str {
        "https://repo1.maven.org/maven2"
    }

    fn detect(&self, path: &Path) -> bool {
        let found = self.manifest_path(path).exists();
        debug!(adapter = "maven", path = %path.display(), found, "detecting package");
        found
    }

    fn manifest_names(&self) -> &[&str] {
        &["pom.xml"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let manifest_path = self.manifest_path(path);
        let pom = PomXml::load(&manifest_path)?;

        let group_id = pom.group_id.ok_or_else(|| {
            AdapterError::ManifestParseError("No groupId found in pom.xml".to_string())
        })?;

        let artifact_id = pom.artifact_id.ok_or_else(|| {
            AdapterError::ManifestParseError("No artifactId found in pom.xml".to_string())
        })?;

        let version = pom.version.ok_or_else(|| {
            AdapterError::ManifestParseError("No version found in pom.xml".to_string())
        })?;

        // Maven uses groupId:artifactId as the full name
        let name = format!("{}:{}", group_id, artifact_id);

        Ok(PackageInfo {
            name,
            version,
            package_type: "maven".to_string(),
            manifest_path,
            private: false,
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let pom = PomXml::load(&self.manifest_path(path))?;
        let version = pom.version.ok_or_else(|| {
            AdapterError::ManifestParseError("No version found in pom.xml".to_string())
        })?;
        debug!(adapter = "maven", version = %version, "read version");
        Ok(version)
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        info!(adapter = "maven", version, path = %path.display(), "setting version");
        let manifest_path = self.manifest_path(path);
        PomXml::update_version(&manifest_path, version)
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        info!(adapter = "maven", path = %path.display(), dry_run = options.dry_run, "publishing package");
        let mvn = self.maven_cmd(path);
        let mut cmd = Command::new(mvn);
        cmd.current_dir(path);

        if options.dry_run {
            // Just validate
            cmd.args(["validate"]);
        } else {
            // Deploy to repository
            cmd.args(["deploy", "-DskipTests"]);

            // Alternative repository
            if let Some(ref registry) = options.registry {
                cmd.arg(format!(
                    "-DaltDeploymentRepository=release::default::{}",
                    registry
                ));
            }

            // Skip GPG signing if specified
            if options.extra.get("skip_gpg").is_some_and(|v| v == "true") {
                cmd.arg("-Dgpg.skip=true");
            }
        }

        // Batch mode (non-interactive)
        cmd.arg("-B");

        let output = cmd.output().map_err(|e| AdapterError::CommandFailed {
            command: format!("{} deploy", mvn),
            reason: e.to_string(),
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);
            return Err(AdapterError::PublishFailed(format!(
                "Maven deploy failed:\n{}\n{}",
                stdout, stderr
            ))
            .into());
        }

        Ok(())
    }

    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        debug!(adapter = "maven", path = %path.display(), "validating publishable");
        let mut result = ValidationResult::pass();

        // Check pom.xml exists
        let manifest_path = self.manifest_path(path);
        if !manifest_path.exists() {
            result.add_error("pom.xml not found");
            return Ok(result);
        }

        // Load and validate pom.xml
        let pom = match PomXml::load(&manifest_path) {
            Ok(p) => p,
            Err(e) => {
                result.add_error(format!("Cannot parse pom.xml: {}", e));
                return Ok(result);
            }
        };

        // Required fields
        if pom.group_id.is_none() {
            result.add_error("groupId is not set");
        }

        if pom.artifact_id.is_none() {
            result.add_error("artifactId is not set");
        }

        if pom.version.is_none() {
            result.add_error("version is not set");
        } else if let Some(ref v) = pom.version {
            if v.contains("SNAPSHOT") {
                result
                    .add_warning("Version contains SNAPSHOT - use release version for publishing");
            }
        }

        // Recommended fields for Maven Central
        if pom.name.is_none() {
            result.add_warning("name is not set (required for Maven Central)");
        }

        if pom.description.is_none() {
            result.add_warning("description is not set (required for Maven Central)");
        }

        if pom.url.is_none() {
            result.add_warning("url is not set (required for Maven Central)");
        }

        if pom.licenses.is_empty() {
            result.add_warning("No licenses defined (required for Maven Central)");
        }

        if pom.developers.is_empty() {
            result.add_warning("No developers defined (required for Maven Central)");
        }

        if pom.scm.is_none() {
            result.add_warning("No SCM information (required for Maven Central)");
        }

        // Run Maven validate
        let mvn = self.maven_cmd(path);
        let validate_output = Command::new(mvn)
            .args(["validate", "-B"])
            .current_dir(path)
            .output();

        if let Ok(output) = validate_output {
            if !output.status.success() {
                result.add_error("Maven validation failed");
            }
        }

        Ok(result)
    }

    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        debug!(adapter = "maven", "checking authentication");
        // Check our credential provider
        if credentials.has_credentials("maven") {
            return Ok(true);
        }

        // Check for Maven settings.xml with server credentials
        let m2_home = std::env::var("M2_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".m2")));

        if let Some(m2) = m2_home {
            let settings = m2.join("settings.xml");
            if settings.exists() {
                // Check if settings contains server credentials
                if let Ok(content) = std::fs::read_to_string(&settings) {
                    return Ok(content.contains("<server>"));
                }
            }
        }

        Ok(false)
    }

    fn build(&self, path: &Path) -> Result<()> {
        let mvn = self.maven_cmd(path);
        let output = Command::new(mvn)
            .args(["package", "-DskipTests", "-B"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: format!("{} package", mvn),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: format!("{} package", mvn),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn test(&self, path: &Path) -> Result<()> {
        let mvn = self.maven_cmd(path);
        let output = Command::new(mvn)
            .args(["test", "-B"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: format!("{} test", mvn),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: format!("{} test", mvn),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn clean(&self, path: &Path) -> Result<()> {
        let mvn = self.maven_cmd(path);
        let output = Command::new(mvn)
            .args(["clean", "-B"])
            .current_dir(path)
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: format!("{} clean", mvn),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AdapterError::CommandFailed {
                command: format!("{} clean", mvn),
                reason: stderr.to_string(),
            }
            .into());
        }

        Ok(())
    }

    fn pack(&self, path: &Path) -> Result<Option<PathBuf>> {
        // Build the package
        self.build(path)?;

        // Find the built artifact
        let pom = PomXml::load(&self.manifest_path(path))?;
        let artifact_id = pom.artifact_id.unwrap_or_default();
        let version = pom.version.unwrap_or_default();
        let packaging = pom.packaging.unwrap_or_else(|| "jar".to_string());

        let artifact_path = path
            .join("target")
            .join(format!("{}-{}.{}", artifact_id, version, packaging));

        if artifact_path.exists() {
            Ok(Some(artifact_path))
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
        let adapter = MavenAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(
            temp.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0.0</version>
</project>"#,
        )
        .unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_info() {
        let adapter = MavenAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("pom.xml"),
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>my-lib</artifactId>
    <version>2.0.0</version>
</project>"#,
        )
        .unwrap();

        let info = adapter.get_info(temp.path()).unwrap();
        assert_eq!(info.name, "com.example:my-lib");
        assert_eq!(info.version, "2.0.0");
        assert_eq!(info.package_type, "maven");
    }

    #[test]
    fn test_manifest_names() {
        let adapter = MavenAdapter::new();
        assert_eq!(adapter.manifest_names(), &["pom.xml"]);
    }
}
