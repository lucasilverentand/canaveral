//! Apple App Store Connect metadata synchronization.
//!
//! This module provides integration with App Store Connect API v2 for
//! syncing app metadata between local storage and the App Store.

use super::{MetadataChange, MetadataDiff, MetadataSync, PushResult};
use crate::{
    AppleLocalizedMetadata, AppleMetadata, FastlaneStorage, Locale, MetadataError,
    MetadataStorage, Result,
};
use async_trait::async_trait;
use chrono::{Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::{Client, Method, StatusCode};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, info, warn};

/// Base URL for App Store Connect API v1.
const API_BASE_URL: &str = "https://api.appstoreconnect.apple.com/v1";

/// Default retry delay in milliseconds.
const RETRY_DELAY_MS: u64 = 1000;

/// Maximum number of retries for rate-limited requests.
const MAX_RETRIES: u32 = 3;

/// Configuration for Apple App Store Connect sync.
#[derive(Debug, Clone)]
pub struct AppleSyncConfig {
    /// The API Key ID (from App Store Connect).
    pub api_key_id: String,
    /// The API Issuer ID (from App Store Connect).
    pub api_issuer_id: String,
    /// The private key content in PEM format.
    pub api_private_key: String,
    /// Optional team ID for filtering apps.
    pub team_id: Option<String>,
}

impl AppleSyncConfig {
    /// Creates a new config from environment variables.
    ///
    /// Looks for:
    /// - `APP_STORE_CONNECT_API_KEY_ID`
    /// - `APP_STORE_CONNECT_ISSUER_ID`
    /// - `APP_STORE_CONNECT_API_KEY` (the PEM content or path to .p8 file)
    /// - `APP_STORE_CONNECT_TEAM_ID` (optional)
    pub fn from_env() -> Result<Self> {
        let api_key_id = std::env::var("APP_STORE_CONNECT_API_KEY_ID").map_err(|_| {
            MetadataError::SyncError("APP_STORE_CONNECT_API_KEY_ID not set".to_string())
        })?;

        let api_issuer_id = std::env::var("APP_STORE_CONNECT_ISSUER_ID").map_err(|_| {
            MetadataError::SyncError("APP_STORE_CONNECT_ISSUER_ID not set".to_string())
        })?;

        let api_key_env = std::env::var("APP_STORE_CONNECT_API_KEY").map_err(|_| {
            MetadataError::SyncError("APP_STORE_CONNECT_API_KEY not set".to_string())
        })?;

        // Check if it's a path to a file or the actual key content
        let api_private_key = if Path::new(&api_key_env).exists() {
            std::fs::read_to_string(&api_key_env).map_err(|e| {
                MetadataError::SyncError(format!("Failed to read API key file: {}", e))
            })?
        } else {
            api_key_env
        };

        let team_id = std::env::var("APP_STORE_CONNECT_TEAM_ID").ok();

        Ok(Self {
            api_key_id,
            api_issuer_id,
            api_private_key,
            team_id,
        })
    }
}

/// Apple App Store Connect metadata sync client.
///
/// Provides methods for pulling and pushing metadata to/from App Store Connect.
pub struct AppleMetadataSync {
    /// Configuration for API authentication.
    config: AppleSyncConfig,
    /// Local storage backend.
    storage: FastlaneStorage,
    /// HTTP client.
    client: Client,
    /// Cached JWT token.
    jwt_cache: RwLock<Option<JwtCache>>,
}

/// Cached JWT token with expiration.
struct JwtCache {
    token: String,
    expires_at: chrono::DateTime<Utc>,
}

