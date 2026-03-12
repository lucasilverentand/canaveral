//! Apple Developer Portal client for provisioning profile management
//!
//! Uses the App Store Connect API v1 to list, create, download, and delete
//! provisioning profiles. Authentication uses JWT with an App Store Connect
//! API key (the same mechanism as `canaveral-stores`).

use std::path::Path;

use chrono::{Duration, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

use crate::error::{Result, SigningError};
use crate::sync::ProfileType;

const API_BASE_URL: &str = "https://api.appstoreconnect.apple.com/v1";

/// Configuration for the Apple Developer Portal client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalConfig {
    /// App Store Connect API Key ID (e.g., "2X9R4HXF34")
    pub api_key_id: String,

    /// Issuer ID from App Store Connect
    pub api_issuer_id: String,

    /// Path to the `.p8` private key file, or the key content directly
    pub api_key: String,
}

/// A provisioning profile as returned by the App Store Connect API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortalProfile {
    /// App Store Connect resource ID
    pub id: String,

    /// Profile name
    pub name: String,

    /// Profile UUID
    pub uuid: String,

    /// Profile type from the API
    pub profile_type: String,

    /// Profile state (e.g., "ACTIVE", "INVALID")
    pub profile_state: String,

    /// Creation date
    pub created_date: Option<String>,

    /// Expiration date
    pub expiration_date: Option<String>,

    /// Base64-encoded profile content (when downloaded)
    pub profile_content: Option<String>,
}

/// JWT claims for App Store Connect API authentication.
#[derive(Debug, Serialize)]
struct Claims {
    iss: String,
    iat: i64,
    exp: i64,
    aud: String,
}

/// Client for the Apple Developer Portal (App Store Connect API).
///
/// Manages provisioning profiles via REST API, including listing,
/// downloading, creating, and deleting profiles.
pub struct PortalClient {
    config: PortalConfig,
    client: reqwest::Client,
    jwt_token: Option<String>,
    token_expires: Option<chrono::DateTime<Utc>>,
}

impl PortalClient {
    /// Create a new portal client.
    pub fn new(config: PortalConfig) -> Result<Self> {
        let client = reqwest::Client::new();
        Ok(Self {
            config,
            client,
            jwt_token: None,
            token_expires: None,
        })
    }

