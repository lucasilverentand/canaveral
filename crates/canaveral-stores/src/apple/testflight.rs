//! TestFlight beta testing management
//!
//! Provides integration with TestFlight for beta distribution via the App Store Connect API.
//! Supports managing testers, groups, and build submissions.

use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, instrument};

use crate::error::{Result, StoreError};
use crate::types::AppleStoreConfig;

const API_BASE_URL: &str = "https://api.appstoreconnect.apple.com/v1";

/// JWT claims for App Store Connect API
#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
    aud: String,
}

/// TestFlight client for managing beta testing
pub struct TestFlight {
    config: AppleStoreConfig,
    client: Client,
    jwt_token: Option<String>,
    token_expires: Option<DateTime<Utc>>,
}

impl TestFlight {
    /// Create a new TestFlight client
    pub fn new(config: AppleStoreConfig) -> Self {
        Self {
            config,
            client: Client::new(),
            jwt_token: None,
            token_expires: None,
        }
    }

    /// Create from environment variables
    pub fn from_env() -> Result<Self> {
        let api_key_id = std::env::var("APP_STORE_CONNECT_API_KEY_ID").map_err(|_| {
            StoreError::ConfigurationError("APP_STORE_CONNECT_API_KEY_ID not set".to_string())
        })?;

        let api_issuer_id = std::env::var("APP_STORE_CONNECT_ISSUER_ID").map_err(|_| {
            StoreError::ConfigurationError("APP_STORE_CONNECT_ISSUER_ID not set".to_string())
        })?;

        let api_key = std::env::var("APP_STORE_CONNECT_API_KEY")
            .or_else(|_| std::env::var("APP_STORE_CONNECT_API_KEY_PATH"))
            .map_err(|_| {
                StoreError::ConfigurationError(
                    "APP_STORE_CONNECT_API_KEY or APP_STORE_CONNECT_API_KEY_PATH not set"
                        .to_string(),
                )
            })?;

        let team_id = std::env::var("APP_STORE_CONNECT_TEAM_ID").ok();

        Ok(Self::new(AppleStoreConfig {
            api_key_id,
            api_issuer_id,
            api_key,
            team_id,
            app_id: None,
            notarize: false,
            staple: false,
            primary_locale: None,
        }))
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
        let exp = now + Duration::minutes(20);

        let claims = Claims {
            iss: self.config.api_issuer_id.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            aud: "appstoreconnect-v1".to_string(),
        };

        // Read the private key
        let key_content = if Path::new(&self.config.api_key).exists() {
            std::fs::read_to_string(&self.config.api_key).map_err(|e| {
                StoreError::ConfigurationError(format!("Failed to read API key: {}", e))
            })?
        } else {
            self.config.api_key.clone()
        };

        let encoding_key = EncodingKey::from_ec_pem(key_content.as_bytes())
            .map_err(|e| StoreError::InvalidCredentials(format!("Invalid API key: {}", e)))?;

        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.config.api_key_id.clone());

        let token = encode(&header, &claims, &encoding_key)?;

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

        debug!("API request: {} {}", method, url);

        let mut request = self
            .client
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

    /// Make an API request that returns no content
    async fn api_request_no_content(
        &mut self,
        method: reqwest::Method,
        endpoint: &str,
        body: Option<serde_json::Value>,
    ) -> Result<()> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        debug!("API request (no content): {} {}", method, url);

        let mut request = self
            .client
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

