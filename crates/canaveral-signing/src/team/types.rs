//! Core types for the team vault

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::roles::Role;
use crate::identity::SigningIdentityType;

/// Vault configuration and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Schema version for forward compatibility
    pub version: u32,

    /// Team metadata
    pub team: TeamInfo,

    /// Vault settings
    #[serde(default)]
    pub settings: VaultSettings,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            version: 1,
            team: TeamInfo::default(),
            settings: VaultSettings::default(),
        }
    }
}

/// Team information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamInfo {
    /// Unique team identifier
    pub id: Uuid,

    /// Team name
    pub name: String,

    /// Team description
    pub description: Option<String>,

    /// When the vault was created
    pub created_at: DateTime<Utc>,

    /// Who created the vault
    pub created_by: Option<String>,
}

impl Default for TeamInfo {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: "Default Team".to_string(),
            description: None,
            created_at: Utc::now(),
            created_by: None,
        }
    }
}

/// Vault settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSettings {
    /// Require audit logging for all operations
    pub require_audit: bool,

    /// Auto-sync settings
    pub auto_sync: bool,

    /// Maximum age of audit entries to keep (days)
    pub audit_retention_days: u32,

    /// Require multiple approvals for certain operations
    pub require_approval: bool,

    /// Minimum number of approvals required
    pub min_approvals: u32,
}

impl Default for VaultSettings {
    fn default() -> Self {
        Self {
            require_audit: true,
            auto_sync: false,
            audit_retention_days: 365,
            require_approval: false,
            min_approvals: 1,
        }
    }
}

/// A team member with vault access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    /// Unique member identifier
    pub id: Uuid,

    /// Member's email address
    pub email: String,

    /// Display name
    pub name: Option<String>,

    /// Member's Age public key for encryption
    pub public_key: String,

    /// Member's role in the team
    pub role: Role,

    /// When the member was added
    pub added_at: DateTime<Utc>,

    /// Who added this member
    pub added_by: Option<String>,

    /// Whether the member is currently active
    pub active: bool,

    /// Last activity timestamp
    pub last_active: Option<DateTime<Utc>>,
}

impl Member {
    /// Create a new member
    pub fn new(email: impl Into<String>, public_key: impl Into<String>, role: Role) -> Self {
        Self {
            id: Uuid::new_v4(),
            email: email.into(),
            name: None,
            public_key: public_key.into(),
            role,
            added_at: Utc::now(),
            added_by: None,
            active: true,
            last_active: None,
        }
    }

    /// Create a new admin member (typically the vault creator)
    pub fn new_admin(email: impl Into<String>, public_key: impl Into<String>) -> Self {
        Self::new(email, public_key, Role::Admin)
    }
}

/// A stored signing identity in the vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredIdentity {
    /// Unique identifier for this identity
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Type of signing identity
    #[serde(rename = "type")]
    pub identity_type: SigningIdentityType,

    /// Description of what this identity is used for
    pub description: Option<String>,

    /// Roles that can access this identity
    pub allowed_roles: Vec<Role>,

    /// When this identity was added
    pub added_at: DateTime<Utc>,

    /// Who added this identity
    pub added_by: Option<String>,

    /// When this identity expires (if known)
    pub expires_at: Option<DateTime<Utc>>,

    /// Tags for organization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Encrypted credential data (Age encrypted)
    /// This contains the actual certificate/key material
    pub encrypted_data: String,

    /// Additional metadata (unencrypted)
    #[serde(default)]
    pub metadata: IdentityMetadata,
}

impl StoredIdentity {
    /// Create a new stored identity
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        identity_type: SigningIdentityType,
        encrypted_data: String,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            identity_type,
            description: None,
            allowed_roles: vec![Role::Admin, Role::Signer],
            added_at: Utc::now(),
            added_by: None,
            expires_at: None,
            tags: Vec::new(),
            encrypted_data,
            metadata: IdentityMetadata::default(),
        }
    }

    /// Check if a role can access this identity
    pub fn can_access(&self, role: &Role) -> bool {
        self.allowed_roles.contains(role) || *role == Role::Admin
    }
}

/// Unencrypted metadata about an identity
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentityMetadata {
    /// Certificate fingerprint (for display, not security)
    pub fingerprint: Option<String>,

    /// Team ID (Apple)
    pub team_id: Option<String>,

    /// Bundle ID pattern this cert can sign
    pub bundle_id_pattern: Option<String>,

    /// Platform this identity is for
    pub platform: Option<String>,

    /// Key alias (Android)
    pub key_alias: Option<String>,

    /// GPG key ID
    pub gpg_key_id: Option<String>,
}

/// Vault metadata stored separately from encrypted data
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VaultMetadata {
    /// Last sync timestamp
    pub last_sync: Option<DateTime<Utc>>,

    /// Local machine identifier
    pub machine_id: Option<String>,

    /// Current user's member ID
    pub current_member_id: Option<Uuid>,
}

/// Credential data that gets encrypted
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CredentialData {
    /// The raw credential bytes (certificate, key, etc.)
    #[serde(with = "base64_serde")]
    pub data: Vec<u8>,

    /// Password/passphrase if needed to use the credential
    pub password: Option<String>,

    /// Additional key material
    #[serde(with = "option_base64_serde")]
    pub private_key: Option<Vec<u8>>,

    /// Format of the credential (p12, pem, jks, etc.)
    pub format: String,
}

impl CredentialData {
    /// Create new credential data from raw bytes
    pub fn new(data: Vec<u8>, format: impl Into<String>) -> Self {
        Self {
            data,
            password: None,
            private_key: None,
            format: format.into(),
        }
    }

    /// Create credential data with a password
    pub fn with_password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }
}

/// Serde helper for base64 encoding
mod base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(data))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        STANDARD.decode(&s).map_err(serde::de::Error::custom)
    }
}

/// Serde helper for optional base64 encoding
mod option_base64_serde {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(data: &Option<Vec<u8>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match data {
            Some(d) => serializer.serialize_some(&STANDARD.encode(d)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Vec<u8>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => STANDARD.decode(&s).map(Some).map_err(serde::de::Error::custom),
            None => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_member_creation() {
        let member = Member::new("test@example.com", "age1test...", Role::Signer);
        assert_eq!(member.email, "test@example.com");
        assert_eq!(member.role, Role::Signer);
        assert!(member.active);
    }

    #[test]
    fn test_stored_identity_access() {
        let identity = StoredIdentity::new(
            "test",
            "Test Cert",
            SigningIdentityType::AppleDeveloper,
            "encrypted".to_string(),
        );

        assert!(identity.can_access(&Role::Admin));
        assert!(identity.can_access(&Role::Signer));
        assert!(!identity.can_access(&Role::Viewer));
    }

    #[test]
    fn test_credential_data() {
        let cred = CredentialData::new(vec![1, 2, 3], "p12")
            .with_password("secret");

        assert_eq!(cred.format, "p12");
        assert_eq!(cred.password, Some("secret".to_string()));
    }
}
