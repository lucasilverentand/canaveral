//! Google Play Console metadata synchronization.
//!
//! This module provides integration with Google Play Developer API for
//! syncing app metadata between local storage and Google Play Console.
//!
//! ## Authentication
//!
//! Authentication uses Google Cloud service account credentials with OAuth 2.0.
//! The service account must have appropriate permissions in Google Play Console.
//!
//! ## Edit-Based Workflow
//!
//! Google Play API uses an edit-based workflow:
//! 1. Create an edit session
//! 2. Make changes within the edit
//! 3. Commit the edit to apply changes (or delete to discard)

use super::{ChangeType, MetadataChange, MetadataDiff, MetadataSync, PushResult};
use crate::{
    FastlaneStorage, GooglePlayLocalizedMetadata, GooglePlayMetadata, Locale, MetadataError,
    MetadataStorage, Result,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Base URL for Google Play Developer API v3.
const API_BASE_URL: &str = "https://androidpublisher.googleapis.com/androidpublisher/v3";

/// OAuth 2.0 token endpoint for Google.
const GOOGLE_TOKEN_URL: &str = "https://oauth2.googleapis.com/token";

/// Scope required for Google Play Developer API.
const ANDROID_PUBLISHER_SCOPE: &str = "https://www.googleapis.com/auth/androidpublisher";

/// Default retry delay in milliseconds.
const RETRY_DELAY_MS: u64 = 1000;

/// Maximum number of retries for rate-limited requests.
const MAX_RETRIES: u32 = 3;

/// Google Play image types for screenshots and graphics.
pub mod image_types {
    /// Phone screenshots.
    pub const PHONE_SCREENSHOTS: &str = "phoneScreenshots";
    /// 7-inch tablet screenshots.
    pub const SEVEN_INCH_SCREENSHOTS: &str = "sevenInchScreenshots";
    /// 10-inch tablet screenshots.
    pub const TEN_INCH_SCREENSHOTS: &str = "tenInchScreenshots";
    /// Android TV screenshots.
    pub const TV_SCREENSHOTS: &str = "tvScreenshots";
    /// Wear OS screenshots.
    pub const WEAR_SCREENSHOTS: &str = "wearScreenshots";
    /// Feature graphic.
    pub const FEATURE_GRAPHIC: &str = "featureGraphic";
    /// Promo graphic.
    pub const PROMO_GRAPHIC: &str = "promoGraphic";
    /// TV banner.
    pub const TV_BANNER: &str = "tvBanner";
    /// App icon.
    pub const ICON: &str = "icon";
}

/// Configuration for Google Play Console sync.
#[derive(Debug, Clone)]
pub struct GooglePlaySyncConfig {
    /// Path to service account JSON key file.
    pub service_account_key_path: Option<PathBuf>,
    /// Or the JSON content directly.
    pub service_account_key_json: Option<String>,
}

impl GooglePlaySyncConfig {
    /// Creates a new config from a service account key file path.
    pub fn from_key_file(path: impl Into<PathBuf>) -> Self {
        Self {
            service_account_key_path: Some(path.into()),
            service_account_key_json: None,
        }
    }

    /// Creates a new config from service account JSON content.
    pub fn from_key_json(json: impl Into<String>) -> Self {
        Self {
            service_account_key_path: None,
            service_account_key_json: Some(json.into()),
        }
    }

    /// Creates a new config from environment variables.
    ///
    /// Looks for:
    /// - `GOOGLE_PLAY_SERVICE_ACCOUNT_KEY` (JSON content or path to file)
    /// - `GOOGLE_APPLICATION_CREDENTIALS` (path to file, fallback)
    pub fn from_env() -> Result<Self> {
        // Try GOOGLE_PLAY_SERVICE_ACCOUNT_KEY first
        if let Ok(key_value) = std::env::var("GOOGLE_PLAY_SERVICE_ACCOUNT_KEY") {
            // Check if it's a file path or JSON content
            let path = Path::new(&key_value);
            if path.exists() && path.is_file() {
                return Ok(Self::from_key_file(path));
            }
            // Assume it's JSON content if it looks like JSON
            if key_value.trim().starts_with('{') {
                return Ok(Self::from_key_json(key_value));
            }
            return Err(MetadataError::SyncError(
                "GOOGLE_PLAY_SERVICE_ACCOUNT_KEY is neither a valid file path nor JSON content"
                    .to_string(),
            ));
        }

        // Fallback to GOOGLE_APPLICATION_CREDENTIALS
        if let Ok(path) = std::env::var("GOOGLE_APPLICATION_CREDENTIALS") {
            let path = PathBuf::from(path);
            if path.exists() {
                return Ok(Self::from_key_file(path));
            }
            return Err(MetadataError::SyncError(format!(
                "GOOGLE_APPLICATION_CREDENTIALS file not found: {:?}",
                path
            )));
        }

        Err(MetadataError::SyncError(
            "No Google Play service account credentials found. Set GOOGLE_PLAY_SERVICE_ACCOUNT_KEY or GOOGLE_APPLICATION_CREDENTIALS".to_string()
        ))
    }

    /// Loads and parses the service account key.
    fn load_service_account_key(&self) -> Result<ServiceAccountKey> {
        let json_content = if let Some(ref json) = self.service_account_key_json {
            json.clone()
        } else if let Some(ref path) = self.service_account_key_path {
            std::fs::read_to_string(path).map_err(|e| {
                MetadataError::SyncError(format!("Failed to read service account key file: {}", e))
            })?
        } else {
            return Err(MetadataError::SyncError(
                "No service account key configured".to_string(),
            ));
        };

        serde_json::from_str(&json_content).map_err(|e| {
            MetadataError::SyncError(format!("Failed to parse service account key: {}", e))
        })
    }
}

/// Google Play Console metadata sync client.
///
/// Provides methods for pulling and pushing metadata to/from Google Play Console.
pub struct GooglePlayMetadataSync {
    /// Configuration for API authentication.
    config: GooglePlaySyncConfig,
    /// Local storage backend.
    storage: FastlaneStorage,
    /// HTTP client.
    client: Client,
    /// Cached access token.
    access_token: RwLock<Option<AccessTokenCache>>,
}

/// Cached access token with expiration.
struct AccessTokenCache {
    token: String,
    expires_at: chrono::DateTime<Utc>,
}

impl GooglePlayMetadataSync {
    /// Creates a new Google Play metadata sync client.
    ///
    /// # Arguments
    ///
    /// * `config` - Google Play Console API configuration
    /// * `storage_path` - Base path for local metadata storage
    pub async fn new(config: GooglePlaySyncConfig, storage_path: PathBuf) -> Result<Self> {
        let storage = FastlaneStorage::new(storage_path);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .map_err(|e| {
                MetadataError::SyncError(format!("Failed to create HTTP client: {}", e))
            })?;

        Ok(Self {
            config,
            storage,
            client,
            access_token: RwLock::new(None),
        })
    }

    /// Authenticate using service account and obtain an access token.
    async fn authenticate(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.access_token.read().unwrap();
            if let Some(ref cached) = *cache {
                // Return cached token if it's still valid (with 5 minute buffer)
                if Utc::now() < cached.expires_at - Duration::minutes(5) {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Load service account key
        let service_account = self.config.load_service_account_key()?;

        // Generate JWT for token request
        let now = Utc::now();
        let exp = now + Duration::hours(1);

        let claims = GoogleJwtClaims {
            iss: service_account.client_email.clone(),
            scope: ANDROID_PUBLISHER_SCOPE.to_string(),
            aud: service_account
                .token_uri
                .clone()
                .unwrap_or_else(|| GOOGLE_TOKEN_URL.to_string()),
            iat: now.timestamp(),
            exp: exp.timestamp(),
        };

        let encoding_key = EncodingKey::from_rsa_pem(service_account.private_key.as_bytes())
            .map_err(|e| MetadataError::SyncError(format!("Invalid private key: {}", e)))?;

        let jwt = encode(&Header::new(Algorithm::RS256), &claims, &encoding_key)
            .map_err(|e| MetadataError::SyncError(format!("Failed to generate JWT: {}", e)))?;

        // Exchange JWT for access token
        let token_url = service_account
            .token_uri
            .unwrap_or_else(|| GOOGLE_TOKEN_URL.to_string());

        let response = self
            .client
            .post(&token_url)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", &jwt),
            ])
            .send()
            .await
            .map_err(|e| MetadataError::SyncError(format!("Token request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(MetadataError::AuthenticationError(format!(
                "Failed to obtain access token: {}",
                error_text
            )));
        }

        let token_response: TokenResponse = response.json().await.map_err(|e| {
            MetadataError::SyncError(format!("Failed to parse token response: {}", e))
        })?;

        // Calculate actual expiry
        let expires_at = Utc::now() + Duration::seconds(token_response.expires_in as i64);

        // Cache the token
        {
            let mut cache = self.access_token.write().unwrap();
            *cache = Some(AccessTokenCache {
                token: token_response.access_token.clone(),
                expires_at,
            });
        }

        Ok(token_response.access_token)
    }

    /// Ensure we have a valid access token.
    async fn ensure_authenticated(&self) -> Result<String> {
        self.authenticate().await
    }

    /// Make an authenticated GET request.
    async fn api_get<T: serde::de::DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        self.api_request::<T, ()>("GET", endpoint, None).await
    }

    /// Make an authenticated POST request with a body.
    async fn api_post<T: serde::de::DeserializeOwned, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.api_request("POST", endpoint, Some(body)).await
    }

    /// Make an authenticated PUT request with a body.
    async fn api_put<T: serde::de::DeserializeOwned, B: Serialize>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T> {
        self.api_request("PUT", endpoint, Some(body)).await
    }

    /// Make an authenticated DELETE request.
    async fn api_delete(&self, endpoint: &str) -> Result<()> {
        let mut retries = 0;

        loop {
            let token = self.ensure_authenticated().await?;
            let url = format!("{}{}", API_BASE_URL, endpoint);

            debug!("API DELETE request: {}", url);

            let response = self
                .client
                .delete(&url)
                .header("Authorization", format!("Bearer {}", token))
                .send()
                .await
                .map_err(|e| MetadataError::SyncError(format!("API request failed: {}", e)))?;

            let status = response.status();

            if status == StatusCode::TOO_MANY_REQUESTS {
                if retries >= MAX_RETRIES {
                    return Err(MetadataError::RateLimited("Too many requests".to_string()));
                }

                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(RETRY_DELAY_MS / 1000);

                warn!(
                    "Rate limited, waiting {} seconds before retry ({}/{})",
                    retry_after,
                    retries + 1,
                    MAX_RETRIES
                );

                sleep(std::time::Duration::from_secs(retry_after)).await;
                retries += 1;
                continue;
            }

            if !status.is_success() && status != StatusCode::NO_CONTENT {
                let error_text = response.text().await.unwrap_or_default();
                return Err(MetadataError::SyncError(format!(
                    "API error ({}): {}",
                    status, error_text
                )));
            }

            return Ok(());
        }
    }

    /// Make an authenticated API request with retry logic.
    async fn api_request<T: serde::de::DeserializeOwned, B: Serialize>(
        &self,
        method: &str,
        endpoint: &str,
        body: Option<&B>,
    ) -> Result<T> {
        let mut retries = 0;

        loop {
            let token = self.ensure_authenticated().await?;
            let url = format!("{}{}", API_BASE_URL, endpoint);

            debug!("API {} request: {}", method, url);

            let request_builder = match method {
                "GET" => self.client.get(&url),
                "POST" => self.client.post(&url),
                "PUT" => self.client.put(&url),
                "PATCH" => self.client.patch(&url),
                _ => {
                    return Err(MetadataError::SyncError(format!(
                        "Unsupported HTTP method: {}",
                        method
                    )))
                }
            };

            let mut request = request_builder
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json");

            if let Some(ref body) = body {
                request = request.json(body);
            }

            let response = request
                .send()
                .await
                .map_err(|e| MetadataError::SyncError(format!("API request failed: {}", e)))?;

            let status = response.status();

            // Handle rate limiting
            if status == StatusCode::TOO_MANY_REQUESTS {
                if retries >= MAX_RETRIES {
                    return Err(MetadataError::RateLimited("Too many requests".to_string()));
                }

                let retry_after = response
                    .headers()
                    .get("Retry-After")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(RETRY_DELAY_MS / 1000);

                warn!(
                    "Rate limited, waiting {} seconds before retry ({}/{})",
                    retry_after,
                    retries + 1,
                    MAX_RETRIES
                );

                sleep(std::time::Duration::from_secs(retry_after)).await;
                retries += 1;
                continue;
            }

            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();
                return Err(MetadataError::SyncError(format!(
                    "API error ({}): {}",
                    status, error_text
                )));
            }

            let result = response.json().await.map_err(|e| {
                MetadataError::SyncError(format!("Failed to parse API response: {}", e))
            })?;

            return Ok(result);
        }
    }

    // ========================================================================
    // Edit workflow methods
    // ========================================================================

    /// Create a new edit session.
    ///
    /// An edit is required before making any changes to an app's metadata.
    pub async fn create_edit(&self, package_name: &str) -> Result<String> {
        let endpoint = format!("/applications/{}/edits", package_name);

        #[derive(Serialize)]
        struct CreateEditRequest {}

        let response: EditResponse = self.api_post(&endpoint, &CreateEditRequest {}).await?;

        debug!("Created edit session: {}", response.id);
        Ok(response.id)
    }

    /// Commit an edit session.
    ///
    /// This applies all changes made within the edit.
    pub async fn commit_edit(&self, package_name: &str, edit_id: &str) -> Result<()> {
        let endpoint = format!("/applications/{}/edits/{}:commit", package_name, edit_id);

        #[derive(Serialize)]
        struct CommitRequest {}

        let _: EditResponse = self.api_post(&endpoint, &CommitRequest {}).await?;

        info!("Committed edit session: {}", edit_id);
        Ok(())
    }

    /// Delete an edit session (discard changes).
    pub async fn delete_edit(&self, package_name: &str, edit_id: &str) -> Result<()> {
        let endpoint = format!("/applications/{}/edits/{}", package_name, edit_id);
        self.api_delete(&endpoint).await?;

        debug!("Deleted edit session: {}", edit_id);
        Ok(())
    }

    // ========================================================================
    // Listing methods
    // ========================================================================

    /// Get all listings (localizations) for an app.
    pub async fn get_listings(&self, package_name: &str, edit_id: &str) -> Result<Vec<Listing>> {
        let endpoint = format!("/applications/{}/edits/{}/listings", package_name, edit_id);

        let response: ListingsResponse = self.api_get(&endpoint).await?;
        Ok(response.listings.unwrap_or_default())
    }

    /// Get listing for a specific locale.
    pub async fn get_listing(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
    ) -> Result<Listing> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}",
            package_name, edit_id, locale
        );

        self.api_get(&endpoint).await
    }

    /// Update a listing for a specific locale.
    pub async fn update_listing(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
        listing: &ListingUpdate,
    ) -> Result<Listing> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}",
            package_name, edit_id, locale
        );

        self.api_put(&endpoint, listing).await
    }

    /// Delete a listing for a specific locale.
    pub async fn delete_listing(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
    ) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}",
            package_name, edit_id, locale
        );

        self.api_delete(&endpoint).await
    }

    // ========================================================================
    // Image methods
    // ========================================================================

    /// List images for a locale and image type.
    pub async fn list_images(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
        image_type: &str,
    ) -> Result<Vec<Image>> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}/{}",
            package_name, edit_id, locale, image_type
        );

        let response: ImagesResponse = self.api_get(&endpoint).await?;
        Ok(response.images.unwrap_or_default())
    }

    /// Upload an image.
    ///
    /// Note: This uses multipart upload which requires special handling.
    pub async fn upload_image(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
        image_type: &str,
        data: Vec<u8>,
        content_type: &str,
    ) -> Result<String> {
        let token = self.ensure_authenticated().await?;

        let url = format!(
            "https://androidpublisher.googleapis.com/upload/androidpublisher/v3/applications/{}/edits/{}/listings/{}/{}",
            package_name, edit_id, locale, image_type
        );

        debug!("Uploading image to: {}", url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", content_type)
            .body(data)
            .send()
            .await
            .map_err(|e| MetadataError::SyncError(format!("Image upload failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(MetadataError::SyncError(format!(
                "Image upload failed: {}",
                error_text
            )));
        }

        let upload_response: ImageUploadResponse = response.json().await.map_err(|e| {
            MetadataError::SyncError(format!("Failed to parse upload response: {}", e))
        })?;

        Ok(upload_response.image.id)
    }

    /// Delete an image.
    pub async fn delete_image(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
        image_type: &str,
        image_id: &str,
    ) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}/{}/{}",
            package_name, edit_id, locale, image_type, image_id
        );

        self.api_delete(&endpoint).await
    }

    /// Delete all images of a specific type for a locale.
    pub async fn delete_all_images(
        &self,
        package_name: &str,
        edit_id: &str,
        locale: &str,
        image_type: &str,
    ) -> Result<()> {
        let endpoint = format!(
            "/applications/{}/edits/{}/listings/{}/{}",
            package_name, edit_id, locale, image_type
        );

        self.api_delete(&endpoint).await
    }

    // ========================================================================
    // Helper methods
    // ========================================================================

    /// Convert a Google Play locale string to our Locale type.
    fn parse_locale(locale_str: &str) -> Result<Locale> {
        // Google Play uses formats like "en-US", "de-DE", etc.
        Locale::new(locale_str)
    }

    /// Convert a Listing to GooglePlayLocalizedMetadata.
    fn listing_to_local_metadata(&self, listing: &Listing) -> GooglePlayLocalizedMetadata {
        GooglePlayLocalizedMetadata {
            title: listing.title.clone().unwrap_or_default(),
            short_description: listing.short_description.clone().unwrap_or_default(),
            full_description: listing.full_description.clone().unwrap_or_default(),
            video_url: listing.video.clone(),
            changelogs: HashMap::new(), // Changelogs are handled separately
        }
    }

    /// Compare two optional strings and determine if they differ.
    fn strings_differ(local: Option<&str>, remote: Option<&str>) -> bool {
        match (local, remote) {
            (Some(l), Some(r)) => l.trim() != r.trim(),
            (Some(l), None) => !l.trim().is_empty(),
            (None, Some(r)) => !r.trim().is_empty(),
            (None, None) => false,
        }
    }

    /// Compare two strings and determine if they differ.
    fn strings_differ_required(local: &str, remote: &str) -> bool {
        local.trim() != remote.trim()
    }
}

