//! NPM Registry integration
//!
//! Provides upload and tag management capabilities for the NPM package registry.
//!
//! ## Authentication
//!
//! Authentication is handled via NPM token, checked in this order:
//! 1. NPM_TOKEN environment variable
//! 2. Token from ~/.npmrc file
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::registries::npm::NpmRegistry;
//!
//! let registry = NpmRegistry::new(config)?;
//! registry.upload(&artifact_path, &options).await?;
//! ```

use crate::error::{Result, StoreError};
use crate::traits::StoreAdapter;
use crate::types::*;
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use tar::Archive;
use tracing::{debug, info, instrument};

/// NPM Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NpmConfig {
    /// Registry URL (default: https://registry.npmjs.org)
    pub registry_url: String,
    /// NPM authentication token
    pub token: Option<String>,
}

impl Default for NpmConfig {
    fn default() -> Self {
        Self {
            registry_url: "https://registry.npmjs.org".to_string(),
            token: None,
        }
    }
}

impl NpmConfig {
    /// Create new NPM config with default registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Set registry URL
    pub fn with_registry_url(mut self, url: String) -> Self {
        self.registry_url = url;
        self
    }

    /// Set authentication token
    pub fn with_token(mut self, token: String) -> Self {
        self.token = Some(token);
        self
    }
}

/// NPM Registry adapter
pub struct NpmRegistry {
    config: NpmConfig,
    client: Client,
}

/// Trait for npm dist-tags support
#[async_trait]
pub trait TagSupport {
    /// Add a dist-tag to a package version
    async fn add_tag(&self, package: &str, version: &str, tag: &str) -> Result<()>;

    /// Remove a dist-tag from a package
    async fn remove_tag(&self, package: &str, tag: &str) -> Result<()>;

    /// List all dist-tags for a package
    async fn list_tags(&self, package: &str) -> Result<Vec<(String, String)>>;
}

/// Package.json structure (minimal required fields)
#[derive(Debug, Deserialize, Serialize)]
struct PackageJson {
    name: String,
    version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    main: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    engines: Option<HashMap<String, String>>,
}

impl NpmRegistry {
    /// Create a new NPM Registry client
    pub fn new(mut config: NpmConfig) -> Result<Self> {
        // Try to load token if not provided
        if config.token.is_none() {
            config.token = Self::load_token()?;
        }

        Ok(Self {
            config,
            client: Client::new(),
        })
    }

    /// Load NPM token from environment or .npmrc
    fn load_token() -> Result<Option<String>> {
        // First try NPM_TOKEN environment variable
        if let Ok(token) = std::env::var("NPM_TOKEN") {
            if !token.is_empty() {
                debug!("Loaded NPM token from NPM_TOKEN environment variable");
                return Ok(Some(token));
            }
        }

        // Try reading from ~/.npmrc
        if let Some(home_dir) = dirs::home_dir() {
            let npmrc_path = home_dir.join(".npmrc");
            if npmrc_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&npmrc_path) {
                    for line in content.lines() {
                        let line = line.trim();
                        // Look for registry.npmjs.org/:_authToken=...
                        if line.starts_with("//registry.npmjs.org/:_authToken=") {
                            let token = line
                                .trim_start_matches("//registry.npmjs.org/:_authToken=")
                                .trim();
                            if !token.is_empty() {
                                debug!("Loaded NPM token from ~/.npmrc");
                                return Ok(Some(token.to_string()));
                            }
                        }
                    }
                }
            }
        }

        debug!("No NPM token found in environment or ~/.npmrc");
        Ok(None)
    }

    /// Extract and validate tarball, parse package.json
    async fn extract_package_info(path: &Path) -> Result<(PackageJson, Vec<u8>)> {
        // Read the entire tarball
        let tarball_data = tokio::fs::read(path).await?;

        // Decompress gzip
        let decoder = GzDecoder::new(&tarball_data[..]);
        let mut archive = Archive::new(decoder);

        let mut package_json_content = None;

        // Extract and find package.json
        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?;

            // npm tarballs have package/ prefix
            if entry_path.to_string_lossy().ends_with("package.json") {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                package_json_content = Some(content);
                break;
            }
        }

        let package_json_str = package_json_content.ok_or_else(|| {
            StoreError::InvalidArtifact("No package.json found in tarball".to_string())
        })?;

        let package_json: PackageJson = serde_json::from_str(&package_json_str)
            .map_err(|e| StoreError::InvalidArtifact(format!("Invalid package.json: {}", e)))?;

        Ok((package_json, tarball_data))
    }

    /// Upload package to npm registry
    async fn publish_package(
        &self,
        package_name: &str,
        package_json: &PackageJson,
        tarball_data: &[u8],
    ) -> Result<()> {
        let token = self.config.token.as_ref().ok_or_else(|| {
            StoreError::AuthenticationFailed("No NPM token configured".to_string())
        })?;

        // Encode tarball as base64
        let tarball_base64 = BASE64.encode(tarball_data);

        // Create attachment
        let filename = format!("{}-{}.tgz", package_name, package_json.version);

        // Build publish payload
        let mut attachments = HashMap::new();
        attachments.insert(
            filename.clone(),
            serde_json::json!({
                "content_type": "application/octet-stream",
                "data": tarball_base64,
                "length": tarball_data.len()
            }),
        );

        let payload = serde_json::json!({
            "name": package_json.name,
            "description": package_json.description,
            "versions": {
                &package_json.version: package_json
            },
            "_attachments": attachments,
            "dist-tags": {
                "latest": package_json.version
            }
        });

        // PUT to /{package}
        let url = format!("{}/{}", self.config.registry_url, package_name);

        debug!("Publishing package to {}", url);

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        Ok(())
    }
}

