//! App Store Connect API integration
//!
//! Provides upload and management capabilities via the App Store Connect API.

use crate::error::{Result, StoreError};
use crate::traits::{NotarizationSupport, StoreAdapter};
use crate::types::*;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

use super::notarize::Notarizer;

const API_BASE_URL: &str = "https://api.appstoreconnect.apple.com/v1";

/// JWT claims for App Store Connect API
#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
    aud: String,
}

/// App Store Connect API client
pub struct AppStoreConnect {
    /// Configuration
    config: AppleStoreConfig,

    /// HTTP client
    client: Client,

    /// Notarizer for macOS apps
    notarizer: Option<Notarizer>,

    /// Cached JWT token
    jwt_token: Option<String>,

    /// Token expiration time
    token_expires: Option<chrono::DateTime<Utc>>,
}

impl AppStoreConnect {
    /// Create a new App Store Connect client
    pub fn new(config: AppleStoreConfig) -> Result<Self> {
        let notarizer = if config.notarize {
            Some(Notarizer::new(&config)?)
        } else {
            None
        };

        Ok(Self {
            config,
            client: Client::new(),
            notarizer,
            jwt_token: None,
            token_expires: None,
        })
    }

    /// Generate a JWT token for API authentication
    fn generate_jwt(&mut self) -> Result<String> {
        // Check if we have a valid cached token
        if let (Some(token), Some(expires)) = (&self.jwt_token, self.token_expires) {
            if Utc::now() < expires - Duration::minutes(5) {
                return Ok(token.clone());
            }
        }

        let now = Utc::now();
        let exp = now + Duration::minutes(20); // Token valid for 20 minutes

        let claims = Claims {
            iss: self.config.api_issuer_id.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            aud: "appstoreconnect-v1".to_string(),
        };

        // Read the private key
        let key_content = if Path::new(&self.config.api_key).exists() {
            std::fs::read_to_string(&self.config.api_key)
                .map_err(|e| StoreError::ConfigurationError(format!("Failed to read API key: {}", e)))?
        } else {
            self.config.api_key.clone()
        };

        let encoding_key = EncodingKey::from_ec_pem(key_content.as_bytes())
            .map_err(|e| StoreError::InvalidCredentials(format!("Invalid API key: {}", e)))?;

        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.config.api_key_id.clone());

        let token = encode(&header, &claims, &encoding_key)?;

        // Cache the token
        self.jwt_token = Some(token.clone());
        self.token_expires = Some(exp);