        Ok(())
    }

    // -------------------------------------------------------------------------
    // App Management
    // -------------------------------------------------------------------------

    /// Get app ID by bundle identifier
    pub async fn get_app_id(&mut self, bundle_id: &str) -> Result<String> {
        #[derive(Deserialize)]
        struct AppsResponse {
            data: Vec<AppData>,
        }

        #[derive(Deserialize)]
        struct AppData {
            id: String,
        }

        let endpoint = format!("/apps?filter[bundleId]={}", bundle_id);
        let response: AppsResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        response
            .data
            .first()
            .map(|app| app.id.clone())
            .ok_or_else(|| StoreError::AppNotFound(bundle_id.to_string()))
    }

    // -------------------------------------------------------------------------
    // Build Management
    // -------------------------------------------------------------------------

    /// List TestFlight builds for an app
    #[instrument(skip(self), fields(app_id, limit))]
    pub async fn list_builds(
        &mut self,
        app_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<TestFlightBuild>> {
        #[derive(Deserialize)]
        struct BuildsResponse {
            data: Vec<BuildData>,
        }

        #[derive(Deserialize)]
        struct BuildData {
            id: String,
            attributes: BuildAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildAttributes {
            version: String,
            uploaded_date: Option<String>,
            expiration_date: Option<String>,
            expired: bool,
            processing_state: String,
            build_audience_type: Option<String>,
            uses_non_exempt_encryption: Option<bool>,
        }

        let limit = limit.unwrap_or(25);
        let endpoint = format!(
            "/builds?filter[app]={}&limit={}&sort=-uploadedDate",
            app_id, limit
        );

        let response: BuildsResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        let builds = response
            .data
            .into_iter()
            .map(|b| TestFlightBuild {
                id: b.id,
                version: b.attributes.version,
                uploaded_at: b
                    .attributes
                    .uploaded_date
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc)),
                expires_at: b
                    .attributes
                    .expiration_date
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc)),
                expired: b.attributes.expired,
                processing_state: BuildProcessingState::from_str(&b.attributes.processing_state),
                audience_type: b
                    .attributes
                    .build_audience_type
                    .map(|s| BuildAudienceType::from_str(&s))
                    .unwrap_or(BuildAudienceType::Internal),
                uses_non_exempt_encryption: b.attributes.uses_non_exempt_encryption,
            })
            .collect();

        Ok(builds)
    }

    /// Get build status by build ID
    #[instrument(skip(self), fields(build_id))]
    pub async fn get_build(&mut self, build_id: &str) -> Result<TestFlightBuild> {
        #[derive(Deserialize)]
        struct BuildResponse {
            data: BuildData,
        }

        #[derive(Deserialize)]
        struct BuildData {
            id: String,
            attributes: BuildAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct BuildAttributes {
            version: String,
            uploaded_date: Option<String>,
            expiration_date: Option<String>,
            expired: bool,
            processing_state: String,
            build_audience_type: Option<String>,
            uses_non_exempt_encryption: Option<bool>,
        }

        let endpoint = format!("/builds/{}", build_id);
        let response: BuildResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        let b = response.data;
        Ok(TestFlightBuild {
            id: b.id,
            version: b.attributes.version,
            uploaded_at: b
                .attributes
                .uploaded_date
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc)),
            expires_at: b
                .attributes
                .expiration_date
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc)),
            expired: b.attributes.expired,
            processing_state: BuildProcessingState::from_str(&b.attributes.processing_state),
            audience_type: b
                .attributes
                .build_audience_type
                .map(|s| BuildAudienceType::from_str(&s))
                .unwrap_or(BuildAudienceType::Internal),
            uses_non_exempt_encryption: b.attributes.uses_non_exempt_encryption,
        })
    }

    /// Set export compliance for a build
    pub async fn set_export_compliance(
        &mut self,
        build_id: &str,
        uses_encryption: bool,
    ) -> Result<()> {
        let body = serde_json::json!({
            "data": {
                "type": "builds",
                "id": build_id,
                "attributes": {
                    "usesNonExemptEncryption": uses_encryption
                }
            }
        });

        let endpoint = format!("/builds/{}", build_id);
        self.api_request_no_content(reqwest::Method::PATCH, &endpoint, Some(body))
            .await
    }

    /// Expire a build (remove from TestFlight)
    pub async fn expire_build(&mut self, build_id: &str) -> Result<()> {
        let body = serde_json::json!({
            "data": {
                "type": "builds",
                "id": build_id,
                "attributes": {
                    "expired": true
                }
            }
        });

        let endpoint = format!("/builds/{}", build_id);
        self.api_request_no_content(reqwest::Method::PATCH, &endpoint, Some(body))
            .await
    }

    // -------------------------------------------------------------------------
    // Beta Group Management
    // -------------------------------------------------------------------------

    /// List beta groups for an app
    pub async fn list_beta_groups(&mut self, app_id: &str) -> Result<Vec<BetaGroup>> {
        #[derive(Deserialize)]
        struct GroupsResponse {
            data: Vec<GroupData>,
        }

        #[derive(Deserialize)]
        struct GroupData {
            id: String,
            attributes: GroupAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        struct GroupAttributes {
            name: String,
            is_internal_group: bool,
            public_link_enabled: Option<bool>,
            public_link: Option<String>,
            public_link_limit_enabled: Option<bool>,
            public_link_limit: Option<u32>,
        }

        let endpoint = format!("/betaGroups?filter[app]={}", app_id);
        let response: GroupsResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        let groups = response
            .data
            .into_iter()
            .map(|g| BetaGroup {
                id: g.id,
                name: g.attributes.name,
                is_internal: g.attributes.is_internal_group,
                public_link_enabled: g.attributes.public_link_enabled.unwrap_or(false),
                public_link: g.attributes.public_link,
                public_link_limit: g.attributes.public_link_limit,
            })
            .collect();

        Ok(groups)
    }

    /// Create a new beta group
    pub async fn create_beta_group(
        &mut self,
        app_id: &str,
        name: &str,
        is_internal: bool,
    ) -> Result<BetaGroup> {
        #[derive(Deserialize)]
        struct CreateResponse {
            data: GroupData,
        }

        #[derive(Deserialize)]
        struct GroupData {
            id: String,
            attributes: GroupAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct GroupAttributes {
            name: String,
            is_internal_group: bool,
        }

        let body = serde_json::json!({
            "data": {
                "type": "betaGroups",
                "attributes": {
                    "name": name,
                    "isInternalGroup": is_internal
                },
                "relationships": {
                    "app": {
                        "data": {
                            "type": "apps",
                            "id": app_id
                        }
                    }
                }
            }
        });

        let response: CreateResponse = self
            .api_request(reqwest::Method::POST, "/betaGroups", Some(body))
            .await?;

        Ok(BetaGroup {
            id: response.data.id,
            name: response.data.attributes.name,
            is_internal: response.data.attributes.is_internal_group,
            public_link_enabled: false,
            public_link: None,
            public_link_limit: None,
        })
    }

    /// Delete a beta group
    pub async fn delete_beta_group(&mut self, group_id: &str) -> Result<()> {
        let endpoint = format!("/betaGroups/{}", group_id);
        self.api_request_no_content(reqwest::Method::DELETE, &endpoint, None)
            .await
    }

    /// Add builds to a beta group
    pub async fn add_builds_to_group(&mut self, group_id: &str, build_ids: &[&str]) -> Result<()> {
        let builds: Vec<_> = build_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "builds",
                    "id": id
                })
            })
            .collect();

        let body = serde_json::json!({
            "data": builds
        });

        let endpoint = format!("/betaGroups/{}/relationships/builds", group_id);
        self.api_request_no_content(reqwest::Method::POST, &endpoint, Some(body))
            .await
    }

    /// Remove builds from a beta group
    pub async fn remove_builds_from_group(
        &mut self,
        group_id: &str,
        build_ids: &[&str],
    ) -> Result<()> {
        let builds: Vec<_> = build_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "builds",
                    "id": id
                })
            })
            .collect();

        let body = serde_json::json!({
            "data": builds
        });

        let endpoint = format!("/betaGroups/{}/relationships/builds", group_id);
        self.api_request_no_content(reqwest::Method::DELETE, &endpoint, Some(body))
            .await
    }

    // -------------------------------------------------------------------------
    // Beta Tester Management
    // -------------------------------------------------------------------------

    /// List beta testers for an app
    pub async fn list_testers(
        &mut self,
        app_id: &str,
        group_id: Option<&str>,
    ) -> Result<Vec<BetaTester>> {
        #[derive(Deserialize)]
        struct TestersResponse {
            data: Vec<TesterData>,
        }

        #[derive(Deserialize)]
        struct TesterData {
            id: String,
            attributes: TesterAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TesterAttributes {
            first_name: Option<String>,
            last_name: Option<String>,
            email: String,
            invite_type: String,
        }

        let endpoint = if let Some(gid) = group_id {
            format!("/betaTesters?filter[betaGroups]={}", gid)
        } else {
            format!("/betaTesters?filter[apps]={}", app_id)
        };

        let response: TestersResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        let testers = response
            .data
            .into_iter()
            .map(|t| BetaTester {
                id: t.id,
                email: t.attributes.email,
                first_name: t.attributes.first_name,
                last_name: t.attributes.last_name,
                invite_type: TesterInviteType::from_str(&t.attributes.invite_type),
            })
            .collect();

        Ok(testers)
    }

    /// Invite a beta tester
    #[instrument(skip(self, first_name, last_name, group_ids), fields(email))]
    pub async fn invite_tester(
        &mut self,
        email: &str,
        first_name: Option<&str>,
        last_name: Option<&str>,
        group_ids: &[&str],
    ) -> Result<BetaTester> {
        #[derive(Deserialize)]
        struct CreateResponse {
            data: TesterData,
        }

        #[derive(Deserialize)]
        struct TesterData {
            id: String,
            attributes: TesterAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct TesterAttributes {
            first_name: Option<String>,
            last_name: Option<String>,
            email: String,
            invite_type: String,
        }

        let groups: Vec<_> = group_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "betaGroups",
                    "id": id
                })
            })
            .collect();

        let mut attributes = serde_json::json!({
            "email": email
        });

        if let Some(name) = first_name {
            attributes["firstName"] = serde_json::json!(name);
        }
        if let Some(name) = last_name {
            attributes["lastName"] = serde_json::json!(name);
        }

        let body = serde_json::json!({
            "data": {
                "type": "betaTesters",
                "attributes": attributes,
                "relationships": {
                    "betaGroups": {
                        "data": groups
                    }
                }
            }
        });

        let response: CreateResponse = self
            .api_request(reqwest::Method::POST, "/betaTesters", Some(body))
            .await?;

        Ok(BetaTester {
            id: response.data.id,
            email: response.data.attributes.email,
            first_name: response.data.attributes.first_name,
            last_name: response.data.attributes.last_name,
            invite_type: TesterInviteType::from_str(&response.data.attributes.invite_type),
        })
    }

    /// Remove a beta tester
    pub async fn remove_tester(&mut self, tester_id: &str) -> Result<()> {
        let endpoint = format!("/betaTesters/{}", tester_id);
        self.api_request_no_content(reqwest::Method::DELETE, &endpoint, None)
            .await
    }

    /// Add a tester to groups
    pub async fn add_tester_to_groups(
        &mut self,
        tester_id: &str,
        group_ids: &[&str],
    ) -> Result<()> {
        let groups: Vec<_> = group_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "betaGroups",
                    "id": id
                })
            })
            .collect();

        let body = serde_json::json!({
            "data": groups
        });

        let endpoint = format!("/betaTesters/{}/relationships/betaGroups", tester_id);
        self.api_request_no_content(reqwest::Method::POST, &endpoint, Some(body))
            .await
    }

    /// Remove a tester from groups
    pub async fn remove_tester_from_groups(
        &mut self,
        tester_id: &str,
        group_ids: &[&str],
    ) -> Result<()> {
        let groups: Vec<_> = group_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "type": "betaGroups",
                    "id": id
                })
            })
            .collect();

        let body = serde_json::json!({
            "data": groups
        });

        let endpoint = format!("/betaTesters/{}/relationships/betaGroups", tester_id);
        self.api_request_no_content(reqwest::Method::DELETE, &endpoint, Some(body))
            .await
    }

    // -------------------------------------------------------------------------
    // Beta App Review Submission
    // -------------------------------------------------------------------------

    /// Submit a build for beta review (external testing)
    #[instrument(skip(self), fields(build_id))]
    pub async fn submit_for_beta_review(
        &mut self,
        build_id: &str,
    ) -> Result<BetaAppReviewSubmission> {
        #[derive(Deserialize)]
        struct SubmissionResponse {
            data: SubmissionData,
        }

        #[derive(Deserialize)]
        struct SubmissionData {
            id: String,
            attributes: SubmissionAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SubmissionAttributes {
            beta_review_state: String,
            submitted_date: Option<String>,
        }

        let body = serde_json::json!({
            "data": {
                "type": "betaAppReviewSubmissions",
                "relationships": {
                    "build": {
                        "data": {
                            "type": "builds",
                            "id": build_id
                        }
                    }
                }
            }
        });

        let response: SubmissionResponse = self
            .api_request(
                reqwest::Method::POST,
                "/betaAppReviewSubmissions",
                Some(body),
            )
            .await?;

        Ok(BetaAppReviewSubmission {
            id: response.data.id,
            build_id: build_id.to_string(),
            state: BetaReviewState::from_str(&response.data.attributes.beta_review_state),
            submitted_at: response
                .data
                .attributes
                .submitted_date
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc)),
        })
    }

    /// Get beta review submission status
    pub async fn get_beta_review_status(
        &mut self,
        submission_id: &str,
    ) -> Result<BetaAppReviewSubmission> {
        #[derive(Deserialize)]
        struct SubmissionResponse {
            data: SubmissionData,
        }

        #[derive(Deserialize)]
        struct SubmissionData {
            id: String,
            attributes: SubmissionAttributes,
            relationships: Option<SubmissionRelationships>,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct SubmissionAttributes {
            beta_review_state: String,
            submitted_date: Option<String>,
        }

        #[derive(Deserialize)]
        struct SubmissionRelationships {
            build: BuildRelationship,
        }

        #[derive(Deserialize)]
        struct BuildRelationship {
            data: BuildRef,
        }

        #[derive(Deserialize)]
        struct BuildRef {
            id: String,
        }

        let endpoint = format!("/betaAppReviewSubmissions/{}?include=build", submission_id);
        let response: SubmissionResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        let build_id = response
            .data
            .relationships
            .map(|r| r.build.data.id)
            .unwrap_or_default();

        Ok(BetaAppReviewSubmission {
            id: response.data.id,
            build_id,
            state: BetaReviewState::from_str(&response.data.attributes.beta_review_state),
            submitted_at: response
                .data
                .attributes
                .submitted_date
                .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                .map(|d| d.with_timezone(&Utc)),
        })
    }

    // -------------------------------------------------------------------------
    // Build Localized Info (What's New)
    // -------------------------------------------------------------------------

    /// Set "What's New" text for a build
    #[instrument(skip(self, whats_new), fields(build_id, locale))]
    pub async fn set_whats_new(
        &mut self,
        build_id: &str,
        locale: &str,
        whats_new: &str,
    ) -> Result<()> {
        // First, get the build localization ID or create one
        #[derive(Deserialize)]
        struct LocalizationsResponse {
            data: Vec<LocalizationData>,
        }

        #[derive(Deserialize)]
        struct LocalizationData {
            id: String,
            attributes: LocalizationAttributes,
        }

        #[derive(Deserialize)]
        struct LocalizationAttributes {
            locale: String,
        }

        let endpoint = format!("/builds/{}/buildBetaDetails", build_id);

        // Get build beta details
        #[derive(Deserialize)]
        struct BetaDetailsResponse {
            data: BetaDetailsData,
        }

        #[derive(Deserialize)]
        struct BetaDetailsData {
            id: String,
        }

        let details_response: BetaDetailsResponse = self
            .api_request(reqwest::Method::GET, &endpoint, None)
            .await?;

        // Get existing localizations
        let loc_endpoint = format!(
            "/buildBetaDetails/{}/buildBetaDetailsBetaLocalizations",
            details_response.data.id
        );

        let loc_response: LocalizationsResponse = self
            .api_request(reqwest::Method::GET, &loc_endpoint, None)
            .await
            .unwrap_or(LocalizationsResponse { data: vec![] });

        // Find or create localization
        let existing = loc_response
            .data
            .iter()
            .find(|l| l.attributes.locale == locale);

        if let Some(loc) = existing {
            // Update existing
            let body = serde_json::json!({
                "data": {
                    "type": "betaBuildLocalizations",
                    "id": loc.id,
                    "attributes": {
                        "whatsNew": whats_new
                    }
                }
            });

            let update_endpoint = format!("/betaBuildLocalizations/{}", loc.id);
            self.api_request_no_content(reqwest::Method::PATCH, &update_endpoint, Some(body))
                .await?;
        } else {
            // Create new
            let body = serde_json::json!({
                "data": {
                    "type": "betaBuildLocalizations",
                    "attributes": {
                        "locale": locale,
                        "whatsNew": whats_new
                    },
                    "relationships": {
                        "build": {
                            "data": {
                                "type": "builds",
                                "id": build_id
                            }
                        }
                    }
                }
            });

            self.api_request_no_content(
                reqwest::Method::POST,
                "/betaBuildLocalizations",
                Some(body),
            )
            .await?;
        }

        info!("Set 'What's New' for build {} in {}", build_id, locale);
        Ok(())
    }
}