#[async_trait]
impl MetadataSync for GooglePlayMetadataSync {
    async fn pull(&self, app_id: &str, locales: Option<&[Locale]>) -> Result<()> {
        info!("Pulling metadata for {} from Google Play Console", app_id);

        // Create an edit to read current state
        let edit_id = self.create_edit(app_id).await?;

        // Use a closure to ensure we clean up the edit on error
        let result = async {
            // Get all listings
            let listings = self.get_listings(app_id, &edit_id).await?;

            // Build metadata structure
            let mut metadata = GooglePlayMetadata::new(app_id);

            // Process each listing
            for listing in &listings {
                let locale_str = &listing.language;

                // Filter by requested locales if specified
                if let Some(filter_locales) = locales {
                    let locale = Self::parse_locale(locale_str)?;
                    if !filter_locales.iter().any(|l| l.code() == locale.code()) {
                        continue;
                    }
                }

                let local_metadata = self.listing_to_local_metadata(listing);
                metadata
                    .localizations
                    .insert(locale_str.clone(), local_metadata);

                debug!("Pulled metadata for locale: {}", locale_str);
            }

            // Set default locale (first one or en-US if available)
            if metadata.localizations.contains_key("en-US") {
                metadata.default_locale = Locale::new("en-US")?;
            } else if let Some(locale_str) = metadata.localizations.keys().next() {
                metadata.default_locale = Self::parse_locale(locale_str)?;
            }

            // Save to local storage
            self.storage.save_google_play(&metadata).await?;

            info!(
                "Successfully pulled metadata for {} locales",
                metadata.localizations.len()
            );

            Ok::<(), MetadataError>(())
        }
        .await;

        // Delete the edit (we don't need to commit since we're only reading)
        if let Err(e) = self.delete_edit(app_id, &edit_id).await {
            warn!("Failed to delete edit session: {}", e);
        }

        result
    }

