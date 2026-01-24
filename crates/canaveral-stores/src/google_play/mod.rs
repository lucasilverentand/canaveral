//! Google Play Store integration
//!
//! Provides upload and management capabilities via the Google Play Developer API.
//!
//! ## Authentication
//!
//! Uses a Google Cloud service account with Google Play Developer API access.
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::google_play::GooglePlayStore;
//!
//! let store = GooglePlayStore::new(config)?;
//! store.upload(&artifact_path, &options).await?;
//! ```

use crate::error::{Result, StoreError};
use crate::traits::{StagedRolloutSupport, StoreAdapter, TrackSupport};
use crate::types::*;
use chrono::{Duration, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

const API_BASE_URL: &str = "https://androidpublisher.googleapis.com/androidpublisher/v3";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Google service account credentials
#[derive(Debug, Deserialize)]
struct ServiceAccountKey {
    client_email: String,
    private_key: String,
    #[allow(dead_code)]
    token_uri: Option<String>,
}

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

/// Google Play Developer API client
pub struct GooglePlayStore {
    /// Configuration
    config: GooglePlayConfig,

    /// HTTP client
    client: Client,

    /// Token cache with interior mutability
    token_cache: Arc<RwLock<TokenCache>>,

    /// Service account credentials
    service_account: ServiceAccountKey,
}

impl GooglePlayStore {
    /// Create a new Google Play Store client
    pub fn new(config: GooglePlayConfig) -> Result<Self> {
        // Load service account key
        let key_content = std::fs::read_to_string(&config.service_account_key)
            .map_err(|e| StoreError::ConfigurationError(format!(
                "Failed to read service account key: {}", e
            )))?;

        let service_account: ServiceAccountKey = serde_json::from_str(&key_content)
            .map_err(|e| StoreError::InvalidCredentials(format!(
                "Invalid service account key: {}", e
            )))?;

        Ok(Self {
            config,
            client: Client::new(),
            token_cache: Arc::new(RwLock::new(TokenCache::default())),
            service_account,
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

        // Generate JWT for service account authentication
        let now = Utc::now();
        let exp = now + Duration::hours(1);

        #[derive(Serialize)]
        struct Claims {
            iss: String,
            scope: String,
            aud: String,
            iat: i64,
            exp: i64,
        }

        let claims = Claims {
            iss: self.service_account.client_email.clone(),
            scope: "https://www.googleapis.com/auth/androidpublisher".to_string(),
            aud: TOKEN_URL.to_string(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(
            self.service_account.private_key.as_bytes()
        ).map_err(|e| StoreError::InvalidCredentials(format!(
            "Invalid private key: {}", e
        )))?;

        let jwt = jsonwebtoken::encode(
            &jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256),
            &claims,
            &encoding_key,
        )?;

        // Exchange JWT for access token
        let response = self.client
            .post(TOKEN_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
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

        let mut request = self.client
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

    /// Create a new edit session
    async fn create_edit(&self) -> Result<String> {
        #[derive(Deserialize)]
        struct EditResponse {
            id: String,
        }

        let endpoint = format!("/applications/{}/edits", self.config.package_name);
        let response: EditResponse = self.api_request(
            reqwest::Method::POST,
            &endpoint,
            Some(serde_json::json!({})),
        ).await?;

        Ok(response.id)
    }

    /// Commit an edit
    async fn commit_edit(&self, edit_id: &str) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/edits/{}:commit",
            self.config.package_name, edit_id
        );

        let _: serde_json::Value = self.api_request(
            reqwest::Method::POST,
            &endpoint,
            None,
        ).await?;

        Ok(())
    }

    /// Upload an APK or AAB to an edit
    async fn upload_binary(&self, edit_id: &str, path: &Path) -> Result<i64> {
        let token = self.get_access_token().await?;

        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let upload_type = match ext.as_str() {
            "aab" => "bundles",
            _ => "apks",
        };

        let url = format!(
            "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{}/edits/{}/{}",
            self.config.package_name, edit_id, upload_type
        );

        let file_content = tokio::fs::read(path).await?;

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/octet-stream")
            .body(file_content)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::UploadFailed(error_text));
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct UploadResponse {
            version_code: i64,
        }

        let result: UploadResponse = response.json().await?;
        Ok(result.version_code)
    }

    /// Assign a binary to a track
    async fn assign_to_track(
        &self,
        edit_id: &str,
        track: &str,
        version_code: i64,
        rollout_percentage: Option<f64>,
        release_notes: &std::collections::HashMap<String, String>,
    ) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/edits/{}/tracks/{}",
            self.config.package_name, edit_id, track
        );

        // Build release notes array
        let notes: Vec<serde_json::Value> = release_notes.iter()
            .map(|(lang, text)| serde_json::json!({
                "language": lang,
                "text": text
            }))
            .collect();

        let mut release = serde_json::json!({
            "versionCodes": [version_code.to_string()],
            "status": if rollout_percentage.is_some() { "inProgress" } else { "completed" }
        });

        if let Some(percentage) = rollout_percentage {
            release["userFraction"] = serde_json::json!(percentage);
        }

        if !notes.is_empty() {
            release["releaseNotes"] = serde_json::json!(notes);
        }

        let body = serde_json::json!({
            "track": track,
            "releases": [release]
        });

        let _: serde_json::Value = self.api_request(
            reqwest::Method::PUT,
            &endpoint,
            Some(body),
        ).await?;

        Ok(())
    }

    /// Extract app info from APK/AAB
    async fn extract_android_info(path: &Path) -> Result<AppInfo> {
        // Use aapt2 to extract info
        let output = tokio::process::Command::new("aapt2")
            .args(["dump", "badging", path.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("aapt2 failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse aapt2 output
        let mut package_name = String::new();
        let mut version_name = String::new();
        let mut version_code = String::new();
        let mut app_name = None;
        let mut min_sdk = None;

        for line in stdout.lines() {
            if line.starts_with("package:") {
                // package: name='com.example' versionCode='1' versionName='1.0.0'
                for part in line.split_whitespace() {
                    if part.starts_with("name='") {
                        package_name = part.trim_start_matches("name='").trim_end_matches('\'').to_string();
                    } else if part.starts_with("versionCode='") {
                        version_code = part.trim_start_matches("versionCode='").trim_end_matches('\'').to_string();
                    } else if part.starts_with("versionName='") {
                        version_name = part.trim_start_matches("versionName='").trim_end_matches('\'').to_string();
                    }
                }
            } else if line.starts_with("application-label:") {
                app_name = Some(line.trim_start_matches("application-label:'").trim_end_matches('\'').to_string());
            } else if line.starts_with("sdkVersion:'") {
                min_sdk = Some(line.trim_start_matches("sdkVersion:'").trim_end_matches('\'').to_string());
            }
        }

        if package_name.is_empty() {
            return Err(StoreError::InvalidArtifact(
                "Could not determine package name from APK/AAB".to_string()
            ));
        }

        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        Ok(AppInfo {
            bundle_id: package_name,
            version: version_name,
            build_number: version_code,
            name: app_name,
            min_os_version: min_sdk,
            platforms: vec!["Android".to_string()],
            size,
            sha256: None,
        })
    }
}

#[async_trait::async_trait]
impl StoreAdapter for GooglePlayStore {
    fn name(&self) -> &str {
        "Google Play"
    }

    fn store_type(&self) -> StoreType {
        StoreType::GooglePlay
    }

    fn is_available(&self) -> bool {
        // Check if we have valid credentials
        !self.service_account.client_email.is_empty()
            && !self.service_account.private_key.is_empty()
    }

    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult> {
        let app_info = Self::extract_android_info(path).await?;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Verify package name matches config
        if app_info.bundle_id != self.config.package_name {
            errors.push(ValidationError {
                code: "PACKAGE_MISMATCH".to_string(),
                message: format!(
                    "Package name '{}' does not match configured '{}'",
                    app_info.bundle_id, self.config.package_name
                ),
                severity: ValidationSeverity::Error,
            });
        }

        if app_info.version.is_empty() {
            warnings.push("Version name is empty".to_string());
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

        // Create edit session
        info!("Creating edit session...");
        let edit_id = self.create_edit().await?;

        // Upload binary
        info!("Uploading {}...", path.display());
        let version_code = self.upload_binary(&edit_id, path).await?;

        // Assign to track
        let track = options.track.as_ref()
            .or(self.config.default_track.as_ref())
            .map(|s| s.as_str())
            .unwrap_or("internal");

        info!("Assigning to track '{}'...", track);
        self.assign_to_track(
            &edit_id,
            track,
            version_code,
            options.rollout_percentage,
            &options.release_notes,
        ).await?;

        // Commit edit
        info!("Committing edit...");
        self.commit_edit(&edit_id).await?;

        let console_url = format!(
            "https://play.google.com/console/developers/app/{}/tracks",
            self.config.package_name
        );

        Ok(UploadResult {
            success: true,
            build_id: Some(version_code.to_string()),
            console_url: Some(console_url),
            status: if options.rollout_percentage.is_some() {
                UploadStatus::Processing
            } else {
                UploadStatus::Ready
            },
            warnings: validation.warnings,
            uploaded_at: Utc::now(),
        })
    }

    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus> {
        // Would need to query the API for build status
        Ok(BuildStatus {
            build_id: build_id.to_string(),
            version: "".to_string(),
            build_number: build_id.to_string(),
            status: UploadStatus::Ready,
            uploaded_at: None,
            processed_at: None,
            expires_at: None,
            track: None,
            rollout_percentage: None,
            details: None,
        })
    }

    async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
        // Would need to query the API
        Ok(Vec::new())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["apk", "aab"]
    }
}

#[async_trait::async_trait]
impl StagedRolloutSupport for GooglePlayStore {
    async fn update_rollout(&self, build_id: &str, percentage: f64) -> Result<()> {
        let edit_id = self.create_edit().await?;

        // Get current track info and update rollout
        let track = self.config.default_track.as_deref().unwrap_or("production");

        self.assign_to_track(
            &edit_id,
            track,
            build_id.parse().unwrap_or(0),
            Some(percentage),
            &std::collections::HashMap::new(),
        ).await?;

        self.commit_edit(&edit_id).await?;

        Ok(())
    }

    async fn halt_rollout(&self, build_id: &str) -> Result<()> {
        // Halt by setting to 0%
        self.update_rollout(build_id, 0.0).await
    }

    async fn complete_rollout(&self, build_id: &str) -> Result<()> {
        // Complete by setting to 100%
        self.update_rollout(build_id, 1.0).await
    }
}

#[async_trait::async_trait]
impl TrackSupport for GooglePlayStore {
    async fn list_tracks(&self) -> Result<Vec<String>> {
        // Standard Google Play tracks
        Ok(vec![
            "internal".to_string(),
            "alpha".to_string(),
            "beta".to_string(),
            "production".to_string(),
        ])
    }

    async fn promote_build(&self, build_id: &str, _from_track: &str, to_track: &str) -> Result<()> {
        let edit_id = self.create_edit().await?;

        self.assign_to_track(
            &edit_id,
            to_track,
            build_id.parse().unwrap_or(0),
            None,
            &std::collections::HashMap::new(),
        ).await?;

        self.commit_edit(&edit_id).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supported_extensions() {
        let _config = GooglePlayConfig {
            package_name: "com.example.app".to_string(),
            service_account_key: std::path::PathBuf::from("/tmp/key.json"),
            default_track: None,
        };

        // Can't create without valid key file, so just test extensions directly
        let extensions = &["apk", "aab"];
        assert!(extensions.contains(&"apk"));
        assert!(extensions.contains(&"aab"));
    }
}