        Ok(token)
    }

    /// Make an authenticated API request
    async fn api_request<T: serde::de::DeserializeOwned>(
        &mut self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        let mut request = self.client
            .request(method, &url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json");

        if let Some(body) = body {
            request = request.json(&body);
        }

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

    /// Get app information by bundle ID
    pub async fn get_app(&mut self, bundle_id: &str) -> Result<AppInfo> {
        #[derive(Deserialize)]
        struct AppsResponse {
            data: Vec<AppData>,
        }

        #[derive(Deserialize)]
        struct AppData {
            #[allow(dead_code)]
            id: String,
            attributes: AppAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct AppAttributes {
            name: String,
            bundle_id: String,
        }

        let endpoint = format!("/apps?filter[bundleId]={}", bundle_id);
        let response: AppsResponse = self.api_request(reqwest::Method::GET, &endpoint, None).await?;

        let app = response.data.first()
            .ok_or_else(|| StoreError::AppNotFound(bundle_id.to_string()))?;

        Ok(AppInfo {
            bundle_id: app.attributes.bundle_id.clone(),
            version: "".to_string(),
            build_number: "".to_string(),
            name: Some(app.attributes.name.clone()),
            min_os_version: None,
            platforms: Vec::new(),
            size: 0,
            sha256: None,
        })
    }

    /// Upload an artifact using altool/Transporter
    async fn upload_with_transporter(&self, path: &Path) -> Result<UploadResult> {
        info!("Uploading {} via Transporter", path.display());

        // Use xcrun altool for uploads (or iTMSTransporter directly)
        let mut cmd = Command::new("xcrun");
        cmd.args(["altool", "--upload-app"]);
        cmd.args(["-f", path.to_str().unwrap()]);
        cmd.args(["--type", self.detect_platform_type(path)]);
        cmd.args(["--apiKey", &self.config.api_key_id]);
        cmd.args(["--apiIssuer", &self.config.api_issuer_id]);

        let output = cmd
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("altool failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        debug!("altool stdout: {}", stdout);
        if !stderr.is_empty() {
            debug!("altool stderr: {}", stderr);
        }

        if !output.status.success() {
            return Err(StoreError::UploadFailed(format!(
                "Upload failed: {}",
                stderr
            )));
        }

        // Parse the output to extract any build ID or confirmation
        let build_id = self.extract_build_id(&stdout);

        Ok(UploadResult {
            success: true,
            build_id,
            console_url: Some("https://appstoreconnect.apple.com/apps".to_string()),
            status: UploadStatus::Processing,
            warnings: Vec::new(),
            uploaded_at: Utc::now(),
        })
    }

    /// Detect platform type from artifact
    fn detect_platform_type(&self, path: &Path) -> &'static str {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        match ext.to_lowercase().as_str() {
            "ipa" => "ios",
            "app" | "pkg" | "dmg" => "osx",
            _ => "ios", // Default to iOS
        }
    }

    /// Extract build ID from upload output
    fn extract_build_id(&self, _output: &str) -> Option<String> {
        // altool outputs something like "No errors uploading 'filename'."
        // The build ID is assigned asynchronously by App Store Connect
        None
    }

    /// Check if altool/Transporter is available
    fn is_altool_available() -> bool {
        std::process::Command::new("xcrun")
            .args(["altool", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

#[async_trait::async_trait]
impl StoreAdapter for AppStoreConnect {
    fn name(&self) -> &str {
        "App Store Connect"
    }

    fn store_type(&self) -> StoreType {
        StoreType::Apple
    }

    fn is_available(&self) -> bool {
        Self::is_altool_available()
    }

    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult> {
        let app_info = super::extract_app_info(path).await?;

        // Basic validation
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        if app_info.bundle_id.is_empty() {
            errors.push(ValidationError {
                code: "MISSING_BUNDLE_ID".to_string(),
                message: "Bundle identifier is missing".to_string(),
                severity: ValidationSeverity::Error,
            });
        }

        if app_info.version.is_empty() || app_info.version == "0.0.0" {
            warnings.push("Version string appears to be missing or default".to_string());
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

        // Notarize first if configured and this is a macOS app
        if self.config.notarize {
            if let Some(notarizer) = &self.notarizer {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if matches!(ext.to_lowercase().as_str(), "app" | "pkg" | "dmg" | "zip") {
                    info!("Notarizing before upload...");
                    notarizer.notarize(path, options.timeout).await?;
                }
            }
        }

        // Upload
        self.upload_with_transporter(path).await
    }

    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus> {
        // This would require API calls to get build status
        // For now, return a placeholder
        Ok(BuildStatus {
            build_id: build_id.to_string(),
            version: "".to_string(),
            build_number: "".to_string(),
            status: UploadStatus::Processing,
            uploaded_at: None,
            processed_at: None,
            expires_at: None,
            track: None,
            rollout_percentage: None,
            details: None,
        })
    }

    async fn list_builds(&self, _limit: Option<usize>) -> Result<Vec<Build>> {
        // This would require API calls to list builds
        // For now, return empty
        Ok(Vec::new())
    }

    fn supported_extensions(&self) -> &[&str] {
        &["ipa", "app", "pkg", "dmg", "zip"]
    }
}

#[async_trait::async_trait]
impl NotarizationSupport for AppStoreConnect {
    async fn submit_for_notarization(&self, path: &Path) -> Result<String> {
        let notarizer = self.notarizer.as_ref()
            .ok_or_else(|| StoreError::ConfigurationError("Notarization not configured".to_string()))?;
        notarizer.submit(path).await
    }

    async fn check_notarization_status(&self, submission_id: &str) -> Result<NotarizationResult> {
        let notarizer = self.notarizer.as_ref()
            .ok_or_else(|| StoreError::ConfigurationError("Notarization not configured".to_string()))?;
        notarizer.status(submission_id).await
    }

    async fn wait_for_notarization(
        &self,
        submission_id: &str,
        timeout_secs: Option<u64>,
    ) -> Result<NotarizationResult> {
        let notarizer = self.notarizer.as_ref()
            .ok_or_else(|| StoreError::ConfigurationError("Notarization not configured".to_string()))?;
        notarizer.wait(submission_id, timeout_secs).await
    }

    async fn staple(&self, path: &Path) -> Result<()> {
        let notarizer = self.notarizer.as_ref()
            .ok_or_else(|| StoreError::ConfigurationError("Notarization not configured".to_string()))?;
        notarizer.staple(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_platform_type() {
        let config = AppleStoreConfig {
            api_key_id: "test".to_string(),
            api_issuer_id: "test".to_string(),
            api_key: "test".to_string(),
            team_id: None,
            app_id: None,
            notarize: false,
            staple: false,
            primary_locale: None,
        };

        let client = AppStoreConnect {
            config,
            client: Client::new(),
            notarizer: None,
            jwt_token: None,
            token_expires: None,
        };

        assert_eq!(client.detect_platform_type(Path::new("app.ipa")), "ios");
        assert_eq!(client.detect_platform_type(Path::new("app.pkg")), "osx");
        assert_eq!(client.detect_platform_type(Path::new("app.dmg")), "osx");
    }
}