    async fn push(
        &self,
        app_id: &str,
        locales: Option<&[Locale]>,
        dry_run: bool,
    ) -> Result<PushResult> {
        info!(
            "Pushing metadata for {} to Google Play Console{}",
            app_id,
            if dry_run { " (dry run)" } else { "" }
        );

        let mut result = PushResult::default();

        // Load local metadata
        let local_metadata = self.storage.load_google_play(app_id).await?;

        if dry_run {
            // For dry run, just compare what would be pushed
            let edit_id = self.create_edit(app_id).await?;

            let compare_result = async {
                let remote_listings = self.get_listings(app_id, &edit_id).await?;
                let remote_map: HashMap<String, &Listing> = remote_listings
                    .iter()
                    .map(|l| (l.language.clone(), l))
                    .collect();

                for (locale_str, local_loc) in &local_metadata.localizations {
                    if let Some(filter_locales) = locales {
                        let locale = Self::parse_locale(locale_str)?;
                        if !filter_locales.iter().any(|l| l.code() == locale.code()) {
                            continue;
                        }
                    }

                    if let Some(remote) = remote_map.get(locale_str) {
                        let title_differs = Self::strings_differ_required(
                            &local_loc.title,
                            remote.title.as_deref().unwrap_or(""),
                        );
                        let short_desc_differs = Self::strings_differ_required(
                            &local_loc.short_description,
                            remote.short_description.as_deref().unwrap_or(""),
                        );
                        let full_desc_differs = Self::strings_differ_required(
                            &local_loc.full_description,
                            remote.full_description.as_deref().unwrap_or(""),
                        );

                        if title_differs || short_desc_differs || full_desc_differs {
                            result.updated_locales.push(locale_str.clone());
                            if title_differs {
                                result.updated_fields.push(format!("{}/title", locale_str));
                            }
                            if short_desc_differs {
                                result
                                    .updated_fields
                                    .push(format!("{}/short_description", locale_str));
                            }
                            if full_desc_differs {
                                result
                                    .updated_fields
                                    .push(format!("{}/full_description", locale_str));
                            }
                        }
                    } else {
                        // New locale
                        result.updated_locales.push(locale_str.clone());
                    }
                }

                Ok::<(), MetadataError>(())
            }
            .await;

            // Clean up edit
            if let Err(e) = self.delete_edit(app_id, &edit_id).await {
                warn!("Failed to delete edit session: {}", e);
            }

            compare_result?;

            info!("[DRY RUN] Would have pushed: {}", result);
            return Ok(result);
        }

        // Create an edit for making changes
        let edit_id = self.create_edit(app_id).await?;

        let push_result = async {
            // Get current remote listings for comparison
            let remote_listings = self.get_listings(app_id, &edit_id).await?;
            let remote_map: HashMap<String, &Listing> = remote_listings
                .iter()
                .map(|l| (l.language.clone(), l))
                .collect();

            // Process each local localization
            for (locale_str, local_loc) in &local_metadata.localizations {
                // Filter by requested locales if specified
                if let Some(filter_locales) = locales {
                    let locale = Self::parse_locale(locale_str)?;
                    if !filter_locales.iter().any(|l| l.code() == locale.code()) {
                        continue;
                    }
                }

                let listing_update = ListingUpdate {
                    language: locale_str.clone(),
                    title: local_loc.title.clone(),
                    full_description: local_loc.full_description.clone(),
                    short_description: local_loc.short_description.clone(),
                    video: local_loc.video_url.clone(),
                };

                // Check if we need to update
                let needs_update = if let Some(remote) = remote_map.get(locale_str) {
                    Self::strings_differ_required(
                        &local_loc.title,
                        remote.title.as_deref().unwrap_or(""),
                    ) || Self::strings_differ_required(
                        &local_loc.short_description,
                        remote.short_description.as_deref().unwrap_or(""),
                    ) || Self::strings_differ_required(
                        &local_loc.full_description,
                        remote.full_description.as_deref().unwrap_or(""),
                    ) || Self::strings_differ(
                        local_loc.video_url.as_deref(),
                        remote.video.as_deref(),
                    )
                } else {
                    true // New locale, always update
                };

                if needs_update {
                    self.update_listing(app_id, &edit_id, locale_str, &listing_update)
                        .await?;
                    result.updated_locales.push(locale_str.clone());
                    debug!("Updated listing for {}", locale_str);
                }
            }

            // Commit the edit
            self.commit_edit(app_id, &edit_id).await?;

            Ok::<(), MetadataError>(())
        }
        .await;

        // If push failed, try to delete the edit
        if push_result.is_err() {
            if let Err(e) = self.delete_edit(app_id, &edit_id).await {
                warn!("Failed to delete edit session after error: {}", e);
            }
        }

        push_result?;

        info!("Pushed metadata: {}", result);

        Ok(result)
    }

