//! Microsoft Store integration
//!
//! Provides upload and management capabilities via the Microsoft Partner Center API.
//!
//! ## Authentication
//!
//! Uses Azure AD app registration with Partner Center API access.
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::microsoft::MicrosoftStore;
//!
//! let store = MicrosoftStore::new(config)?;
//! store.upload(&artifact_path, &options).await?;
//! ```

use crate::error::{Result, StoreError};
use crate::traits::{StoreAdapter, TrackSupport};
use crate::types::*;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

// Re-export config from types
pub use crate::types::MicrosoftStoreConfig;

const LOGIN_URL: &str = "https://login.microsoftonline.com";
const API_BASE_URL: &str = "https://manage.devcenter.microsoft.com/v1.0/my";

/// OAuth token response
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: i64,
}

/// Token cache for thread-safe access
#[derive(Debug, Default)]
struct TokenCache {
    access_token: Option<String>,
    expires_at: Option<chrono::DateTime<Utc>>,
}

/// Microsoft Partner Center API client
pub struct MicrosoftStore {
    /// Configuration
    config: MicrosoftStoreConfig,

    /// HTTP client
    client: Client,

    /// Token cache with interior mutability
    token_cache: Arc<RwLock<TokenCache>>,
}

impl MicrosoftStore {
    /// Create a new Microsoft Store client
    pub fn new(config: MicrosoftStoreConfig) -> Result<Self> {
        Ok(Self {
            config,
            client: Client::new(),
            token_cache: Arc::new(RwLock::new(TokenCache::default())),
        })
    }

    /// Get or refresh OAuth2 access token
    async fn get_access_token(&self) -> Result<String> {
        // Check if we have a valid cached token
        {
            let cache = self.token_cache.read().await;
            if let (Some(token), Some(expires)) = (&cache.access_token, cache.expires_at) {
                if Utc::now() < expires - Duration::minutes(5) {
                    return Ok(token.clone());
                }
            }
        }

        // Request new token
        let token_url = format!("{}/{}/oauth2/token", LOGIN_URL, self.config.tenant_id);

        let response = self
            .client
            .post(&token_url)
            .form(&[
                ("grant_type", "client_credentials"),
                ("client_id", &self.config.client_id),
                ("client_secret", &self.config.client_secret),
                ("resource", "https://manage.devcenter.microsoft.com"),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::AuthenticationFailed(error_text));
        }

        let token_response: TokenResponse = response.json().await?;

        // Cache the token
        {
            let mut cache = self.token_cache.write().await;
            cache.access_token = Some(token_response.access_token.clone());
            cache.expires_at = Some(Utc::now() + Duration::seconds(token_response.expires_in));
        }

        Ok(token_response.access_token)
    }

