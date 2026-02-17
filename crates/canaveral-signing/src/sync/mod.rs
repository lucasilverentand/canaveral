//! Certificate and profile synchronization (match-style)
//!
//! Provides secure storage and synchronization of code signing certificates
//! and provisioning profiles across a team, similar to fastlane match.

pub mod registry;
pub mod storage;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Result, SigningError};
use crate::team::{decrypt_data, encrypt_data, generate_keypair, KeyPair};

pub use registry::{StorageBackendEntry, StorageBackendRegistry};
pub use storage::{GitStorage, S3Storage, StorageBackend, SyncStorage};

/// Certificate type for iOS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CertificateType {
    /// Development certificate
    Development,
    /// Distribution certificate (App Store)
    Distribution,
}

impl std::fmt::Display for CertificateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Distribution => write!(f, "distribution"),
        }
    }
}

/// Profile type for iOS
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProfileType {
    /// Development profile
    Development,
    /// Ad-hoc distribution profile
    AdHoc,
    /// App Store distribution profile
    AppStore,
    /// Enterprise distribution profile
    Enterprise,
}

impl std::fmt::Display for ProfileType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::AdHoc => write!(f, "adhoc"),
            Self::AppStore => write!(f, "appstore"),
            Self::Enterprise => write!(f, "enterprise"),
        }
    }
}

/// Stored certificate metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredCertificate {
    /// Certificate type
    pub cert_type: CertificateType,

    /// Team ID
    pub team_id: String,

    /// Certificate name (common name)
    pub name: String,

    /// Expiration date (ISO 8601)
    pub expires: String,

    /// SHA-256 fingerprint
    pub fingerprint: String,

    /// File path in storage
    pub path: String,
}

/// Stored provisioning profile metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredProfile {
    /// Profile type
    pub profile_type: ProfileType,

    /// Team ID
    pub team_id: String,

    /// App identifier (bundle ID)
    pub app_id: String,

    /// Profile name
    pub name: String,

    /// Profile UUID
    pub uuid: String,

    /// Expiration date (ISO 8601)
    pub expires: String,

    /// File path in storage
    pub path: String,
}

/// Sync manifest containing all stored items
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncManifest {
    /// Manifest version
    pub version: u32,

    /// Team ID
    pub team_id: String,

    /// Stored certificates by type
    pub certificates: HashMap<CertificateType, Vec<StoredCertificate>>,

    /// Stored profiles by app ID and type
    pub profiles: HashMap<String, HashMap<ProfileType, StoredProfile>>,

    /// Last sync time (ISO 8601)
    pub last_sync: String,
}

impl SyncManifest {
    /// Create a new empty manifest
    pub fn new(team_id: impl Into<String>) -> Self {
        Self {
            version: 1,
            team_id: team_id.into(),
            certificates: HashMap::new(),
            profiles: HashMap::new(),
            last_sync: chrono::Utc::now().to_rfc3339(),
        }
    }

    /// Add a certificate
    pub fn add_certificate(&mut self, cert: StoredCertificate) {
        self.certificates
            .entry(cert.cert_type)
            .or_default()
            .push(cert);
    }

    /// Add a profile
    pub fn add_profile(&mut self, profile: StoredProfile) {
        self.profiles
            .entry(profile.app_id.clone())
            .or_default()
            .insert(profile.profile_type, profile);
    }

    /// Get certificate by type
    pub fn get_certificate(&self, cert_type: CertificateType) -> Option<&StoredCertificate> {
        self.certificates
            .get(&cert_type)
            .and_then(|certs| certs.first())
    }

    /// Get profile by app ID and type
    pub fn get_profile(&self, app_id: &str, profile_type: ProfileType) -> Option<&StoredProfile> {
        self.profiles
            .get(app_id)
            .and_then(|profiles| profiles.get(&profile_type))
    }
}

/// Match sync configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchConfig {
    /// Storage backend configuration
    pub storage: SyncStorage,

    /// Team ID
    pub team_id: String,

    /// Encryption key (public key for encrypting)
    pub encryption_key: Option<String>,

    /// Read-only mode (don't modify storage)
    pub readonly: bool,

    /// Force re-download even if cached
    pub force: bool,

    /// App IDs to sync (empty = all)
    pub app_ids: Vec<String>,

    /// Certificate types to sync
    pub cert_types: Vec<CertificateType>,

    /// Profile types to sync
    pub profile_types: Vec<ProfileType>,
}

impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            storage: SyncStorage::Git {
                url: String::new(),
                branch: "main".to_string(),
            },
            team_id: String::new(),
            encryption_key: None,
            readonly: false,
            force: false,
            app_ids: Vec::new(),
            cert_types: vec![CertificateType::Development, CertificateType::Distribution],
            profile_types: vec![
                ProfileType::Development,
                ProfileType::AdHoc,
                ProfileType::AppStore,
            ],
        }
    }
}

impl MatchConfig {
    /// Create a new config for git storage
    pub fn git(url: impl Into<String>, team_id: impl Into<String>) -> Self {
        Self {
            storage: SyncStorage::Git {
                url: url.into(),
                branch: "main".to_string(),
            },
            team_id: team_id.into(),
            ..Default::default()
        }
    }

    /// Create a new config for S3 storage
    pub fn s3(bucket: impl Into<String>, team_id: impl Into<String>) -> Self {
        Self {
            storage: SyncStorage::S3 {
                bucket: bucket.into(),
                prefix: "match".to_string(),
                region: "us-east-1".to_string(),
            },
            team_id: team_id.into(),
            ..Default::default()
        }
    }

    /// Set read-only mode
    pub fn readonly(mut self) -> Self {
        self.readonly = true;
        self
    }

    /// Set force re-download
    pub fn force(mut self) -> Self {
        self.force = true;
        self
    }

    /// Add app ID to sync
    pub fn with_app_id(mut self, app_id: impl Into<String>) -> Self {
        self.app_ids.push(app_id.into());
        self
    }

    /// Set profile types to sync
    pub fn with_profile_types(mut self, types: Vec<ProfileType>) -> Self {
        self.profile_types = types;
        self
    }
}

/// Match sync manager
pub struct MatchSync {
    /// Configuration
    config: MatchConfig,

    /// Storage backend
    storage: Box<dyn StorageBackend>,

    /// Local cache directory
    cache_dir: PathBuf,

    /// Encryption keypair (if available)
    keypair: Option<KeyPair>,
}

impl MatchSync {
    /// Create a new match sync manager
    pub fn new(config: MatchConfig) -> Result<Self> {
        let storage: Box<dyn StorageBackend> = match &config.storage {
            SyncStorage::Git { url, branch } => {
                Box::new(GitStorage::new(url.clone(), branch.clone()))
            }
            SyncStorage::S3 {
                bucket,
                prefix,
                region,
            } => Box::new(S3Storage::new(
                bucket.clone(),
                prefix.clone(),
                region.clone(),
            )),
            SyncStorage::GoogleCloudStorage { bucket, prefix } => {
                // GCS not yet implemented, use S3-compatible interface
                Box::new(S3Storage::new(
                    bucket.clone(),
                    prefix.clone(),
                    "auto".to_string(),
                ))
            }
            SyncStorage::AzureBlob { container, prefix } => {
                // Azure not yet implemented, use S3-compatible interface
                Box::new(S3Storage::new(
                    container.clone(),
                    prefix.clone(),
                    "auto".to_string(),
                ))
            }
        };

        let cache_dir = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("canaveral")
            .join("match")
            .join(&config.team_id);

        Ok(Self {
            config,
            storage,
            cache_dir,
            keypair: None,
        })
    }

    /// Set encryption keypair
    pub fn with_keypair(mut self, keypair: KeyPair) -> Self {
        self.keypair = Some(keypair);
        self
    }