    async fn diff(&self, app_id: &str) -> Result<MetadataDiff> {
        info!("Comparing metadata for {} with Google Play Console", app_id);

        let mut diff = MetadataDiff::default();

        // Load local metadata
        let local_metadata = self.storage.load_google_play(app_id).await?;

        // Create an edit to read current remote state
        let edit_id = self.create_edit(app_id).await?;

        let diff_result = async {
            // Get remote listings
            let remote_listings = self.get_listings(app_id, &edit_id).await?;
            let remote_map: HashMap<String, &Listing> = remote_listings
                .iter()
                .map(|l| (l.language.clone(), l))
                .collect();

            // Check for changes in local locales
            for (locale_str, local_loc) in &local_metadata.localizations {
                if let Some(remote) = remote_map.get(locale_str) {
                    // Compare title
                    let remote_title = remote.title.as_deref().unwrap_or("");
                    if Self::strings_differ_required(&local_loc.title, remote_title) {
                        diff.changes.push(MetadataChange {
                            locale: locale_str.clone(),
                            field: "title".to_string(),
                            local_value: Some(local_loc.title.clone()),
                            remote_value: Some(remote_title.to_string()),
                            change_type: ChangeType::Modified,
                        });
                    }

                    // Compare short_description
                    let remote_short = remote.short_description.as_deref().unwrap_or("");
                    if Self::strings_differ_required(&local_loc.short_description, remote_short) {
                        diff.changes.push(MetadataChange {
                            locale: locale_str.clone(),
                            field: "short_description".to_string(),
                            local_value: Some(local_loc.short_description.clone()),
                            remote_value: Some(remote_short.to_string()),
                            change_type: ChangeType::Modified,
                        });
                    }

                    // Compare full_description
                    let remote_full = remote.full_description.as_deref().unwrap_or("");
                    if Self::strings_differ_required(&local_loc.full_description, remote_full) {
                        diff.changes.push(MetadataChange {
                            locale: locale_str.clone(),
                            field: "full_description".to_string(),
                            local_value: Some(local_loc.full_description.clone()),
                            remote_value: Some(remote_full.to_string()),
                            change_type: ChangeType::Modified,
                        });
                    }

                    // Compare video URL
                    if Self::strings_differ(local_loc.video_url.as_deref(), remote.video.as_deref())
                    {
                        diff.changes.push(MetadataChange {
                            locale: locale_str.clone(),
                            field: "video_url".to_string(),
                            local_value: local_loc.video_url.clone(),
                            remote_value: remote.video.clone(),
                            change_type: if local_loc.video_url.is_some() && remote.video.is_none()
                            {
                                ChangeType::Added
                            } else if local_loc.video_url.is_none() && remote.video.is_some() {
                                ChangeType::Removed
                            } else {
                                ChangeType::Modified
                            },
                        });
                    }
                } else {
                    // Locale exists locally but not remotely
                    diff.changes.push(MetadataChange::added(
                        locale_str,
                        "locale",
                        format!("New locale with title: {}", local_loc.title),
                    ));
                }
            }

            // Check for locales that exist remotely but not locally
            for locale_str in remote_map.keys() {
                if !local_metadata.localizations.contains_key(locale_str) {
                    diff.changes.push(MetadataChange::removed(
                        locale_str,
                        "locale",
                        "Remote locale not present locally".to_string(),
                    ));
                }
            }

            Ok::<(), MetadataError>(())
        }
        .await;

        // Clean up the edit
        if let Err(e) = self.delete_edit(app_id, &edit_id).await {
            warn!("Failed to delete edit session: {}", e);
        }

        diff_result?;

        info!("Found {} differences", diff.len());

        Ok(diff)
    }
}

