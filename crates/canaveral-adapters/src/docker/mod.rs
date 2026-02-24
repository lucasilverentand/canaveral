//! Docker image adapter
//!
//! Supports building and pushing Docker images to multiple registries.

mod parser;
mod registry;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use tracing::{debug, info};

use canaveral_core::error::{AdapterError, Result};
use canaveral_core::types::PackageInfo;

use crate::credentials::CredentialProvider;
use crate::publish::{PublishOptions, ValidationResult};
use crate::traits::PackageAdapter;

/// Docker image adapter
pub struct DockerAdapter {
    /// Additional tags to apply
    additional_tags: Vec<String>,
    /// Registries to push to
    registries: Vec<String>,
    /// Build arguments
    build_args: HashMap<String, String>,
    /// Target platform(s)
    platforms: Vec<String>,
}

impl DockerAdapter {
    /// Create a new Docker adapter
    pub fn new() -> Self {
        Self {
            additional_tags: Vec::new(),
            registries: Vec::new(),
            build_args: HashMap::new(),
            platforms: Vec::new(),
        }
    }

    /// Add additional tags (e.g., "latest", "stable")
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.additional_tags = tags;
        self
    }

    /// Add registries to push to
    pub fn with_registries(mut self, registries: Vec<String>) -> Self {
        self.registries = registries;
        self
    }

    /// Add build arguments
    pub fn with_build_args(mut self, args: HashMap<String, String>) -> Self {
        self.build_args = args;
        self
    }

    /// Set target platforms for multi-arch builds
    pub fn with_platforms(mut self, platforms: Vec<String>) -> Self {
        self.platforms = platforms;
        self
    }

    /// Get the Dockerfile path
    fn dockerfile_path(&self, path: &Path) -> PathBuf {
        path.join("Dockerfile")
    }
}