    /// Make an authenticated API request
    async fn api_request<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let token = self.get_access_token().await?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        let mut request = self
            .client
            .request(method.clone(), &url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

        debug!("Making {} request to {}", method, url);

        let response = request.send().await?;
        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: error_text,
            });
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Create a new submission for the app
    async fn create_submission(&self) -> Result<SubmissionInfo> {
        let endpoint = format!("/applications/{}/submissions", self.config.app_id);

        let response: SubmissionResponse = self
            .api_request(reqwest::Method::POST, &endpoint, None)
            .await?;

        Ok(SubmissionInfo {
            id: response.id,
            file_upload_url: response.file_upload_url,
            status: response.status,
        })
    }

    /// Create a new flight submission
    async fn create_flight_submission(&self, flight_id: &str) -> Result<SubmissionInfo> {
        let endpoint = format!(
            "/applications/{}/flights/{}/submissions",
            self.config.app_id, flight_id
        );

        let response: SubmissionResponse = self
            .api_request(reqwest::Method::POST, &endpoint, None)
            .await?;

        Ok(SubmissionInfo {
            id: response.id,
            file_upload_url: response.file_upload_url,
            status: response.status,
        })
    }

    /// Upload a package to Azure Blob Storage
    async fn upload_package(&self, upload_url: &str, path: &Path) -> Result<()> {
        info!("Uploading package to Azure Blob Storage...");

        let file_content = tokio::fs::read(path).await?;

        // Azure Blob Storage requires specific headers for block blob upload
        let response = self
            .client
            .put(upload_url)
            .header("x-ms-blob-type", "BlockBlob")
            .header("Content-Type", "application/octet-stream")
            .body(file_content)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::UploadFailed(format!(
                "Azure Blob upload failed: {}",
                error_text
            )));
        }

        info!("Package uploaded successfully");
        Ok(())
    }

    /// Update submission with package info
    async fn update_submission(
        &self,
        submission_id: &str,
        package_path: &Path,
        release_notes: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/submissions/{}",
            self.config.app_id, submission_id
        );

        // Get current submission data
        let current: serde_json::Value = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        // Build updated submission
        let filename = package_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("package.msix");

        let mut updated = current.clone();

        // Update application packages
        if let Some(packages) = updated.get_mut("applicationPackages") {
            if let Some(arr) = packages.as_array_mut() {
                // Mark existing packages for deletion and add new one
                for pkg in arr.iter_mut() {
                    if let Some(obj) = pkg.as_object_mut() {
                        obj.insert("fileStatus".to_string(), serde_json::json!("PendingDelete"));
                    }
                }
                arr.push(serde_json::json!({
                    "fileName": filename,
                    "fileStatus": "PendingUpload",
                    "minimumDirectXVersion": "None",
                    "minimumSystemRam": "None"
                }));
            }
        }

        // Update release notes/listing
        if !release_notes.is_empty() {
            if let Some(listings) = updated.get_mut("listings") {
                for (lang, notes) in release_notes {
                    if let Some(listing) = listings.get_mut(lang) {
                        if let Some(base_listing) = listing.get_mut("baseListing") {
                            base_listing["releaseNotes"] = serde_json::json!(notes);
                        }
                    }
                }
            }
        }

        let _: serde_json::Value = self
            .api_request(reqwest::Method::PUT, &endpoint, Some(updated))
            .await?;

        Ok(())
    }

    /// Commit a submission
    async fn commit_submission(&self, submission_id: &str) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/submissions/{}/commit",
            self.config.app_id, submission_id
        );

        let _: serde_json::Value = self
            .api_request(reqwest::Method::POST, &endpoint, None)
            .await?;

        Ok(())
    }

    /// Get submission status
    async fn get_submission_status(&self, submission_id: &str) -> Result<SubmissionStatus> {
        let endpoint = format!(
            "/applications/{}/submissions/{}/status",
            self.config.app_id, submission_id
        );

        let response: StatusResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        Ok(SubmissionStatus {
            status: response.status,
            status_details: response.status_details,
            certification_reports: response.certification_reports,
        })
    }

    /// List package flights
    async fn list_flights(&self) -> Result<Vec<FlightInfo>> {
        let endpoint = format!("/applications/{}/listflights", self.config.app_id);

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FlightsResponse {
            value: Vec<FlightData>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct FlightData {
            flight_id: String,
            friendly_name: String,
        }

        let response: FlightsResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        Ok(response
            .value
            .into_iter()
            .map(|f| FlightInfo {
                id: f.flight_id,
                name: f.friendly_name,
            })
            .collect())
    }

    /// Extract app info from MSIX/APPX package
    async fn extract_package_info(path: &Path) -> Result<AppInfo> {
        // For MSIX/APPX, we can use makeappx to extract the manifest
        // or read the zip file directly since MSIX is a zip format

        let file = std::fs::File::open(path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| StoreError::InvalidArtifact(format!("Invalid MSIX/APPX: {}", e)))?;

        // Look for AppxManifest.xml
        let manifest_index = (0..archive.len()).find(|&i| {
            archive
                .by_index(i)
                .map(|f| f.name().eq_ignore_ascii_case("AppxManifest.xml"))
                .unwrap_or(false)
        });

        if let Some(index) = manifest_index {
            let mut manifest_file = archive.by_index(index).map_err(|e| {
                StoreError::InvalidArtifact(format!("Failed to read manifest: {}", e))
            })?;

            let mut contents = String::new();
            std::io::Read::read_to_string(&mut manifest_file, &mut contents)?;

            // Parse basic info from XML (simplified parsing)
            let identity_name = extract_xml_attr(&contents, "Identity", "Name").unwrap_or_default();
            let version = extract_xml_attr(&contents, "Identity", "Version")
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let display_name = extract_xml_value(&contents, "DisplayName");

            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            return Ok(AppInfo {
                identifier: identity_name,
                version: version.clone(),
                build_number: version,
                name: display_name,
                min_os_version: extract_xml_attr(&contents, "TargetDeviceFamily", "MinVersion"),
                platforms: vec!["Windows".to_string()],
                size,
                sha256: None,
            });
        }

        // Fallback
        let filename = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown");
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Ok(AppInfo {
            identifier: filename.to_string(),
            version: "0.0.0.0".to_string(),
            build_number: "0".to_string(),
            name: Some(filename.to_string()),
            min_os_version: None,
            platforms: vec!["Windows".to_string()],
            size,
            sha256: None,
        })
    }
}