// ============================================================================
// Service Account Types
// ============================================================================

/// Google Cloud service account key structure.
#[derive(Debug, Clone, Deserialize)]
struct ServiceAccountKey {
    /// Service account email.
    client_email: String,
    /// RSA private key in PEM format.
    private_key: String,
    /// Token URI (usually https://oauth2.googleapis.com/token).
    token_uri: Option<String>,
    /// Project ID (optional, for reference).
    #[allow(dead_code)]
    project_id: Option<String>,
}

/// JWT claims for Google OAuth 2.0 service account authentication.
#[derive(Debug, Serialize)]
struct GoogleJwtClaims {
    /// Issuer (service account email).
    iss: String,
    /// Scope (API access requested).
    scope: String,
    /// Audience (token endpoint).
    aud: String,
    /// Issued at (timestamp).
    iat: i64,
    /// Expiration (timestamp).
    exp: i64,
}

/// OAuth 2.0 token response.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    /// The access token.
    access_token: String,
    /// Token type (usually "Bearer").
    #[allow(dead_code)]
    token_type: String,
    /// Token expiration in seconds.
    expires_in: u64,
}

// ============================================================================
// API Response Types
// ============================================================================

/// Edit session response.
#[derive(Debug, Deserialize)]
struct EditResponse {
    /// Edit ID.
    id: String,
    /// Expiry time (optional).
    #[allow(dead_code)]
    #[serde(rename = "expiryTimeSeconds")]
    expiry_time_seconds: Option<String>,
}