impl Default for DockerAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageAdapter for DockerAdapter {
    fn name(&self) -> &'static str {
        "docker"
    }

    fn default_registry(&self) -> &'static str {
        "docker.io"
    }

    fn detect(&self, path: &Path) -> bool {
        let found = self.dockerfile_path(path).exists();
        debug!(adapter = "docker", path = %path.display(), found, "detecting package");
        found
    }

    fn manifest_names(&self) -> &[&str] {
        &["Dockerfile"]
    }

    fn get_info(&self, path: &Path) -> Result<PackageInfo> {
        let (name, version) = parser::parse_image_info(path)?;

        Ok(PackageInfo {
            name,
            version,
            package_type: "docker".to_string(),
            manifest_path: self.dockerfile_path(path),
            private: false,
        })
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let (_, version) = parser::parse_image_info(path)?;
        debug!(adapter = "docker", version = %version, "read version");
        Ok(version)
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        info!(adapter = "docker", version, path = %path.display(), "setting version");
        let dockerfile = self.dockerfile_path(path);
        if !dockerfile.exists() {
            return Err(
                AdapterError::ManifestParseError("Dockerfile not found".to_string()).into(),
            );
        }

        let content = std::fs::read_to_string(&dockerfile)?;
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let mut found = false;

        for line in &mut lines {
            let trimmed = line.trim();
            // Update OCI version label
            if trimmed.starts_with("LABEL org.opencontainers.image.version=") {
                *line = format!("LABEL org.opencontainers.image.version=\"{}\"", version);
                found = true;
                break;
            }
            // Update simple version label
            if trimmed.starts_with("LABEL version=") {
                *line = format!("LABEL version=\"{}\"", version);
                found = true;
                break;
            }
        }

        if !found {
            // Add version label after FROM instruction
            let insert_pos = lines
                .iter()
                .position(|l| l.trim().starts_with("FROM"))
                .map(|i| i + 1)
                .unwrap_or(0);

            lines.insert(
                insert_pos,
                format!("LABEL org.opencontainers.image.version=\"{}\"", version),
            );
        }

        std::fs::write(&dockerfile, lines.join("\n"))?;
        Ok(())
    }

    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()> {
        info!(adapter = "docker", path = %path.display(), dry_run = options.dry_run, "publishing image");
        let (name, version) = parser::parse_image_info(path)?;

        // Determine registries to push to
        let registries: Vec<String> = if !self.registries.is_empty() {
            self.registries.clone()
        } else if let Some(ref reg) = options.registry {
            vec![reg.clone()]
        } else {
            vec![self.default_registry().to_string()]
        };

        // Build the primary tag
        let primary_tag = registry::format_tag(&registries[0], &name, &version);

        if options.dry_run {
            return registry::build_image(path, &primary_tag, &self.build_args, &self.platforms);
        }

        // Build the image
        registry::build_image(path, &primary_tag, &self.build_args, &self.platforms)?;

        // Collect all tags to push
        let mut tags_to_push = vec![primary_tag.clone()];

        for reg in &registries {
            let base = registry::format_base(reg, &name);

            // Version tag
            let version_tag = format!("{}:{}", base, version);
            if !tags_to_push.contains(&version_tag) {
                registry::tag_image(&primary_tag, &version_tag)?;
                tags_to_push.push(version_tag);
            }

            // Additional tags (latest, etc.)
            for extra_tag in &self.additional_tags {
                let full_tag = format!("{}:{}", base, extra_tag);
                registry::tag_image(&primary_tag, &full_tag)?;
                tags_to_push.push(full_tag);
            }

            // Tag from options
            if let Some(ref tag) = options.tag {
                let full_tag = format!("{}:{}", base, tag);
                if !tags_to_push.contains(&full_tag) {
                    registry::tag_image(&primary_tag, &full_tag)?;
                    tags_to_push.push(full_tag);
                }
            }
        }

        // Push all tags
        for tag in &tags_to_push {
            registry::push_image(tag)?;
        }

        Ok(())
    }

    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult> {
        debug!(adapter = "docker", path = %path.display(), "validating publishable");
        let mut result = ValidationResult::pass();

        // Check Dockerfile exists
        let dockerfile = self.dockerfile_path(path);
        if !dockerfile.exists() {
            result.add_error("Dockerfile not found");
            return Ok(result);
        }

        // Read and validate Dockerfile
        let content = match std::fs::read_to_string(&dockerfile) {
            Ok(c) => c,
            Err(e) => {
                result.add_error(format!("Cannot read Dockerfile: {}", e));
                return Ok(result);
            }
        };

        // Check for FROM instruction
        if !content.lines().any(|l| l.trim().starts_with("FROM")) {
            result.add_error("Dockerfile has no FROM instruction");
        }

        // Check for version label (warning only)
        let has_version = content.contains("org.opencontainers.image.version")
            || content.contains("LABEL version=");
        if !has_version {
            result.add_warning("No version label in Dockerfile");
        }

        // Check for .dockerignore
        if !path.join(".dockerignore").exists() {
            result.add_warning("No .dockerignore file found");
        }

        // Verify Docker is available
        let docker_check = Command::new("docker")
            .args(["version", "--format", "{{.Server.Version}}"])
            .output();

        match docker_check {
            Ok(output) if output.status.success() => {}
            Ok(_) => {
                result.add_error("Docker daemon is not running");
            }
            Err(_) => {
                result.add_error("Docker is not installed");
            }
        }

        // Try a build test (syntax check)
        let build_check = Command::new("docker")
            .args(["build", "--check", "."])
            .current_dir(path)
            .output();

        if let Ok(output) = build_check {
            if !output.status.success()
                && !String::from_utf8_lossy(&output.stderr).contains("unknown flag")
            {
                result
                    .add_warning("Dockerfile may have issues (run 'docker build' to see details)");
            }
        }

        Ok(result)
    }

    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool> {
        debug!(adapter = "docker", "checking authentication");
        let config_path = dirs::home_dir().map(|h| h.join(".docker").join("config.json"));

        if let Some(path) = config_path {
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.contains("\"auths\"") && content.contains("\"auth\"") {
                        return Ok(true);
                    }
                }
            }
        }

        if credentials.has_credentials("docker") {
            return Ok(true);
        }

        Ok(false)
    }

    fn lint(&self, path: &Path) -> Result<()> {
        let dockerfile = self.dockerfile_path(path);
        if !dockerfile.exists() {
            return Ok(());
        }

        let output = Command::new("hadolint")
            .arg(&dockerfile)
            .current_dir(path)
            .output();

        match output {
            Ok(out) if !out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                let stdout = String::from_utf8_lossy(&out.stdout);
                Err(AdapterError::CommandFailed {
                    command: "hadolint".to_string(),
                    reason: format!("{}{}", stdout, stderr),
                }
                .into())
            }
            Err(_) => Ok(()),
            _ => Ok(()),
        }
    }

    fn build(&self, path: &Path) -> Result<()> {
        let (name, version) = parser::parse_image_info(path)?;
        let tag = format!("{}:{}", name, version);
        registry::build_image(path, &tag, &self.build_args, &self.platforms)
    }

    fn clean(&self, path: &Path) -> Result<()> {
        let (name, _) = parser::parse_image_info(path)?;

        let output = Command::new("docker")
            .args([
                "image",
                "prune",
                "-f",
                "--filter",
                &format!("label=name={}", name),
            ])
            .output()
            .map_err(|e| AdapterError::CommandFailed {
                command: "docker image prune".to_string(),
                reason: e.to_string(),
            })?;

        if !output.status.success() {
            tracing::warn!("Failed to prune Docker images");
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
        let adapter = DockerAdapter::new();

        let temp = TempDir::new().unwrap();
        assert!(!adapter.detect(temp.path()));

        std::fs::write(temp.path().join("Dockerfile"), "FROM alpine:latest\n").unwrap();
        assert!(adapter.detect(temp.path()));
    }

    #[test]
    fn test_get_info_from_dockerfile() {
        let adapter = DockerAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("Dockerfile"),
            r#"FROM alpine:latest
LABEL org.opencontainers.image.version="1.2.3"
LABEL org.opencontainers.image.title="myapp"
"#,
        )
        .unwrap();

        let info = adapter.get_info(temp.path()).unwrap();
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.package_type, "docker");
    }

    #[test]
    fn test_get_info_from_package_json() {
        let adapter = DockerAdapter::new();
        let temp = TempDir::new().unwrap();

        std::fs::write(temp.path().join("Dockerfile"), "FROM node:18\n").unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "@scope/myapp", "version": "2.0.0"}"#,
        )
        .unwrap();

        let info = adapter.get_info(temp.path()).unwrap();
        assert_eq!(info.version, "2.0.0");
        assert_eq!(info.name, "scope-myapp");
    }

    #[test]
    fn test_manifest_names() {
        let adapter = DockerAdapter::new();
        assert_eq!(adapter.manifest_names(), &["Dockerfile"]);
    }

    #[test]
    fn test_with_tags() {
        let adapter =
            DockerAdapter::new().with_tags(vec!["latest".to_string(), "stable".to_string()]);
        assert_eq!(adapter.additional_tags, vec!["latest", "stable"]);
    }

    #[test]
    fn test_with_registries() {
        let adapter = DockerAdapter::new().with_registries(vec![
            "gcr.io/myproject".to_string(),
            "ghcr.io/myorg".to_string(),
        ]);
        assert_eq!(adapter.registries.len(), 2);
    }
}