impl AppleMetadataSync {
    /// Creates a new Apple metadata sync client.
    ///
    /// # Arguments
    ///
    /// * `config` - App Store Connect API configuration
    /// * `storage_path` - Base path for local metadata storage
    pub async fn new(config: AppleSyncConfig, storage_path: PathBuf) -> Result<Self> {
        let storage = FastlaneStorage::new(storage_path);

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| MetadataError::SyncError(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            config,
            storage,
            client,
            jwt_cache: RwLock::new(None),
        })
    }

    /// Generate a JWT token for App Store Connect API authentication.
    fn generate_jwt(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.jwt_cache.read().unwrap();
            if let Some(ref cached) = *cache {
                // Return cached token if it's still valid (with 5 minute buffer)
                if Utc::now() < cached.expires_at - Duration::minutes(5) {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Generate new token
        let now = Utc::now();
        let exp = now + Duration::minutes(20); // Token valid for 20 minutes

        let claims = JwtClaims {
            iss: self.config.api_issuer_id.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            aud: "appstoreconnect-v1".to_string(),
        };

        let encoding_key = EncodingKey::from_ec_pem(self.config.api_private_key.as_bytes())
            .map_err(|e| MetadataError::SyncError(format!("Invalid API key: {}", e)))?;

        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.config.api_key_id.clone());

        let token = encode(&header, &claims, &encoding_key)
            .map_err(|e| MetadataError::SyncError(format!("Failed to generate JWT: {}", e)))?;

        // Cache the token
        {
            let mut cache = self.jwt_cache.write().unwrap();
            *cache = Some(JwtCache {
                token: token.clone(),
                expires_at: exp,
            });
        }

        Ok(token)
    }

    /// Make an authenticated API request with retry logic.
    async fn api_request<T: serde::de::DeserializeOwned>(
        &self,
        method: Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        let mut retries = 0;

        loop {
            let token = self.generate_jwt()?;
            let url = format!("{}{}", API_BASE_URL, endpoint);

            debug!("API request: {} {}", method, url);

            let mut request = self
                .client
                .request(method.clone(), &url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json");

            if let Some(ref body) = body {
                request = request.json(body);
            }

            let response = request.send().await.map_err(|e| {
                MetadataError::SyncError(format!("API request failed: {}", e))
            })?;

            let status = response.status();

            // Handle rate limiting
            if status == StatusCode::TOO_MANY_REQUESTS {
                if retries >= MAX_RETRIES {
                    return Err(MetadataError::SyncError(
                        "Rate limited: too many requests".to_string(),
                    ));
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

    /// Make a PATCH request to update a resource.
    async fn api_patch(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<()> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        debug!("API PATCH request: {}", url);

        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| MetadataError::SyncError(format!("API request failed: {}", e)))?;

        let status = response.status();

        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(MetadataError::SyncError(format!(
                "API error ({}): {}",
                status, error_text
            )));
        }

        Ok(())
    }

    /// Make a POST request to create a resource.
    async fn api_post<T: serde::de::DeserializeOwned>(
        &self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<T> {
        self.api_request(Method::POST, endpoint, Some(body)).await
    }

    /// Get the App Store Connect app ID from a bundle ID.
    pub async fn get_app_id(&self, bundle_id: &str) -> Result<String> {
        let endpoint = format!("/apps?filter[bundleId]={}", bundle_id);
        let response: AppsResponse = self.api_request(Method::GET, &endpoint, None).await?;

        response
            .data
            .first()
            .map(|app| app.id.clone())
            .ok_or_else(|| MetadataError::NotFound(format!("App not found: {}", bundle_id)))
    }

    /// Get App Store versions for an app.
    async fn get_app_store_versions(&self, app_id: &str) -> Result<Vec<AppStoreVersion>> {
        let endpoint = format!(
            "/apps/{}/appStoreVersions?filter[platform]=IOS&filter[appStoreState]=PREPARE_FOR_SUBMISSION,READY_FOR_REVIEW,WAITING_FOR_REVIEW,IN_REVIEW,DEVELOPER_REJECTED,PENDING_DEVELOPER_RELEASE,READY_FOR_SALE",
            app_id
        );
        let response: AppStoreVersionsResponse =
            self.api_request(Method::GET, &endpoint, None).await?;

        Ok(response.data)
    }

    /// Get the editable (draft) App Store version for an app.
    async fn get_editable_version(&self, app_id: &str) -> Result<AppStoreVersion> {
        let versions = self.get_app_store_versions(app_id).await?;

        // Find the version that's in an editable state
        let editable_states = [
            "PREPARE_FOR_SUBMISSION",
            "DEVELOPER_REJECTED",
            "REJECTED",
        ];

        versions
            .into_iter()
            .find(|v| editable_states.contains(&v.attributes.app_store_state.as_str()))
            .ok_or_else(|| {
                MetadataError::SyncError(
                    "No editable App Store version found. Create a new version first.".to_string(),
                )
            })
    }

    /// Get localizations for an App Store version.
    async fn get_version_localizations(
        &self,
        version_id: &str,
    ) -> Result<Vec<AppStoreVersionLocalization>> {
        let endpoint = format!(
            "/appStoreVersions/{}/appStoreVersionLocalizations",
            version_id
        );
        let response: LocalizationsResponse =
            self.api_request(Method::GET, &endpoint, None).await?;

        Ok(response.data)
    }

    /// Get app info localizations (name, subtitle, privacy text).
    async fn get_app_info_localizations(&self, app_id: &str) -> Result<Vec<AppInfoLocalization>> {
        // First get the current app info
        let endpoint = format!("/apps/{}/appInfos", app_id);
        let response: AppInfosResponse = self.api_request(Method::GET, &endpoint, None).await?;

        let app_info = response.data.first().ok_or_else(|| {
            MetadataError::SyncError("No app info found".to_string())
        })?;

        // Then get localizations for that app info
        let endpoint = format!("/appInfos/{}/appInfoLocalizations", app_info.id);
        let response: AppInfoLocalizationsResponse =
            self.api_request(Method::GET, &endpoint, None).await?;

        Ok(response.data)
    }

    /// Update an App Store version localization.
    async fn update_version_localization(
        &self,
        localization_id: &str,
        update: &LocalizationUpdate,
    ) -> Result<()> {
        let endpoint = format!("/appStoreVersionLocalizations/{}", localization_id);

        let body = serde_json::json!({
            "data": {
                "type": "appStoreVersionLocalizations",
                "id": localization_id,
                "attributes": update
            }
        });

        self.api_patch(&endpoint, body).await
    }

    /// Create a new App Store version localization.
    async fn create_version_localization(
        &self,
        version_id: &str,
        locale: &str,
        update: &LocalizationUpdate,
    ) -> Result<String> {
        let endpoint = "/appStoreVersionLocalizations";

        let body = serde_json::json!({
            "data": {
                "type": "appStoreVersionLocalizations",
                "attributes": {
                    "locale": locale,
                    "description": update.description,
                    "keywords": update.keywords,
                    "whatsNew": update.whats_new,
                    "promotionalText": update.promotional_text,
                    "marketingUrl": update.marketing_url,
                    "supportUrl": update.support_url
                },
                "relationships": {
                    "appStoreVersion": {
                        "data": {
                            "type": "appStoreVersions",
                            "id": version_id
                        }
                    }
                }
            }
        });

        let response: LocalizationCreateResponse = self.api_post(endpoint, body).await?;
        Ok(response.data.id)
    }

    /// Update an app info localization (name, subtitle).
    async fn update_app_info_localization(
        &self,
        localization_id: &str,
        name: Option<&str>,
        subtitle: Option<&str>,
    ) -> Result<()> {
        let endpoint = format!("/appInfoLocalizations/{}", localization_id);

        let mut attributes = serde_json::Map::new();
        if let Some(name) = name {
            attributes.insert("name".to_string(), serde_json::Value::String(name.to_string()));
        }
        if let Some(subtitle) = subtitle {
            attributes.insert("subtitle".to_string(), serde_json::Value::String(subtitle.to_string()));
        }

        let body = serde_json::json!({
            "data": {
                "type": "appInfoLocalizations",
                "id": localization_id,
                "attributes": attributes
            }
        });

        self.api_patch(&endpoint, body).await
    }

    /// Convert App Store Connect locale to our Locale type.
    fn parse_locale(locale_str: &str) -> Result<Locale> {
        // App Store Connect uses formats like "en-US", "de-DE", etc.
        Locale::new(locale_str)
    }

    /// Convert remote localizations to AppleLocalizedMetadata.
    fn convert_to_local_metadata(
        &self,
        version_loc: &AppStoreVersionLocalization,
        app_info_loc: Option<&AppInfoLocalization>,
    ) -> AppleLocalizedMetadata {
        let attrs = &version_loc.attributes;

        AppleLocalizedMetadata {
            name: app_info_loc
                .map(|l| l.attributes.name.clone().unwrap_or_default())
                .unwrap_or_default(),
            subtitle: app_info_loc.and_then(|l| l.attributes.subtitle.clone()),
            description: attrs.description.clone().unwrap_or_default(),
            keywords: attrs.keywords.clone(),
            whats_new: attrs.whats_new.clone(),
            promotional_text: attrs.promotional_text.clone(),
            support_url: attrs.support_url.clone(),
            marketing_url: attrs.marketing_url.clone(),
            privacy_policy_url: None, // This is at app level, not locale level
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
}

#[async_trait]
impl MetadataSync for AppleMetadataSync {
    async fn pull(&self, app_id: &str, locales: Option<&[Locale]>) -> Result<()> {
        info!("Pulling metadata for {} from App Store Connect", app_id);

        // Get the App Store Connect app ID
        let asc_app_id = self.get_app_id(app_id).await?;

        // Get the current/editable version
        let versions = self.get_app_store_versions(&asc_app_id).await?;
        let version = versions.first().ok_or_else(|| {
            MetadataError::SyncError("No App Store versions found".to_string())
        })?;

        // Get version localizations
        let version_locs = self.get_version_localizations(&version.id).await?;

        // Get app info localizations
        let app_info_locs = self.get_app_info_localizations(&asc_app_id).await?;

        // Create a map of app info localizations by locale
        let app_info_map: HashMap<String, &AppInfoLocalization> = app_info_locs
            .iter()
            .map(|l| (l.attributes.locale.clone(), l))
            .collect();

        // Build metadata structure
        let mut metadata = AppleMetadata::new(app_id);

        // Process each localization
        for version_loc in &version_locs {
            let locale_str = &version_loc.attributes.locale;

            // Filter by requested locales if specified
            if let Some(filter_locales) = locales {
                let locale = Self::parse_locale(locale_str)?;
                if !filter_locales.iter().any(|l| l.code() == locale.code()) {
                    continue;
                }
            }

            let app_info_loc = app_info_map.get(locale_str).copied();
            let local_metadata = self.convert_to_local_metadata(version_loc, app_info_loc);

            metadata.localizations.insert(locale_str.clone(), local_metadata);

            debug!("Pulled metadata for locale: {}", locale_str);
        }

        // Set primary locale (first one or en-US if available)
        if let Some(locale_str) = metadata.localizations.keys().next() {
            metadata.primary_locale = Self::parse_locale(locale_str)?;
        }

        // Save to local storage
        self.storage.save_apple(&metadata).await?;

        info!(
            "Successfully pulled metadata for {} locales",
            metadata.localizations.len()
        );

        Ok(())
    }

    async fn push(
        &self,
        app_id: &str,
        locales: Option<&[Locale]>,
        dry_run: bool,
    ) -> Result<PushResult> {
        info!(
            "Pushing metadata for {} to App Store Connect{}",
            app_id,
            if dry_run { " (dry run)" } else { "" }
        );

        let mut result = PushResult::default();

        // Load local metadata
        let local_metadata = self.storage.load_apple(app_id).await?;

        // Get the App Store Connect app ID
        let asc_app_id = self.get_app_id(app_id).await?;

        // Get the editable version
        let version = self.get_editable_version(&asc_app_id).await?;

        // Get current remote localizations
        let version_locs = self.get_version_localizations(&version.id).await?;
        let app_info_locs = self.get_app_info_localizations(&asc_app_id).await?;

        // Create maps for easy lookup
        let version_loc_map: HashMap<String, &AppStoreVersionLocalization> = version_locs
            .iter()
            .map(|l| (l.attributes.locale.clone(), l))
            .collect();

        let app_info_loc_map: HashMap<String, &AppInfoLocalization> = app_info_locs
            .iter()
            .map(|l| (l.attributes.locale.clone(), l))
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

            let update = LocalizationUpdate {
                description: Some(local_loc.description.clone()),
                keywords: local_loc.keywords.clone(),
                whats_new: local_loc.whats_new.clone(),
                promotional_text: local_loc.promotional_text.clone(),
                marketing_url: local_loc.marketing_url.clone(),
                support_url: local_loc.support_url.clone(),
            };

            if let Some(version_loc) = version_loc_map.get(locale_str) {
                // Update existing localization
                if !dry_run {
                    self.update_version_localization(&version_loc.id, &update).await?;
                }
                result.updated_locales.push(locale_str.clone());
                debug!("Updated version localization for {}", locale_str);
            } else {
                // Create new localization
                if !dry_run {
                    self.create_version_localization(&version.id, locale_str, &update).await?;
                }
                result.updated_locales.push(locale_str.clone());
                debug!("Created version localization for {}", locale_str);
            }

            // Update app info localization (name, subtitle) if it exists
            if let Some(app_info_loc) = app_info_loc_map.get(locale_str) {
                let name_changed = app_info_loc.attributes.name.as_deref() != Some(&local_loc.name);
                let subtitle_changed = app_info_loc.attributes.subtitle != local_loc.subtitle;

                if name_changed || subtitle_changed {
                    if !dry_run {
                        self.update_app_info_localization(
                            &app_info_loc.id,
                            if name_changed { Some(&local_loc.name) } else { None },
                            if subtitle_changed { local_loc.subtitle.as_deref() } else { None },
                        ).await?;
                    }

                    if name_changed {
                        result.updated_fields.push(format!("{}/name", locale_str));
                    }
                    if subtitle_changed {
                        result.updated_fields.push(format!("{}/subtitle", locale_str));
                    }
                }
            }
        }

        info!(
            "{}Pushed metadata: {}",
            if dry_run { "[DRY RUN] Would have " } else { "" },
            result
        );

        Ok(result)
    }

    async fn diff(&self, app_id: &str) -> Result<MetadataDiff> {
        info!("Comparing metadata for {} with App Store Connect", app_id);

        let mut diff = MetadataDiff::default();

        // Load local metadata
        let local_metadata = self.storage.load_apple(app_id).await?;

        // Get the App Store Connect app ID
        let asc_app_id = self.get_app_id(app_id).await?;

        // Get current remote version and localizations
        let versions = self.get_app_store_versions(&asc_app_id).await?;
        let version = versions.first().ok_or_else(|| {
            MetadataError::SyncError("No App Store versions found".to_string())
        })?;

        let version_locs = self.get_version_localizations(&version.id).await?;
        let app_info_locs = self.get_app_info_localizations(&asc_app_id).await?;

        // Create maps for easy lookup
        let version_loc_map: HashMap<String, &AppStoreVersionLocalization> = version_locs
            .iter()
            .map(|l| (l.attributes.locale.clone(), l))
            .collect();

        let app_info_loc_map: HashMap<String, &AppInfoLocalization> = app_info_locs
            .iter()
            .map(|l| (l.attributes.locale.clone(), l))
            .collect();

        // Check for changes in local locales
        for (locale_str, local_loc) in &local_metadata.localizations {
            if let Some(version_loc) = version_loc_map.get(locale_str) {
                let remote_attrs = &version_loc.attributes;

                // Compare description
                if Self::strings_differ(
                    Some(&local_loc.description),
                    remote_attrs.description.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "description",
                        local_loc.description.clone(),
                        remote_attrs.description.clone().unwrap_or_default(),
                    ));
                }

                // Compare keywords
                if Self::strings_differ(
                    local_loc.keywords.as_deref(),
                    remote_attrs.keywords.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "keywords",
                        local_loc.keywords.clone().unwrap_or_default(),
                        remote_attrs.keywords.clone().unwrap_or_default(),
                    ));
                }

                // Compare what's new
                if Self::strings_differ(
                    local_loc.whats_new.as_deref(),
                    remote_attrs.whats_new.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "whats_new",
                        local_loc.whats_new.clone().unwrap_or_default(),
                        remote_attrs.whats_new.clone().unwrap_or_default(),
                    ));
                }

                // Compare promotional text
                if Self::strings_differ(
                    local_loc.promotional_text.as_deref(),
                    remote_attrs.promotional_text.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "promotional_text",
                        local_loc.promotional_text.clone().unwrap_or_default(),
                        remote_attrs.promotional_text.clone().unwrap_or_default(),
                    ));
                }

                // Compare URLs
                if Self::strings_differ(
                    local_loc.support_url.as_deref(),
                    remote_attrs.support_url.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "support_url",
                        local_loc.support_url.clone().unwrap_or_default(),
                        remote_attrs.support_url.clone().unwrap_or_default(),
                    ));
                }

                if Self::strings_differ(
                    local_loc.marketing_url.as_deref(),
                    remote_attrs.marketing_url.as_deref(),
                ) {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "marketing_url",
                        local_loc.marketing_url.clone().unwrap_or_default(),
                        remote_attrs.marketing_url.clone().unwrap_or_default(),
                    ));
                }
            } else {
                // Locale exists locally but not remotely
                diff.changes.push(MetadataChange::added(
                    locale_str,
                    "locale",
                    format!("New locale with {} fields", 6),
                ));
            }

            // Check app info localization (name, subtitle)
            if let Some(app_info_loc) = app_info_loc_map.get(locale_str) {
                let remote_name = app_info_loc.attributes.name.as_deref().unwrap_or_default();
                if local_loc.name != remote_name {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "name",
                        local_loc.name.clone(),
                        remote_name.to_string(),
                    ));
                }

                if local_loc.subtitle != app_info_loc.attributes.subtitle {
                    diff.changes.push(MetadataChange::modified(
                        locale_str,
                        "subtitle",
                        local_loc.subtitle.clone().unwrap_or_default(),
                        app_info_loc.attributes.subtitle.clone().unwrap_or_default(),
                    ));
                }
            }
        }

        // Check for locales that exist remotely but not locally
        for locale_str in version_loc_map.keys() {
            if !local_metadata.localizations.contains_key(locale_str) {
                diff.changes.push(MetadataChange::removed(
                    locale_str,
                    "locale",
                    "Remote locale not present locally".to_string(),
                ));
            }
        }

        info!("Found {} differences", diff.len());

        Ok(diff)
    }
}