// =============================================================================
// Types
// =============================================================================

/// TestFlight build information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestFlightBuild {
    /// Build ID
    pub id: String,
    /// Build version string
    pub version: String,
    /// Upload timestamp
    pub uploaded_at: Option<DateTime<Utc>>,
    /// Expiration timestamp
    pub expires_at: Option<DateTime<Utc>>,
    /// Whether the build has expired
    pub expired: bool,
    /// Processing state
    pub processing_state: BuildProcessingState,
    /// Audience type (internal/external)
    pub audience_type: BuildAudienceType,
    /// Whether uses non-exempt encryption
    pub uses_non_exempt_encryption: Option<bool>,
}

/// Build processing state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BuildProcessingState {
    Processing,
    Failed,
    Invalid,
    Valid,
}

impl BuildProcessingState {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PROCESSING" => Self::Processing,
            "FAILED" => Self::Failed,
            "INVALID" => Self::Invalid,
            "VALID" => Self::Valid,
            _ => Self::Processing,
        }
    }
}

/// Build audience type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BuildAudienceType {
    Internal,
    External,
}

impl BuildAudienceType {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "APP_STORE_ELIGIBLE" => Self::External,
            _ => Self::Internal,
        }
    }
}

/// Beta group information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaGroup {
    /// Group ID
    pub id: String,
    /// Group name
    pub name: String,
    /// Whether this is an internal group
    pub is_internal: bool,
    /// Whether public link is enabled
    pub public_link_enabled: bool,
    /// Public link URL
    pub public_link: Option<String>,
    /// Public link limit
    pub public_link_limit: Option<u32>,
}