/// List of listings response.
#[derive(Debug, Deserialize)]
struct ListingsResponse {
    /// The listings.
    listings: Option<Vec<Listing>>,
}

/// A single listing (localization) for an app.
#[derive(Debug, Clone, Deserialize)]
pub struct Listing {
    /// BCP 47 language tag (e.g., "en-US").
    pub language: String,
    /// App title.
    pub title: Option<String>,
    /// Full description.
    #[serde(rename = "fullDescription")]
    pub full_description: Option<String>,
    /// Short description.
    #[serde(rename = "shortDescription")]
    pub short_description: Option<String>,
    /// YouTube video URL.
    pub video: Option<String>,
}

/// Listing update request body.
#[derive(Debug, Clone, Serialize)]
pub struct ListingUpdate {
    /// BCP 47 language tag (e.g., "en-US").
    pub language: String,
    /// App title (required).
    pub title: String,
    /// Full description (required).
    #[serde(rename = "fullDescription")]
    pub full_description: String,
    /// Short description (required).
    #[serde(rename = "shortDescription")]
    pub short_description: String,
    /// YouTube video URL (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video: Option<String>,
}

/// List of images response.
#[derive(Debug, Deserialize)]
struct ImagesResponse {
    /// The images.
    images: Option<Vec<Image>>,
}

