//! Firebase App Distribution client

use std::path::Path;

use chrono::{DateTime, Utc};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::error::{Result, StoreError};

const FIREBASE_API_BASE: &str = "https://firebaseappdistribution.googleapis.com/v1";
const FIREBASE_UPLOAD_BASE: &str = "https://firebaseappdistribution.googleapis.com/upload/v1";

/// Firebase App Distribution client
pub struct Firebase {
    config: FirebaseConfig,
    client: Client,
    access_token: Option<String>,
    token_expires: Option<DateTime<Utc>>,
}

/// Firebase configuration
#[derive(Debug, Clone)]
pub struct FirebaseConfig {
    /// Firebase project ID (e.g., "my-project-123")
    pub project_id: String,

    /// Firebase app ID (e.g., "1:123456789:ios:abcdef" or Android equivalent)
    pub app_id: String,

    /// Service account JSON content or path
    pub service_account: Option<String>,

    /// Firebase CLI token (alternative to service account)
    pub cli_token: Option<String>,
}

/// Upload options for Firebase
#[derive(Debug, Clone, Default)]
pub struct FirebaseUploadOptions {
    /// Release notes
    pub release_notes: Option<String>,

    /// Tester groups to distribute to
    pub groups: Vec<String>,

    /// Individual tester emails
    pub testers: Vec<String>,

    /// Dry run (validate but don't upload)
    pub dry_run: bool,
}

impl Firebase {
    /// Create a new Firebase client
    pub fn new(config: FirebaseConfig) -> Self {
        Self {
            config,
            client: Client::new(),
            access_token: None,
            token_expires: None,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let project_id = std::env::var("FIREBASE_PROJECT_ID")
            .or_else(|_| std::env::var("GOOGLE_CLOUD_PROJECT"))
            .map_err(|_| StoreError::ConfigurationError(
                "FIREBASE_PROJECT_ID or GOOGLE_CLOUD_PROJECT not set".to_string()
            ))?;

        let app_id = std::env::var("FIREBASE_APP_ID")
            .map_err(|_| StoreError::ConfigurationError(
                "FIREBASE_APP_ID not set".to_string()
            ))?;

        let service_account = std::env::var("GOOGLE_APPLICATION_CREDENTIALS").ok()
            .or_else(|| std::env::var("FIREBASE_SERVICE_ACCOUNT").ok());

        let cli_token = std::env::var("FIREBASE_TOKEN").ok();

        if service_account.is_none() && cli_token.is_none() {
            return Err(StoreError::ConfigurationError(
                "Either GOOGLE_APPLICATION_CREDENTIALS or FIREBASE_TOKEN must be set".to_string()
            ));
        }

        Ok(Self::new(FirebaseConfig {
            project_id,
            app_id,
            service_account,
            cli_token,
        }))
    }

    /// Get access token for API authentication
    async fn get_access_token(&mut self) -> Result<String> {
        // Check if we have a valid cached token
        if let (Some(ref token), Some(expires)) = (&self.access_token, self.token_expires) {
            if Utc::now() < expires {
                return Ok(token.clone());
            }
        }

        // Use Firebase CLI token if available
        if let Some(ref cli_token) = self.config.cli_token {
            // Firebase CLI tokens can be used directly
            self.access_token = Some(cli_token.clone());
            self.token_expires = Some(Utc::now() + chrono::Duration::hours(1));
            return Ok(cli_token.clone());
        }

        // Use service account to get OAuth2 token
        if let Some(sa_path) = self.config.service_account.clone() {
            let token = self.get_service_account_token(&sa_path).await?;
            return Ok(token);
        }

        Err(StoreError::InvalidCredentials(
            "No valid authentication method available".to_string()
        ))
    }

    /// Get OAuth2 token from service account
    async fn get_service_account_token(&mut self, sa_path: &str) -> Result<String> {
        // Read service account JSON
        let sa_content = if Path::new(sa_path).exists() {
            std::fs::read_to_string(sa_path)
                .map_err(|e| StoreError::ConfigurationError(
                    format!("Failed to read service account file: {}", e)
                ))?
        } else {
            // Assume it's the JSON content directly
            sa_path.to_string()
        };

        #[derive(Deserialize)]
        struct ServiceAccount {
            client_email: String,
            private_key: String,
            token_uri: String,
        }

        let sa: ServiceAccount = serde_json::from_str(&sa_content)
            .map_err(|e| StoreError::ConfigurationError(
                format!("Invalid service account JSON: {}", e)
            ))?;

        // Create JWT
        use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

        #[derive(Serialize)]
        struct Claims {
            iss: String,
            scope: String,
            aud: String,
            iat: i64,
            exp: i64,
        }

        let now = Utc::now();
        let exp = now + chrono::Duration::hours(1);

        let claims = Claims {
            iss: sa.client_email,
            scope: "https://www.googleapis.com/auth/cloud-platform".to_string(),
            aud: sa.token_uri.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let encoding_key = EncodingKey::from_rsa_pem(sa.private_key.as_bytes())
            .map_err(|e| StoreError::InvalidCredentials(
                format!("Invalid service account private key: {}", e)
            ))?;

        let jwt = encode(&Header::new(Algorithm::RS256), &claims, &encoding_key)
            .map_err(|e| StoreError::InvalidCredentials(
                format!("Failed to create JWT: {}", e)
            ))?;

        // Exchange JWT for access token
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }

        let response = self.client
            .post(&sa.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: 401,
                message: format!("Token exchange failed: {}", error_text),
            });
        }