    /// Generate a JWT token for API authentication.
    ///
    /// Tokens are cached and reused until 5 minutes before expiration.
    fn generate_jwt(&mut self) -> Result<String> {
        // Reuse cached token if still valid
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
                SigningError::Configuration(format!("Failed to read API key file: {}", e))
            })?
        } else {
            self.config.api_key.clone()
        };

        let encoding_key = jsonwebtoken::EncodingKey::from_ec_pem(key_content.as_bytes())
            .map_err(|e| SigningError::Configuration(format!("Invalid API key: {}", e)))?;

        let mut header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::ES256);
        header.kid = Some(self.config.api_key_id.clone());

        let token = jsonwebtoken::encode(&header, &claims, &encoding_key)
            .map_err(|e| SigningError::Configuration(format!("Failed to generate JWT: {}", e)))?;

        self.jwt_token = Some(token.clone());
        self.token_expires = Some(exp);

        Ok(token)
    }

    /// Make an authenticated GET request to the API.
    async fn api_get<T: serde::de::DeserializeOwned>(&mut self, endpoint: &str) -> Result<T> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        debug!(url = %url, "API GET request");

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| SigningError::Configuration(format!("API request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SigningError::Configuration(format!(
                "API error ({}): {}",
                status, error_text
            )));
        }

        response.json().await.map_err(|e| {
            SigningError::Configuration(format!("Failed to parse API response: {}", e))
        })
    }

    /// Make an authenticated POST request to the API.
    async fn api_post<T: serde::de::DeserializeOwned>(
        &mut self,
        endpoint: &str,
        body: serde_json::Value,
    ) -> Result<T> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        debug!(url = %url, "API POST request");

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SigningError::Configuration(format!("API request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SigningError::Configuration(format!(
                "API error ({}): {}",
                status, error_text
            )));
        }

        response.json().await.map_err(|e| {
            SigningError::Configuration(format!("Failed to parse API response: {}", e))
        })
    }

    /// Make an authenticated DELETE request to the API.
    async fn api_delete(&mut self, endpoint: &str) -> Result<()> {
        let token = self.generate_jwt()?;
        let url = format!("{}{}", API_BASE_URL, endpoint);

        debug!(url = %url, "API DELETE request");

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| SigningError::Configuration(format!("API request failed: {}", e)))?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(SigningError::Configuration(format!(
                "API error ({}): {}",
                status, error_text
            )));
        }

        Ok(())
    }

    /// List all provisioning profiles from the developer portal.
    #[instrument(skip(self))]
    pub async fn list_profiles(&mut self) -> Result<Vec<PortalProfile>> {
        #[derive(Deserialize)]
        struct ProfilesResponse {
            data: Vec<ProfileData>,
        }

        #[derive(Deserialize)]
        struct ProfileData {
            id: String,
            attributes: ProfileAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ProfileAttributes {
            name: String,
            uuid: String,
            profile_type: String,
            profile_state: String,
            created_date: Option<String>,
            expiration_date: Option<String>,
            profile_content: Option<String>,
        }

        let response: ProfilesResponse = self.api_get("/profiles").await?;

        let profiles = response
            .data
            .into_iter()
            .map(|d| PortalProfile {
                id: d.id,
                name: d.attributes.name,
                uuid: d.attributes.uuid,
                profile_type: d.attributes.profile_type,
                profile_state: d.attributes.profile_state,
                created_date: d.attributes.created_date,
                expiration_date: d.attributes.expiration_date,
                profile_content: d.attributes.profile_content,
            })
            .collect::<Vec<_>>();

        info!(count = profiles.len(), "Listed profiles from portal");
        Ok(profiles)
    }

    /// Download a specific provisioning profile by its resource ID.
    ///
    /// The API returns the profile content as base64-encoded data.
    #[instrument(skip(self), fields(profile_id = %profile_id))]
    pub async fn download_profile(&mut self, profile_id: &str) -> Result<PortalProfile> {
        #[derive(Deserialize)]
        struct ProfileResponse {
            data: ProfileData,
        }

        #[derive(Deserialize)]
        struct ProfileData {
            id: String,
            attributes: ProfileAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ProfileAttributes {
            name: String,
            uuid: String,
            profile_type: String,
            profile_state: String,
            created_date: Option<String>,
            expiration_date: Option<String>,
            profile_content: Option<String>,
        }

        let endpoint = format!("/profiles/{}", profile_id);
        let response: ProfileResponse = self.api_get(&endpoint).await?;

        let d = response.data;
        info!(
            profile_id = %d.id,
            name = %d.attributes.name,
            "Downloaded profile from portal"
        );

        Ok(PortalProfile {
            id: d.id,
            name: d.attributes.name,
            uuid: d.attributes.uuid,
            profile_type: d.attributes.profile_type,
            profile_state: d.attributes.profile_state,
            created_date: d.attributes.created_date,
            expiration_date: d.attributes.expiration_date,
            profile_content: d.attributes.profile_content,
        })
    }

    /// Create a new provisioning profile on the developer portal.
    ///
    /// # Arguments
    /// * `name` - Profile name
    /// * `profile_type` - The type of profile to create
    /// * `bundle_id_resource_id` - The App Store Connect resource ID for the bundle ID
    /// * `certificate_ids` - Resource IDs of certificates to include
    /// * `device_ids` - Resource IDs of devices to include (for dev/ad-hoc)
    #[instrument(skip(self, certificate_ids, device_ids), fields(name = %name, profile_type = %profile_type))]
    pub async fn create_profile(
        &mut self,
        name: &str,
        profile_type: ProfileType,
        bundle_id_resource_id: &str,
        certificate_ids: &[String],
        device_ids: &[String],
    ) -> Result<PortalProfile> {
        let api_profile_type = match profile_type {
            ProfileType::Development => "IOS_APP_DEVELOPMENT",
            ProfileType::AdHoc => "IOS_APP_ADHOC",
            ProfileType::AppStore => "IOS_APP_STORE",
            ProfileType::Enterprise => "IOS_APP_INHOUSE",
        };

        let mut relationships = serde_json::json!({
            "bundleId": {
                "data": {
                    "type": "bundleIds",
                    "id": bundle_id_resource_id
                }
            },
            "certificates": {
                "data": certificate_ids.iter().map(|id| {
                    serde_json::json!({
                        "type": "certificates",
                        "id": id
                    })
                }).collect::<Vec<_>>()
            }
        });

        // Only include devices for development and ad-hoc profiles
        if matches!(profile_type, ProfileType::Development | ProfileType::AdHoc)
            && !device_ids.is_empty()
        {
            relationships["devices"] = serde_json::json!({
                "data": device_ids.iter().map(|id| {
                    serde_json::json!({
                        "type": "devices",
                        "id": id
                    })
                }).collect::<Vec<_>>()
            });
        }

        let body = serde_json::json!({
            "data": {
                "type": "profiles",
                "attributes": {
                    "name": name,
                    "profileType": api_profile_type
                },
                "relationships": relationships
            }
        });

        #[derive(Deserialize)]
        struct ProfileResponse {
            data: ProfileData,
        }

        #[derive(Deserialize)]
        struct ProfileData {
            id: String,
            attributes: ProfileAttributes,
        }

        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct ProfileAttributes {
            name: String,
            uuid: String,
            profile_type: String,
            profile_state: String,
            created_date: Option<String>,
            expiration_date: Option<String>,
            profile_content: Option<String>,
        }

        let response: ProfileResponse = self.api_post("/profiles", body).await?;

        let d = response.data;
        info!(
            profile_id = %d.id,
            uuid = %d.attributes.uuid,
            "Created profile on portal"
        );

        Ok(PortalProfile {
            id: d.id,
            name: d.attributes.name,
            uuid: d.attributes.uuid,
            profile_type: d.attributes.profile_type,
            profile_state: d.attributes.profile_state,
            created_date: d.attributes.created_date,
            expiration_date: d.attributes.expiration_date,
            profile_content: d.attributes.profile_content,
        })
    }

    /// Delete (revoke) a provisioning profile from the developer portal.
    #[instrument(skip(self), fields(profile_id = %profile_id))]
    pub async fn delete_profile(&mut self, profile_id: &str) -> Result<()> {
        let endpoint = format!("/profiles/{}", profile_id);
        self.api_delete(&endpoint).await?;
        info!(profile_id = %profile_id, "Deleted profile from portal");
        Ok(())
    }

    /// Decode the base64 profile content into raw bytes.
    pub fn decode_profile_content(portal_profile: &PortalProfile) -> Result<Vec<u8>> {
        let content = portal_profile.profile_content.as_ref().ok_or_else(|| {
            SigningError::ProvisioningProfileError(
                "Profile content not available (use download_profile to fetch it)".to_string(),
            )
        })?;

        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(content)
            .map_err(|e| {
                SigningError::ProvisioningProfileError(format!(
                    "Failed to decode profile content: {}",
                    e
                ))
            })
    }

    /// Map a portal profile type string to our ProfileType enum.
    pub fn map_profile_type(api_type: &str) -> ProfileType {
        match api_type {
            "IOS_APP_DEVELOPMENT" | "MAC_APP_DEVELOPMENT" | "TVOS_APP_DEVELOPMENT" => {
                ProfileType::Development
            }
            "IOS_APP_ADHOC" | "TVOS_APP_ADHOC" => ProfileType::AdHoc,
            "IOS_APP_STORE" | "MAC_APP_STORE" | "TVOS_APP_STORE" | "MAC_APP_DIRECT" => {
                ProfileType::AppStore
            }
            "IOS_APP_INHOUSE" | "TVOS_APP_INHOUSE" => ProfileType::Enterprise,
            _ => ProfileType::Development, // Default fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_profile_type() {
        assert!(matches!(
            PortalClient::map_profile_type("IOS_APP_DEVELOPMENT"),
            ProfileType::Development
        ));
        assert!(matches!(
            PortalClient::map_profile_type("IOS_APP_ADHOC"),
            ProfileType::AdHoc
        ));
        assert!(matches!(
            PortalClient::map_profile_type("IOS_APP_STORE"),
            ProfileType::AppStore
        ));
        assert!(matches!(
            PortalClient::map_profile_type("MAC_APP_STORE"),
            ProfileType::AppStore
        ));
        assert!(matches!(
            PortalClient::map_profile_type("IOS_APP_INHOUSE"),
            ProfileType::Enterprise
        ));
        assert!(matches!(
            PortalClient::map_profile_type("UNKNOWN_TYPE"),
            ProfileType::Development
        ));
    }

    #[test]
    fn test_decode_missing_content() {
        let profile = PortalProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            uuid: "uuid".to_string(),
            profile_type: "IOS_APP_DEVELOPMENT".to_string(),
            profile_state: "ACTIVE".to_string(),
            created_date: None,
            expiration_date: None,
            profile_content: None,
        };

        let result = PortalClient::decode_profile_content(&profile);
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_valid_content() {
        use base64::Engine;
        let data = b"test profile data";
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);

        let profile = PortalProfile {
            id: "test".to_string(),
            name: "Test".to_string(),
            uuid: "uuid".to_string(),
            profile_type: "IOS_APP_DEVELOPMENT".to_string(),
            profile_state: "ACTIVE".to_string(),
            created_date: None,
            expiration_date: None,
            profile_content: Some(encoded),
        };

        let decoded = PortalClient::decode_profile_content(&profile).unwrap();
        assert_eq!(decoded, data);
    }
}