    /// Initialize a new match repository
    pub async fn init(&self) -> Result<()> {
        // Create manifest
        let manifest = SyncManifest::new(&self.config.team_id);

        // Generate encryption keypair if not provided
        let keypair = self.keypair.clone().unwrap_or_else(generate_keypair);

        // Encrypt and store manifest
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            SigningError::Configuration(format!("Failed to serialize manifest: {}", e))
        })?;

        let encrypted = encrypt_data(
            manifest_json.as_bytes(),
            std::slice::from_ref(&keypair.public_key),
        )
        .map_err(|e| SigningError::Configuration(format!("Failed to encrypt manifest: {}", e)))?;

        // Store manifest
        self.storage
            .write("manifest.enc", encrypted.as_bytes())
            .await?;

        // Store public key
        self.storage
            .write("encryption_key.pub", keypair.public_key.as_bytes())
            .await?;

        Ok(())
    }

    /// Sync certificates and profiles from storage
    pub async fn sync(&self) -> Result<SyncManifest> {
        // Ensure cache directory exists
        std::fs::create_dir_all(&self.cache_dir).map_err(SigningError::Io)?;

        // Clone/pull storage
        self.storage.sync().await?;

        // Read and decrypt manifest
        let manifest = self.read_manifest().await?;

        // Download certificates
        for cert_type in &self.config.cert_types {
            if let Some(certs) = manifest.certificates.get(cert_type) {
                for cert in certs {
                    self.download_certificate(cert).await?;
                }
            }
        }

        // Download profiles
        for (app_id, profiles) in &manifest.profiles {
            if !self.config.app_ids.is_empty() && !self.config.app_ids.contains(app_id) {
                continue;
            }

            for profile_type in &self.config.profile_types {
                if let Some(profile) = profiles.get(profile_type) {
                    self.download_profile(profile).await?;
                }
            }
        }

        Ok(manifest)
    }

    /// Read manifest from storage
    async fn read_manifest(&self) -> Result<SyncManifest> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        let encrypted = self.storage.read("manifest.enc").await?;
        let encrypted_str = String::from_utf8(encrypted).map_err(|e| {
            SigningError::Configuration(format!("Invalid manifest encoding: {}", e))
        })?;

        let decrypted = decrypt_data(&encrypted_str, &keypair.private_key).map_err(|e| {
            SigningError::Configuration(format!("Failed to decrypt manifest: {}", e))
        })?;

        let manifest: SyncManifest = serde_json::from_slice(&decrypted)
            .map_err(|e| SigningError::Configuration(format!("Invalid manifest format: {}", e)))?;

        Ok(manifest)
    }

    /// Download and install a certificate
    async fn download_certificate(&self, cert: &StoredCertificate) -> Result<PathBuf> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        let encrypted = self.storage.read(&cert.path).await?;
        let encrypted_str = String::from_utf8(encrypted).map_err(|e| {
            SigningError::Configuration(format!("Invalid certificate encoding: {}", e))
        })?;

        let decrypted = decrypt_data(&encrypted_str, &keypair.private_key).map_err(|e| {
            SigningError::Configuration(format!("Failed to decrypt certificate: {}", e))
        })?;

        // Save to cache
        let cache_path = self
            .cache_dir
            .join("certs")
            .join(format!("{}_{}.p12", cert.cert_type, cert.fingerprint));

        std::fs::create_dir_all(cache_path.parent().unwrap()).map_err(SigningError::Io)?;

        std::fs::write(&cache_path, &decrypted).map_err(SigningError::Io)?;

        Ok(cache_path)
    }

    /// Download and install a profile
    async fn download_profile(&self, profile: &StoredProfile) -> Result<PathBuf> {
        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        let encrypted = self.storage.read(&profile.path).await?;
        let encrypted_str = String::from_utf8(encrypted)
            .map_err(|e| SigningError::Configuration(format!("Invalid profile encoding: {}", e)))?;

        let decrypted = decrypt_data(&encrypted_str, &keypair.private_key).map_err(|e| {
            SigningError::Configuration(format!("Failed to decrypt profile: {}", e))
        })?;

        // Save to provisioning profiles directory
        let profiles_dir = dirs::home_dir()
            .map(|h| h.join("Library/MobileDevice/Provisioning Profiles"))
            .unwrap_or_else(|| self.cache_dir.join("profiles"));

        std::fs::create_dir_all(&profiles_dir).map_err(SigningError::Io)?;

        let profile_path = profiles_dir.join(format!("{}.mobileprovision", profile.uuid));

        std::fs::write(&profile_path, &decrypted).map_err(SigningError::Io)?;

        Ok(profile_path)
    }

    /// Upload a certificate to storage
    pub async fn upload_certificate(
        &self,
        _cert_type: CertificateType,
        data: &[u8],
        metadata: StoredCertificate,
    ) -> Result<()> {
        if self.config.readonly {
            return Err(SigningError::Configuration(
                "Cannot upload in readonly mode".to_string(),
            ));
        }

        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        // Encrypt certificate
        let encrypted =
            encrypt_data(data, std::slice::from_ref(&keypair.public_key)).map_err(|e| {
                SigningError::Configuration(format!("Failed to encrypt certificate: {}", e))
            })?;

        // Write to storage
        self.storage
            .write(&metadata.path, encrypted.as_bytes())
            .await?;

        // Update manifest
        let mut manifest = self.read_manifest().await?;
        manifest.add_certificate(metadata);
        manifest.last_sync = chrono::Utc::now().to_rfc3339();

        // Write updated manifest
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            SigningError::Configuration(format!("Failed to serialize manifest: {}", e))
        })?;

        let encrypted_manifest = encrypt_data(
            manifest_json.as_bytes(),
            std::slice::from_ref(&keypair.public_key),
        )
        .map_err(|e| SigningError::Configuration(format!("Failed to encrypt manifest: {}", e)))?;

        self.storage
            .write("manifest.enc", encrypted_manifest.as_bytes())
            .await?;

        Ok(())
    }

    /// Upload a profile to storage
    pub async fn upload_profile(&self, data: &[u8], metadata: StoredProfile) -> Result<()> {
        if self.config.readonly {
            return Err(SigningError::Configuration(
                "Cannot upload in readonly mode".to_string(),
            ));
        }

        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        // Encrypt profile
        let encrypted =
            encrypt_data(data, std::slice::from_ref(&keypair.public_key)).map_err(|e| {
                SigningError::Configuration(format!("Failed to encrypt profile: {}", e))
            })?;

        // Write to storage
        self.storage
            .write(&metadata.path, encrypted.as_bytes())
            .await?;

        // Update manifest
        let mut manifest = self.read_manifest().await?;
        manifest.add_profile(metadata);
        manifest.last_sync = chrono::Utc::now().to_rfc3339();

        // Write updated manifest
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            SigningError::Configuration(format!("Failed to serialize manifest: {}", e))
        })?;

        let encrypted_manifest = encrypt_data(
            manifest_json.as_bytes(),
            std::slice::from_ref(&keypair.public_key),
        )
        .map_err(|e| SigningError::Configuration(format!("Failed to encrypt manifest: {}", e)))?;

        self.storage
            .write("manifest.enc", encrypted_manifest.as_bytes())
            .await?;

        Ok(())
    }

    /// Remove all certificates and profiles (nuke)
    pub async fn nuke(&self, profile_type: Option<ProfileType>) -> Result<()> {
        if self.config.readonly {
            return Err(SigningError::Configuration(
                "Cannot nuke in readonly mode".to_string(),
            ));
        }

        let keypair = self
            .keypair
            .as_ref()
            .ok_or_else(|| SigningError::Configuration("No encryption key provided".to_string()))?;

        let mut manifest = self.read_manifest().await?;

        if let Some(pt) = profile_type {
            // Remove only profiles of specified type
            for profiles in manifest.profiles.values_mut() {
                profiles.remove(&pt);
            }
        } else {
            // Remove everything
            manifest.certificates.clear();
            manifest.profiles.clear();
        }

        manifest.last_sync = chrono::Utc::now().to_rfc3339();

        // Write updated manifest
        let manifest_json = serde_json::to_string_pretty(&manifest).map_err(|e| {
            SigningError::Configuration(format!("Failed to serialize manifest: {}", e))
        })?;

        let encrypted = encrypt_data(
            manifest_json.as_bytes(),
            std::slice::from_ref(&keypair.public_key),
        )
        .map_err(|e| SigningError::Configuration(format!("Failed to encrypt manifest: {}", e)))?;

        self.storage
            .write("manifest.enc", encrypted.as_bytes())
            .await?;

        Ok(())
    }

    /// Get cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_new() {
        let manifest = SyncManifest::new("TEAM123");
        assert_eq!(manifest.team_id, "TEAM123");
        assert_eq!(manifest.version, 1);
    }

    #[test]
    fn test_config_builder() {
        let config = MatchConfig::git("git@github.com:org/certs.git", "TEAM123")
            .readonly()
            .with_app_id("com.example.app");

        assert!(config.readonly);
        assert_eq!(config.app_ids, vec!["com.example.app"]);
    }
}