        let token_response: TokenResponse = response.json().await?;

        self.access_token = Some(token_response.access_token.clone());
        self.token_expires = Some(now + chrono::Duration::seconds(token_response.expires_in - 60));

        Ok(token_response.access_token)
    }

    /// Make an authenticated API request
    async fn api_request<T: serde::de::DeserializeOwned>(
        &mut self,
        method: reqwest::Method,
        url: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let token = self.get_access_token().await?;

        debug!("Firebase API request: {} {}", method, url);

        let mut request = self.client
            .request(method, url)
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

    // -------------------------------------------------------------------------
    // Upload Operations
    // -------------------------------------------------------------------------

    /// Upload an artifact to Firebase App Distribution
    pub async fn upload(
        &mut self,
        path: &Path,
        options: &FirebaseUploadOptions,
    ) -> Result<FirebaseRelease> {
        // Validate file exists
        if !path.exists() {
            return Err(StoreError::InvalidArtifact(
                format!("File not found: {}", path.display())
            ));
        }

        // Validate file type
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        if !matches!(ext.as_str(), "apk" | "aab" | "ipa") {
            return Err(StoreError::InvalidArtifact(
                format!("Unsupported file type: {}. Expected APK, AAB, or IPA.", ext)
            ));
        }

        if options.dry_run {
            info!("Dry run - would upload {}", path.display());
            return Ok(FirebaseRelease {
                name: "dry-run".to_string(),
                display_version: "dry-run".to_string(),
                build_version: "dry-run".to_string(),
                release_notes: options.release_notes.clone(),
                create_time: Utc::now(),
                firebase_console_uri: None,
            });
        }

        info!("Uploading {} to Firebase App Distribution", path.display());

        // Step 1: Upload the binary
        let upload_url = format!(
            "{}/projects/{}/apps/{}/releases:upload",
            FIREBASE_UPLOAD_BASE,
            self.config.project_id,
            self.config.app_id
        );

        let token = self.get_access_token().await?;
        let file_content = tokio::fs::read(path).await
            .map_err(|e| StoreError::Io(e))?;

        let file_name = path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("app")
            .to_string();

        let part = Part::bytes(file_content)
            .file_name(file_name)
            .mime_str(match ext.as_str() {
                "apk" => "application/vnd.android.package-archive",
                "aab" => "application/octet-stream",
                "ipa" => "application/octet-stream",
                _ => "application/octet-stream",
            })
            .map_err(|e| StoreError::UploadFailed(format!("Failed to create multipart: {}", e)))?;

        let form = Form::new().part("file", part);

        let response = self.client
            .post(&upload_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("X-Goog-Upload-Protocol", "multipart")
            .multipart(form)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::UploadFailed(format!(
                "Upload failed ({}): {}",
                status, error_text
            )));
        }

        #[derive(Deserialize)]
        struct UploadResponse {
            name: Option<String>,
            result: Option<UploadResult>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct UploadResult {
            release: Option<ReleaseData>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReleaseData {
            name: String,
            #[allow(dead_code)]
            display_version: Option<String>,
            #[allow(dead_code)]
            build_version: Option<String>,
            #[allow(dead_code)]
            create_time: Option<String>,
            #[allow(dead_code)]
            firebase_console_uri: Option<String>,
        }

        let upload_response: UploadResponse = response.json().await?;

        let release_name = if let Some(ref result) = upload_response.result {
            if let Some(ref release) = result.release {
                release.name.clone()
            } else {
                upload_response.name.unwrap_or_else(|| "unknown".to_string())
            }
        } else {
            upload_response.name.unwrap_or_else(|| "unknown".to_string())
        };

        info!("Upload complete, release: {}", release_name);

        // Step 2: Update release notes if provided
        if let Some(ref notes) = options.release_notes {
            self.update_release_notes(&release_name, notes).await?;
        }

        // Step 3: Distribute to groups/testers if specified
        if !options.groups.is_empty() || !options.testers.is_empty() {
            self.distribute_release(
                &release_name,
                &options.groups,
                &options.testers,
            ).await?;
        }

        // Get final release info
        let release = self.get_release(&release_name).await?;

        Ok(release)
    }

    /// Update release notes for a release
    async fn update_release_notes(&mut self, release_name: &str, notes: &str) -> Result<()> {
        let url = format!("{}/{}", FIREBASE_API_BASE, release_name);

        let body = serde_json::json!({
            "releaseNotes": {
                "text": notes
            }
        });

        let token = self.get_access_token().await?;

        let response = self.client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .query(&[("updateMask", "releaseNotes.text")])
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to update release notes: {}", error_text),
            });
        }

        info!("Updated release notes");
        Ok(())
    }

    /// Distribute a release to groups and testers
    async fn distribute_release(
        &mut self,
        release_name: &str,
        groups: &[String],
        testers: &[String],
    ) -> Result<()> {
        let url = format!("{}/{}:distribute", FIREBASE_API_BASE, release_name);

        let body = serde_json::json!({
            "testerEmails": testers,
            "groupAliases": groups
        });

        let token = self.get_access_token().await?;

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to distribute release: {}", error_text),
            });
        }

        info!(
            "Distributed release to {} group(s) and {} tester(s)",
            groups.len(),
            testers.len()
        );
        Ok(())
    }

    /// Get release information
    pub async fn get_release(&mut self, release_name: &str) -> Result<FirebaseRelease> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReleaseResponse {
            name: String,
            display_version: Option<String>,
            build_version: Option<String>,
            release_notes: Option<ReleaseNotes>,
            create_time: Option<String>,
            firebase_console_uri: Option<String>,
        }

        #[derive(Deserialize)]
        struct ReleaseNotes {
            text: Option<String>,
        }

        let url = format!("{}/{}", FIREBASE_API_BASE, release_name);
        let response: ReleaseResponse = self.api_request(reqwest::Method::GET, &url, None).await?;

        Ok(FirebaseRelease {
            name: response.name,
            display_version: response.display_version.unwrap_or_default(),
            build_version: response.build_version.unwrap_or_default(),
            release_notes: response.release_notes.and_then(|rn| rn.text),
            create_time: response.create_time
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(Utc::now),
            firebase_console_uri: response.firebase_console_uri,
        })
    }

    // -------------------------------------------------------------------------
    // Release Management
    // -------------------------------------------------------------------------

    /// List recent releases
    pub async fn list_releases(&mut self, limit: Option<usize>) -> Result<Vec<FirebaseRelease>> {
        #[derive(Deserialize)]
        struct ReleasesResponse {
            releases: Option<Vec<ReleaseData>>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ReleaseData {
            name: String,
            display_version: Option<String>,
            build_version: Option<String>,
            release_notes: Option<ReleaseNotes>,
            create_time: Option<String>,
            firebase_console_uri: Option<String>,
        }

        #[derive(Deserialize)]
        struct ReleaseNotes {
            text: Option<String>,
        }

        let limit = limit.unwrap_or(25);
        let url = format!(
            "{}/projects/{}/apps/{}/releases?pageSize={}",
            FIREBASE_API_BASE,
            self.config.project_id,
            self.config.app_id,
            limit
        );

        let response: ReleasesResponse = self.api_request(reqwest::Method::GET, &url, None).await?;

        let releases = response.releases.unwrap_or_default();
        Ok(releases.into_iter().map(|r| {
            FirebaseRelease {
                name: r.name,
                display_version: r.display_version.unwrap_or_default(),
                build_version: r.build_version.unwrap_or_default(),
                release_notes: r.release_notes.and_then(|rn| rn.text),
                create_time: r.create_time
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now),
                firebase_console_uri: r.firebase_console_uri,
            }
        }).collect())
    }

    // -------------------------------------------------------------------------
    // Tester Management
    // -------------------------------------------------------------------------

    /// List tester groups
    pub async fn list_groups(&mut self) -> Result<Vec<TesterGroup>> {
        #[derive(Deserialize)]
        struct GroupsResponse {
            groups: Option<Vec<GroupData>>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GroupData {
            name: String,
            display_name: Option<String>,
            tester_count: Option<i32>,
        }

        let url = format!(
            "{}/projects/{}/groups",
            FIREBASE_API_BASE,
            self.config.project_id
        );

        let response: GroupsResponse = self.api_request(reqwest::Method::GET, &url, None).await?;

        let groups = response.groups.unwrap_or_default();
        Ok(groups.into_iter().map(|g| {
            // Extract alias from name (e.g., "projects/xxx/groups/alias")
            let alias = g.name.rsplit('/').next().unwrap_or(&g.name).to_string();
            TesterGroup {
                name: g.name,
                alias,
                display_name: g.display_name,
                tester_count: g.tester_count.unwrap_or(0) as u32,
            }
        }).collect())
    }

    /// Create a tester group
    pub async fn create_group(&mut self, alias: &str, display_name: Option<&str>) -> Result<TesterGroup> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GroupResponse {
            name: String,
            display_name: Option<String>,
        }

        let url = format!(
            "{}/projects/{}/groups?groupId={}",
            FIREBASE_API_BASE,
            self.config.project_id,
            alias
        );

        let body = serde_json::json!({
            "displayName": display_name.unwrap_or(alias)
        });

        let response: GroupResponse = self.api_request(reqwest::Method::POST, &url, Some(body)).await?;

        Ok(TesterGroup {
            name: response.name,
            alias: alias.to_string(),
            display_name: response.display_name,
            tester_count: 0,
        })
    }

    /// Delete a tester group
    pub async fn delete_group(&mut self, group_alias: &str) -> Result<()> {
        let url = format!(
            "{}/projects/{}/groups/{}",
            FIREBASE_API_BASE,
            self.config.project_id,
            group_alias
        );

        let token = self.get_access_token().await?;

        let response = self.client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to delete group: {}", error_text),
            });
        }

        Ok(())
    }

    /// Add testers to a group
    pub async fn add_testers_to_group(
        &mut self,
        group_alias: &str,
        emails: &[&str],
    ) -> Result<()> {
        let url = format!(
            "{}/projects/{}/groups/{}:batchJoin",
            FIREBASE_API_BASE,
            self.config.project_id,
            group_alias
        );

        let body = serde_json::json!({
            "emails": emails
        });

        let token = self.get_access_token().await?;

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to add testers: {}", error_text),
            });
        }

        info!("Added {} tester(s) to group '{}'", emails.len(), group_alias);
        Ok(())
    }

    /// Remove testers from a group
    pub async fn remove_testers_from_group(
        &mut self,
        group_alias: &str,
        emails: &[&str],
    ) -> Result<()> {
        let url = format!(
            "{}/projects/{}/groups/{}:batchLeave",
            FIREBASE_API_BASE,
            self.config.project_id,
            group_alias
        );

        let body = serde_json::json!({
            "emails": emails
        });

        let token = self.get_access_token().await?;

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(StoreError::ApiError {
                status: status.as_u16(),
                message: format!("Failed to remove testers: {}", error_text),
            });
        }

        info!("Removed {} tester(s) from group '{}'", emails.len(), group_alias);
        Ok(())
    }
}

