//! Crates.io registry integration
//!
//! Provides upload capabilities to the Rust package registry.
//!
//! ## Authentication
//!
//! Uses CARGO_REGISTRY_TOKEN environment variable or reads from ~/.cargo/credentials.toml.
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::registries::crates_io::CratesIoRegistry;
//!
//! let registry = CratesIoRegistry::new(config)?;
//! registry.upload(&crate_path, &options).await?;
//! ```

use crate::error::{Result, StoreError};
use crate::traits::StoreAdapter;
use crate::types::*;
use chrono::Utc;
use flate2::read::GzDecoder;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;
use tar::Archive;
use tracing::{debug, info};

const DEFAULT_REGISTRY_URL: &str = "https://crates.io";

/// Crates.io registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CratesIoConfig {
    /// Registry URL (default: "https://crates.io")
    pub registry_url: String,

    /// API token from CARGO_REGISTRY_TOKEN or ~/.cargo/credentials.toml
    pub token: Option<String>,
}

impl Default for CratesIoConfig {
    fn default() -> Self {
        Self {
            registry_url: DEFAULT_REGISTRY_URL.to_string(),
            token: None,
        }
    }
}

/// Crates.io registry adapter
pub struct CratesIoRegistry {
    config: CratesIoConfig,
    client: Client,
}

impl CratesIoRegistry {
    /// Create a new Crates.io registry adapter
    pub fn new(mut config: CratesIoConfig) -> Result<Self> {
        // Try to load token if not provided
        if config.token.is_none() {
            config.token = Self::load_token()?;
        }

        Ok(Self {
            config,
            client: Client::new(),
        })
    }

    /// Load token from environment or credentials file
    fn load_token() -> Result<Option<String>> {
        // First try environment variable
        if let Ok(token) = std::env::var("CARGO_REGISTRY_TOKEN") {
            if !token.is_empty() {
                debug!("Loaded token from CARGO_REGISTRY_TOKEN");
                return Ok(Some(token));
            }
        }

        // Fall back to ~/.cargo/credentials.toml
        if let Some(home_dir) = dirs::home_dir() {
            let credentials_path = home_dir.join(".cargo").join("credentials.toml");

            if credentials_path.exists() {
                let content = std::fs::read_to_string(&credentials_path)
                    .map_err(|e| StoreError::ConfigurationError(format!(
                        "Failed to read credentials file: {}", e
                    )))?;

                // Parse TOML
                let credentials: toml::Value = toml::from_str(&content)
                    .map_err(|e| StoreError::ConfigurationError(format!(
                        "Failed to parse credentials.toml: {}", e
                    )))?;

                // Extract token from [registry] section
                if let Some(registry) = credentials.get("registry") {
                    if let Some(token) = registry.get("token") {
                        if let Some(token_str) = token.as_str() {
                            debug!("Loaded token from ~/.cargo/credentials.toml");
                            return Ok(Some(token_str.to_string()));
                        }
                    }
                }
            }
        }

        Ok(None)
    }

    /// Extract and parse Cargo.toml from a .crate file
    async fn extract_crate_info(path: &Path) -> Result<AppInfo> {
        // .crate files are gzipped tar archives
        let file = std::fs::File::open(path)
            .map_err(|e| StoreError::InvalidArtifact(format!(
                "Failed to open .crate file: {}", e
            )))?;

        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let mut cargo_toml_content = String::new();
        let mut found_cargo_toml = false;

        // Search for Cargo.toml in the archive
        for entry in archive.entries()
            .map_err(|e| StoreError::InvalidArtifact(format!(
                "Failed to read .crate archive: {}", e
            )))?
        {
            let mut entry = entry.map_err(|e| StoreError::InvalidArtifact(format!(
                "Failed to read archive entry: {}", e
            )))?;

            let path = entry.path()
                .map_err(|e| StoreError::InvalidArtifact(format!(
                    "Failed to get entry path: {}", e
                )))?;

            // Cargo.toml is typically in the format: package-name-version/Cargo.toml
            if path.file_name() == Some(std::ffi::OsStr::new("Cargo.toml")) {
                entry.read_to_string(&mut cargo_toml_content)
                    .map_err(|e| StoreError::InvalidArtifact(format!(
                        "Failed to read Cargo.toml: {}", e
                    )))?;
                found_cargo_toml = true;
                break;
            }
        }

        if !found_cargo_toml {
            return Err(StoreError::InvalidArtifact(
                "Cargo.toml not found in .crate archive".to_string()
            ));
        }

        // Parse Cargo.toml
        let cargo_toml: toml::Value = toml::from_str(&cargo_toml_content)
            .map_err(|e| StoreError::InvalidArtifact(format!(
                "Failed to parse Cargo.toml: {}", e
            )))?;

        // Extract package information
        let package = cargo_toml.get("package")
            .ok_or_else(|| StoreError::InvalidArtifact(
                "Cargo.toml missing [package] section".to_string()
            ))?;

        let name = package.get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::InvalidArtifact(
                "Cargo.toml missing package.name".to_string()
            ))?
            .to_string();