/// Submission info returned when creating a submission
#[derive(Debug)]
struct SubmissionInfo {
    id: String,
    file_upload_url: String,
    #[allow(dead_code)]
    status: String,
}

/// Submission response from API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubmissionResponse {
    id: String,
    file_upload_url: String,
    status: String,
}

/// Status response from API
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StatusResponse {
    status: String,
    status_details: Option<StatusDetails>,
    certification_reports: Option<Vec<CertificationReport>>,
}

/// Status details
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StatusDetails {
    pub errors: Option<Vec<SubmissionError>>,
    pub warnings: Option<Vec<SubmissionWarning>>,
}

/// Submission error
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubmissionError {
    pub code: String,
    pub details: String,
}

/// Submission warning
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SubmissionWarning {
    pub code: String,
    pub details: String,
}

/// Certification report
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CertificationReport {
    pub date: String,
    pub report_url: Option<String>,
}

/// Submission status info
#[derive(Debug)]
pub struct SubmissionStatus {
    pub status: String,
    pub status_details: Option<StatusDetails>,
    pub certification_reports: Option<Vec<CertificationReport>>,
}

/// Flight info
#[derive(Debug)]
pub struct FlightInfo {
    pub id: String,
    pub name: String,
}

/// Helper to extract XML attribute value (simplified)
fn extract_xml_attr(xml: &str, element: &str, attr: &str) -> Option<String> {
    let pattern = format!(r#"<{}\s+[^>]*{}="([^"]+)""#, element, attr);
    regex::Regex::new(&pattern)
        .ok()
        .and_then(|re| re.captures(xml))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Helper to extract XML element value (simplified)
fn extract_xml_value(xml: &str, element: &str) -> Option<String> {
    let pattern = format!(r#"<{}[^>]*>([^<]+)</{}"#, element, element);
    regex::Regex::new(&pattern)
        .ok()
        .and_then(|re| re.captures(xml))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

#[async_trait::async_trait]
impl StoreAdapter for MicrosoftStore {
    fn name(&self) -> &str {
        "Microsoft Store"
    }

    fn store_type(&self) -> StoreType {
        StoreType::Microsoft
    }

    fn is_available(&self) -> bool {
        !self.config.client_id.is_empty()
            && !self.config.client_secret.is_empty()
            && !self.config.tenant_id.is_empty()
    }

    #[instrument(skip(self), fields(store = "Microsoft Store", path = %path.display()))]
    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult> {
        let app_info = Self::extract_package_info(path).await?;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Check file extension
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(
            ext.as_str(),
            "msix" | "msixbundle" | "appx" | "appxbundle" | "msixupload" | "appxupload"
        ) {
            errors.push(ValidationError {
                code: "INVALID_FORMAT".to_string(),
                message: format!("Unsupported file format: .{}", ext),
                severity: ValidationSeverity::Error,
            });
        }

        if app_info.identifier.is_empty() {
            errors.push(ValidationError {
                code: "MISSING_IDENTITY".to_string(),
                message: "Package identity name is missing".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if app_info.version == "0.0.0.0" {
            warnings.push("Package version appears to be default".to_string());
        }

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

    #[instrument(skip(self, options), fields(store = "Microsoft Store", path = %path.display()))]
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

        // Create submission
        info!("Creating submission...");
        let submission = if let Some(flight) = options
            .track
            .as_ref()
            .or(self.config.default_flight.as_ref())
        {
            // Find flight ID by name
            let flights = self.list_flights().await?;
            let flight_info = flights
                .iter()
                .find(|f| f.name.eq_ignore_ascii_case(flight))
                .ok_or_else(|| StoreError::AppNotFound(format!("Flight '{}' not found", flight)))?;
            self.create_flight_submission(&flight_info.id).await?
        } else {
            self.create_submission().await?
        };

        // Update submission with package info
        info!("Updating submission...");
        self.update_submission(&submission.id, path, &options.release_notes)
            .await?;

        // Upload package to Azure Blob Storage
        info!("Uploading package...");
        self.upload_package(&submission.file_upload_url, path)
            .await?;

        // Commit submission
        info!("Committing submission...");
        self.commit_submission(&submission.id).await?;

        let console_url = format!(
            "https://partner.microsoft.com/dashboard/products/{}/submissions/{}",
            self.config.app_id, submission.id
        );

        Ok(UploadResult {
            success: true,
            build_id: Some(submission.id),
            console_url: Some(console_url),
            status: UploadStatus::Processing,
            warnings: validation.warnings,
            uploaded_at: Utc::now(),
        })
    }

    #[instrument(skip(self), fields(store = "Microsoft Store"))]
    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus> {
        let status = self.get_submission_status(build_id).await?;

        let upload_status = match status.status.as_str() {
            "CommitStarted" | "CommitFailed" => UploadStatus::Processing,
            "PreProcessing" | "PreProcessingFailed" => UploadStatus::Processing,
            "Certification" | "CertificationFailed" => UploadStatus::InReview,
            "Release" | "ReleaseFailed" => UploadStatus::Ready,
            "Publishing" => UploadStatus::Processing,
            "Published" => UploadStatus::Live,
            "Canceled" | "Failed" => UploadStatus::Failed,
            _ => UploadStatus::Processing,
        };

        let details = status.status_details.map(|d| {
            let mut details = String::new();
            if let Some(errors) = d.errors {
                for err in errors {
                    details.push_str(&format!("Error {}: {}\n", err.code, err.details));
                }
            }
            if let Some(warnings) = d.warnings {
                for warn in warnings {
                    details.push_str(&format!("Warning {}: {}\n", warn.code, warn.details));
                }
            }
            details
        });

        Ok(BuildStatus {
            build_id: build_id.to_string(),
            version: "".to_string(),
            build_number: "".to_string(),
            status: upload_status,
            uploaded_at: None,
            processed_at: None,
            expires_at: None,
            track: None,
            rollout_percentage: None,
            details,
        })
    }

    async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
        // Would need to query submission history
        Ok(Vec::new())
    }

    fn supported_extensions(&self) -> &[&str] {
        &[
            "msix",
            "msixbundle",
            "appx",
            "appxbundle",
            "msixupload",
            "appxupload",
        ]
    }
}

#[async_trait::async_trait]
impl TrackSupport for MicrosoftStore {
    async fn list_tracks(&self) -> Result<Vec<String>> {
        let flights = self.list_flights().await?;
        let mut tracks: Vec<String> = flights.into_iter().map(|f| f.name).collect();
        tracks.insert(0, "production".to_string()); // Default/main submission
        Ok(tracks)
    }

    async fn promote_build(
        &self,
        _build_id: &str,
        _from_track: &str,
        _to_track: &str,
    ) -> Result<()> {
        // Microsoft Store doesn't support direct promotion between flights
        // Would need to create a new submission on the target track
        Err(StoreError::Other(
            "Microsoft Store doesn't support direct promotion. Create a new submission instead."
                .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_xml_attr() {
        let xml = r#"<Identity Name="MyApp" Version="1.0.0.0" Publisher="CN=Test"/>"#;
        assert_eq!(
            extract_xml_attr(xml, "Identity", "Name"),
            Some("MyApp".to_string())
        );
        assert_eq!(
            extract_xml_attr(xml, "Identity", "Version"),
            Some("1.0.0.0".to_string())
        );
    }

    #[test]
    fn test_extract_xml_value() {
        let xml = r#"<DisplayName>My Application</DisplayName>"#;
        assert_eq!(
            extract_xml_value(xml, "DisplayName"),
            Some("My Application".to_string())
        );
    }

    #[test]
    fn test_supported_extensions() {
        let extensions = &[
            "msix",
            "msixbundle",
            "appx",
            "appxbundle",
            "msixupload",
            "appxupload",
        ];
        assert!(extensions.contains(&"msix"));
        assert!(extensions.contains(&"appxbundle"));
    }
}