/// A single image.
#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    /// Image ID.
    pub id: String,
    /// Image URL.
    pub url: Option<String>,
    /// SHA-256 hash of the image.
    pub sha256: Option<String>,
}

/// Image upload response.
#[derive(Debug, Deserialize)]
struct ImageUploadResponse {
    /// The uploaded image info.
    image: ImageInfo,
}

/// Image info from upload response.
#[derive(Debug, Deserialize)]
struct ImageInfo {
    /// Image ID.
    id: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strings_differ() {
        assert!(!GooglePlayMetadataSync::strings_differ(None, None));
        assert!(!GooglePlayMetadataSync::strings_differ(Some(""), None));
        assert!(!GooglePlayMetadataSync::strings_differ(None, Some("")));
        assert!(!GooglePlayMetadataSync::strings_differ(
            Some("hello"),
            Some("hello")
        ));
        assert!(!GooglePlayMetadataSync::strings_differ(
            Some(" hello "),
            Some("hello")
        ));

        assert!(GooglePlayMetadataSync::strings_differ(
            Some("hello"),
            Some("world")
        ));
        assert!(GooglePlayMetadataSync::strings_differ(Some("hello"), None));
        assert!(GooglePlayMetadataSync::strings_differ(None, Some("world")));
    }

    #[test]
    fn test_strings_differ_required() {
        assert!(!GooglePlayMetadataSync::strings_differ_required(
            "hello", "hello"
        ));
        assert!(!GooglePlayMetadataSync::strings_differ_required(
            " hello ", "hello"
        ));
        assert!(GooglePlayMetadataSync::strings_differ_required(
            "hello", "world"
        ));
    }