        let version = package.get("version")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::InvalidArtifact(
                "Cargo.toml missing package.version".to_string()
            ))?
            .to_string();

        let description = package.get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        Ok(AppInfo {
            identifier: name.clone(),
            version: version.clone(),
            build_number: version, // For crates, version and build number are the same
            name: description,
            min_os_version: None, // Rust version could go here but not standardized
            platforms: vec!["Rust".to_string()],
            size,
            sha256: None,
        })
    }
}

#[async_trait::async_trait]
impl StoreAdapter for CratesIoRegistry {
    fn name(&self) -> &str {
        "Crates.io"
    }

    fn store_type(&self) -> StoreType {
        StoreType::Crates
    }

    fn is_available(&self) -> bool {
        self.config.token.is_some()
    }

    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult> {
        // Check file extension
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        if ext != "crate" {
            return Ok(ValidationResult::failure(vec![ValidationError {
                code: "INVALID_EXTENSION".to_string(),
                message: format!("Expected .crate file, got .{}", ext),
                severity: ValidationSeverity::Error,
            }]));
        }

        // Extract and validate .crate file
        let app_info = Self::extract_crate_info(path).await?;

        let mut warnings = Vec::new();

        if app_info.name.is_none() {
            warnings.push("Package description is missing".to_string());
        }

        Ok(ValidationResult {
            valid: true,
            errors: Vec::new(),
            warnings,
            app_info: Some(app_info),
        })
    }

    async fn upload(&self, path: &Path, options: &UploadOptions) -> Result<UploadResult> {
        // Validate first
        let validation = self.validate_artifact(path).await?;
        if !validation.valid {
            return Err(StoreError::ValidationFailed(
                validation.errors.iter()
                    .map(|e| e.message.clone())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }

        if options.dry_run {
            info!("Dry run - would upload {}", path.display());
            return Ok(UploadResult {
                success: true,
                build_id: None,
                console_url: None,
                status: UploadStatus::Processing,
                warnings: validation.warnings,
                uploaded_at: Utc::now(),
            });
        }

        let token = self.config.token.as_ref()
            .ok_or_else(|| StoreError::AuthenticationFailed(
                "No API token configured".to_string()
            ))?;

        // Read .crate file
        let file_content = tokio::fs::read(path).await
            .map_err(|e| StoreError::UploadFailed(format!(
                "Failed to read .crate file: {}", e
            )))?;

        // Prepare multipart form
        let form = reqwest::multipart::Form::new()
            .part(
                "crate",
                reqwest::multipart::Part::bytes(file_content)
                    .file_name(path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("package.crate")
                        .to_string()
                    )
            );

        // Upload to crates.io
        let url = format!("{}/api/v1/crates/new", self.config.registry_url);

        info!("Uploading to {}...", url);

        let response = self.client
            .put(&url)
            .header("Authorization", token)
            .multipart(form)
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

        // Parse response
        #[derive(serde::Deserialize)]
        struct PublishResponse {
            warnings: Option<serde_json::Value>,
        }

        let publish_response: PublishResponse = response.json().await
            .map_err(|e| StoreError::UploadFailed(format!(
                "Failed to parse response: {}", e
            )))?;

        let mut warnings = validation.warnings;
        if let Some(api_warnings) = publish_response.warnings {
            if let Some(warning_str) = api_warnings.as_str() {
                warnings.push(warning_str.to_string());
            }
        }

        let app_info = validation.app_info.as_ref().unwrap();
        let console_url = format!(
            "{}/crates/{}",
            self.config.registry_url,
            app_info.identifier
        );

        Ok(UploadResult {
            success: true,
            build_id: Some(format!("{}-{}", app_info.identifier, app_info.version)),
            console_url: Some(console_url),
            status: UploadStatus::Live, // Crates.io publishes immediately
            warnings,
            uploaded_at: Utc::now(),
        })
    }

    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus> {
        // Crates.io doesn't have a build status concept
        // Crates are published immediately
        Ok(BuildStatus {
            build_id: build_id.to_string(),
            version: "".to_string(),
            build_number: "".to_string(),
            status: UploadStatus::Live,
            uploaded_at: None,
            processed_at: None,
            expires_at: None,
            track: None,
            rollout_percentage: None,
            details: Some("Crates.io publishes immediately upon successful upload".to_string()),
        })
    }

    async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
        // Crates.io doesn't provide a build list API in the context we need
        // Would need to scrape the website or use the crates.io API differently
        Ok(Vec::new())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["crate"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CratesIoConfig::default();
        assert_eq!(config.registry_url, DEFAULT_REGISTRY_URL);
        assert!(config.token.is_none());
    }

    #[test]
    fn test_supported_extensions() {
        let config = CratesIoConfig::default();
        let registry = CratesIoRegistry::new(config).unwrap();
        assert_eq!(registry.supported_extensions(), &["crate"]);
    }

    #[test]
    fn test_is_available_without_token() {
        let config = CratesIoConfig::default();
        let registry = CratesIoRegistry::new(config).unwrap();
        assert!(!registry.is_available());
    }

    #[test]
    fn test_is_available_with_token() {
        let config = CratesIoConfig {
            registry_url: DEFAULT_REGISTRY_URL.to_string(),
            token: Some("test-token".to_string()),
        };
        let registry = CratesIoRegistry::new(config).unwrap();
        assert!(registry.is_available());
    }
}