// =============================================================================
// Types
// =============================================================================

/// Firebase release information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirebaseRelease {
    /// Full resource name
    pub name: String,
    /// Display version (e.g., "1.2.3")
    pub display_version: String,
    /// Build version (e.g., "42")
    pub build_version: String,
    /// Release notes
    pub release_notes: Option<String>,
    /// Creation time
    pub create_time: DateTime<Utc>,
    /// Firebase console URL
    pub firebase_console_uri: Option<String>,
}

/// Tester group information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TesterGroup {
    /// Full resource name
    pub name: String,
    /// Group alias (short name)
    pub alias: String,
    /// Display name
    pub display_name: Option<String>,
    /// Number of testers
    pub tester_count: u32,
}

/// Distribution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DistributionStatus {
    /// Distribution pending
    Pending,
    /// Distributed to testers
    Distributed,
    /// Distribution failed
    Failed,
}

/// Release info summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseInfo {
    /// Release ID
    pub id: String,
    /// Version string
    pub version: String,
    /// Build number
    pub build: String,
    /// Distribution status
    pub status: DistributionStatus,
    /// Distributed groups
    pub groups: Vec<String>,
    /// Creation time
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firebase_config() {
        let config = FirebaseConfig {
            project_id: "test-project".to_string(),
            app_id: "1:123:ios:abc".to_string(),
            service_account: None,
            cli_token: Some("test-token".to_string()),
        };

        assert_eq!(config.project_id, "test-project");
        assert_eq!(config.app_id, "1:123:ios:abc");
    }

    #[test]
    fn test_upload_options() {
        let options = FirebaseUploadOptions {
            release_notes: Some("Test release".to_string()),
            groups: vec!["testers".to_string()],
            testers: vec!["test@example.com".to_string()],
            dry_run: false,
        };

        assert_eq!(options.groups.len(), 1);
        assert_eq!(options.testers.len(), 1);
    }
}