    #[test]
    fn test_listing_update_serialization() {
        let update = ListingUpdate {
            language: "en-US".to_string(),
            title: "My App".to_string(),
            full_description: "A great app".to_string(),
            short_description: "Great".to_string(),
            video: None,
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"language\":\"en-US\""));
        assert!(json.contains("\"title\":\"My App\""));
        assert!(json.contains("\"fullDescription\":\"A great app\""));
        assert!(json.contains("\"shortDescription\":\"Great\""));
        assert!(!json.contains("video")); // Should be skipped when None
    }

    #[test]
    fn test_listing_update_with_video() {
        let update = ListingUpdate {
            language: "en-US".to_string(),
            title: "My App".to_string(),
            full_description: "A great app".to_string(),
            short_description: "Great".to_string(),
            video: Some("https://youtube.com/watch?v=abc123".to_string()),
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("video"));
        assert!(json.contains("youtube.com"));
    }

    #[test]
    fn test_config_from_key_file() {
        let config = GooglePlaySyncConfig::from_key_file("/path/to/key.json");
        assert!(config.service_account_key_path.is_some());
        assert!(config.service_account_key_json.is_none());
    }

    #[test]
    fn test_config_from_key_json() {
        let json = r#"{"client_email": "test@example.com", "private_key": "..."}"#;
        let config = GooglePlaySyncConfig::from_key_json(json);
        assert!(config.service_account_key_path.is_none());
        assert!(config.service_account_key_json.is_some());
    }

    #[test]
    fn test_listing_deserialization() {
        let json = r#"{
            "language": "en-US",
            "title": "My App",
            "fullDescription": "A full description",
            "shortDescription": "Short desc"
        }"#;

        let listing: Listing = serde_json::from_str(json).unwrap();
        assert_eq!(listing.language, "en-US");
        assert_eq!(listing.title, Some("My App".to_string()));
        assert_eq!(
            listing.full_description,
            Some("A full description".to_string())
        );
        assert_eq!(listing.short_description, Some("Short desc".to_string()));
        assert!(listing.video.is_none());
    }

    #[test]
    fn test_edit_response_deserialization() {
        let json = r#"{"id": "abc123", "expiryTimeSeconds": "1234567890"}"#;

        let response: EditResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, "abc123");
    }
}