// ============================================================================
// JWT Claims
// ============================================================================

#[derive(Debug, Serialize)]
struct JwtClaims {
    iss: String,
    iat: i64,
    exp: i64,
    aud: String,
}

// ============================================================================
// API Request/Response Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct AppsResponse {
    data: Vec<AppData>,
}

#[derive(Debug, Deserialize)]
struct AppData {
    id: String,
    #[allow(dead_code)]
    attributes: AppAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppAttributes {
    #[allow(dead_code)]
    bundle_id: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct AppStoreVersionsResponse {
    data: Vec<AppStoreVersion>,
}

#[derive(Debug, Deserialize)]
pub struct AppStoreVersion {
    pub id: String,
    pub attributes: VersionAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VersionAttributes {
    #[allow(dead_code)]
    pub version_string: Option<String>,
    pub app_store_state: String,
    #[allow(dead_code)]
    pub platform: String,
}

#[derive(Debug, Deserialize)]
struct LocalizationsResponse {
    data: Vec<AppStoreVersionLocalization>,
}

#[derive(Debug, Deserialize)]
pub struct AppStoreVersionLocalization {
    pub id: String,
    pub attributes: LocalizationAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalizationAttributes {
    pub locale: String,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub whats_new: Option<String>,
    pub promotional_text: Option<String>,
    pub marketing_url: Option<String>,
    pub support_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LocalizationUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    keywords: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    whats_new: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    promotional_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    marketing_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    support_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LocalizationCreateResponse {
    data: LocalizationCreateData,
}

#[derive(Debug, Deserialize)]
struct LocalizationCreateData {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AppInfosResponse {
    data: Vec<AppInfo>,
}

#[derive(Debug, Deserialize)]
struct AppInfo {
    id: String,
}

#[derive(Debug, Deserialize)]
struct AppInfoLocalizationsResponse {
    data: Vec<AppInfoLocalization>,
}

#[derive(Debug, Deserialize)]
struct AppInfoLocalization {
    id: String,
    attributes: AppInfoLocalizationAttributes,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppInfoLocalizationAttributes {
    locale: String,
    name: Option<String>,
    subtitle: Option<String>,
    #[allow(dead_code)]
    privacy_policy_text: Option<String>,
    #[allow(dead_code)]
    privacy_policy_url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strings_differ() {
        assert!(!AppleMetadataSync::strings_differ(None, None));
        assert!(!AppleMetadataSync::strings_differ(Some(""), None));
        assert!(!AppleMetadataSync::strings_differ(None, Some("")));
        assert!(!AppleMetadataSync::strings_differ(Some("hello"), Some("hello")));
        assert!(!AppleMetadataSync::strings_differ(Some(" hello "), Some("hello")));

        assert!(AppleMetadataSync::strings_differ(Some("hello"), Some("world")));
        assert!(AppleMetadataSync::strings_differ(Some("hello"), None));
        assert!(AppleMetadataSync::strings_differ(None, Some("world")));
    }

    #[test]
    fn test_localization_update_serialization() {
        let update = LocalizationUpdate {
            description: Some("Test description".to_string()),
            keywords: None,
            whats_new: Some("Bug fixes".to_string()),
            promotional_text: None,
            marketing_url: None,
            support_url: Some("https://example.com".to_string()),
        };

        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("description"));
        assert!(json.contains("whatsNew"));
        assert!(json.contains("supportUrl"));
        assert!(!json.contains("keywords"));
        assert!(!json.contains("promotionalText"));
    }
}