/// Beta tester information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaTester {
    /// Tester ID
    pub id: String,
    /// Email address
    pub email: String,
    /// First name
    pub first_name: Option<String>,
    /// Last name
    pub last_name: Option<String>,
    /// How the tester was invited
    pub invite_type: TesterInviteType,
}

/// Tester invite type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TesterInviteType {
    Email,
    PublicLink,
}

impl TesterInviteType {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "PUBLIC_LINK" => Self::PublicLink,
            _ => Self::Email,
        }
    }
}

/// Beta app review submission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BetaAppReviewSubmission {
    /// Submission ID
    pub id: String,
    /// Build ID
    pub build_id: String,
    /// Review state
    pub state: BetaReviewState,
    /// Submission timestamp
    pub submitted_at: Option<DateTime<Utc>>,
}

/// Beta review state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BetaReviewState {
    WaitingForReview,
    InReview,
    Rejected,
    Approved,
}

impl BetaReviewState {
    fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "WAITING_FOR_REVIEW" => Self::WaitingForReview,
            "IN_REVIEW" => Self::InReview,
            "REJECTED" => Self::Rejected,
            "APPROVED" => Self::Approved,
            _ => Self::WaitingForReview,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_processing_state() {
        assert_eq!(
            BuildProcessingState::from_str("PROCESSING"),
            BuildProcessingState::Processing
        );
        assert_eq!(
            BuildProcessingState::from_str("VALID"),
            BuildProcessingState::Valid
        );
        assert_eq!(
            BuildProcessingState::from_str("invalid"),
            BuildProcessingState::Invalid
        );
    }

    #[test]
    fn test_beta_review_state() {
        assert_eq!(
            BetaReviewState::from_str("APPROVED"),
            BetaReviewState::Approved
        );
        assert_eq!(
            BetaReviewState::from_str("IN_REVIEW"),
            BetaReviewState::InReview
        );
    }

    #[test]
    fn test_tester_invite_type() {
        assert_eq!(TesterInviteType::from_str("EMAIL"), TesterInviteType::Email);
        assert_eq!(
            TesterInviteType::from_str("PUBLIC_LINK"),
            TesterInviteType::PublicLink
        );
    }
}