#[async_trait]
impl StoreAdapter for NpmRegistry {
    fn name(&self) -> &str {
        "NPM"
    }

    fn store_type(&self) -> StoreType {
        StoreType::Npm
    }

    fn is_available(&self) -> bool {
        self.config.token.is_some()
    }

    #[instrument(skip(self), fields(store = "NPM", path = %path.display()))]
    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let warnings = Vec::new();

        // Check file extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if ext != "tgz" && !path.to_string_lossy().ends_with(".tar.gz") {
            errors.push(ValidationError {
                code: "INVALID_EXTENSION".to_string(),
                message: format!("Invalid file extension '{}'. Expected .tgz or .tar.gz", ext),
                severity: ValidationSeverity::Error,
            });
            return Ok(ValidationResult::failure(errors));
        }

        // Extract and validate package.json
        let (package_json, _) = match Self::extract_package_info(path).await {
            Ok(result) => result,
            Err(e) => {
                errors.push(ValidationError {
                    code: "EXTRACTION_FAILED".to_string(),
                    message: format!("Failed to extract package: {}", e),
                    severity: ValidationSeverity::Error,
                });
                return Ok(ValidationResult::failure(errors));
            }
        };

        // Validate package name
        if package_json.name.is_empty() {
            errors.push(ValidationError {
                code: "MISSING_NAME".to_string(),
                message: "Package name is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        // Validate version
        if package_json.version.is_empty() {
            errors.push(ValidationError {
                code: "MISSING_VERSION".to_string(),
                message: "Package version is required".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        // Create AppInfo
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        let app_info = AppInfo {
            identifier: package_json.name.clone(),
            version: package_json.version.clone(),
            build_number: package_json.version.clone(), // npm doesn't have separate build numbers
            name: package_json.description.clone(),
            min_os_version: package_json
                .engines
                .as_ref()
                .and_then(|e| e.get("node"))
                .cloned(),
            platforms: vec!["Node.js".to_string()],
            size,
            sha256: None,
        };

        if errors.is_empty() {
            Ok(ValidationResult {
                valid: true,
                errors: Vec::new(),
                warnings,
                app_info: Some(app_info),
            })
        } else {
            Ok(ValidationResult {
                valid: false,
                errors,
                warnings,
                app_info: Some(app_info),
            })
        }
    }

    #[instrument(skip(self, options), fields(store = "NPM", path = %path.display()))]
    async fn upload(&self, path: &Path, options: &UploadOptions) -> Result<UploadResult> {
        // Validate first
        let validation = self.validate_artifact(path).await?;
        if !validation.valid {
            return Err(StoreError::ValidationFailed(
                validation
                    .errors
                    .iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join("; "),
            ));
        }

        if options.dry_run {
            info!("Dry run - would publish package from {}", path.display());
            return Ok(UploadResult {
                success: true,
                build_id: None,
                console_url: None,
                status: UploadStatus::Ready,
                warnings: validation.warnings,
                uploaded_at: Utc::now(),
            });
        }

        // Extract package info
        info!("Extracting package information from {}...", path.display());
        let (package_json, tarball_data) = Self::extract_package_info(path).await?;

        // Publish to npm
        info!(
            "Publishing {} version {} to registry...",
            package_json.name, package_json.version
        );
        self.publish_package(&package_json.name, &package_json, &tarball_data)
            .await?;

        let console_url = format!("https://www.npmjs.com/package/{}", package_json.name);

        Ok(UploadResult {
            success: true,
            build_id: Some(package_json.version.clone()),
            console_url: Some(console_url),
            status: UploadStatus::Ready,
            warnings: validation.warnings,
            uploaded_at: Utc::now(),
        })
    }

    #[instrument(skip(self), fields(store = "NPM"))]
    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus> {
        // npm doesn't have build status concept - packages are immediately available
        Ok(BuildStatus {
            build_id: build_id.to_string(),
            version: build_id.to_string(),
            build_number: build_id.to_string(),
            status: UploadStatus::Ready,
            uploaded_at: Some(Utc::now()),
            processed_at: Some(Utc::now()),
            expires_at: None,
            track: Some("latest".to_string()),
            rollout_percentage: None,
            details: Some("NPM packages are immediately available after publishing".to_string()),
        })
    }

    async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
        // npm doesn't have a concept of builds/submissions like app stores
        // Return empty list
        Ok(Vec::new())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["tgz", "tar.gz"]
    }
}

#[async_trait]
impl TagSupport for NpmRegistry {
    async fn add_tag(&self, package: &str, version: &str, tag: &str) -> Result<()> {
        let token = self.config.token.as_ref().ok_or_else(|| {
            StoreError::AuthenticationFailed("No NPM token configured".to_string())
        })?;

        // PUT /-/package/{package}/dist-tags/{tag}
        let url = format!(
            "{}/-/package/{}/dist-tags/{}",
            self.config.registry_url, package, tag
        );

        debug!("Adding tag '{}' to {}@{}", tag, package, version);

        let response = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&serde_json::json!(version))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        info!(
            "Successfully added tag '{}' to {}@{}",
            tag, package, version
        );
        Ok(())
    }

    async fn remove_tag(&self, package: &str, tag: &str) -> Result<()> {
        let token = self.config.token.as_ref().ok_or_else(|| {
            StoreError::AuthenticationFailed("No NPM token configured".to_string())
        })?;

        // DELETE /-/package/{package}/dist-tags/{tag}
        let url = format!(
            "{}/-/package/{}/dist-tags/{}",
            self.config.registry_url, package, tag
        );

        debug!("Removing tag '{}' from {}", tag, package);

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        info!("Successfully removed tag '{}' from {}", tag, package);
        Ok(())
    }

    async fn list_tags(&self, package: &str) -> Result<Vec<(String, String)>> {
        // GET /{package}
        let url = format!("{}/{}", self.config.registry_url, package);

        debug!("Fetching tags for package {}", package);

        let response = self.client.get(&url).send().await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        #[derive(Deserialize)]
        struct PackageInfo {
            #[serde(rename = "dist-tags")]
            dist_tags: HashMap<String, String>,
        }

        let package_info: PackageInfo = response.json().await?;

        let tags: Vec<(String, String)> = package_info.dist_tags.into_iter().collect();

        Ok(tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_npm_config_default() {
        let config = NpmConfig::default();
        assert_eq!(config.registry_url, "https://registry.npmjs.org");
        assert!(config.token.is_none());
    }

    #[test]
    fn test_npm_config_builder() {
        let config = NpmConfig::new()
            .with_registry_url("https://custom-registry.example.com".to_string())
            .with_token("test-token".to_string());

        assert_eq!(config.registry_url, "https://custom-registry.example.com");
        assert_eq!(config.token, Some("test-token".to_string()));
    }
}
