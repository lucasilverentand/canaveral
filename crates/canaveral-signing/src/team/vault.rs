//! Team vault implementation

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

use super::audit::{AuditAction, AuditLog};
use super::encryption::{decrypt_data, encrypt_data, generate_keypair, EncryptionError, KeyPair};
use super::roles::{Permission, Role};
use super::types::{CredentialData, Member, StoredIdentity, VaultConfig, VaultMetadata};
use crate::identity::SigningIdentityType;

/// Vault-related errors
#[derive(Debug, Error)]
pub enum VaultError {
    /// Vault not found at path
    #[error("Vault not found at {0}")]
    NotFound(PathBuf),

    /// Vault already exists
    #[error("Vault already exists at {0}")]
    AlreadyExists(PathBuf),

    /// Not initialized
    #[error("Vault not initialized. Run 'canaveral signing team init' first")]
    NotInitialized,

    /// Member not found
    #[error("Member not found: {0}")]
    MemberNotFound(String),

    /// Member already exists
    #[error("Member already exists: {0}")]
    MemberExists(String),

    /// Identity not found
    #[error("Identity not found: {0}")]
    IdentityNotFound(String),

    /// Identity already exists
    #[error("Identity already exists: {0}")]
    IdentityExists(String),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// No current user configured
    #[error("No current user configured. Set CANAVERAL_SIGNING_KEY or run 'canaveral signing team auth'")]
    NoCurrentUser,

    /// Encryption error
    #[error("Encryption error: {0}")]
    Encryption(#[from] EncryptionError),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parsing error
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// JSON error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for vault operations
pub type Result<T> = std::result::Result<T, VaultError>;

/// The team signing vault
pub struct TeamVault {
    /// Path to vault directory
    path: PathBuf,

    /// Vault configuration
    config: VaultConfig,

    /// Team members
    members: HashMap<Uuid, Member>,

    /// Stored identities
    identities: HashMap<String, StoredIdentity>,

    /// Audit log
    audit: AuditLog,

    /// Local metadata
    metadata: VaultMetadata,

    /// Current user's private key (loaded from env/file)
    current_key: Option<String>,

    /// Current member
    current_member: Option<Member>,
}

impl TeamVault {
    /// Initialize a new vault
    #[instrument(skip_all, fields(team = team_name, vault_path = %path.display()))]
    pub fn init(team_name: &str, path: &Path, creator_email: &str) -> Result<(Self, KeyPair)> {
        if path.exists() && path.join("vault.yaml").exists() {
            return Err(VaultError::AlreadyExists(path.to_path_buf()));
        }

        // Create vault directory
        std::fs::create_dir_all(path)?;

        // Generate keypair for creator
        let keypair = generate_keypair();

        // Create config
        let mut config = VaultConfig::default();
        config.team.name = team_name.to_string();
        config.team.created_by = Some(creator_email.to_string());

        // Create admin member
        let admin = Member::new_admin(creator_email, &keypair.public_key);

        let mut members = HashMap::new();
        members.insert(admin.id, admin.clone());

        // Create audit log
        let mut audit = AuditLog::new();
        audit.add(creator_email, AuditAction::VaultInit);

        let vault = Self {
            path: path.to_path_buf(),
            config,
            members,
            identities: HashMap::new(),
            audit,
            metadata: VaultMetadata::default(),
            current_key: Some(keypair.private_key.clone()),
            current_member: Some(admin),
        };

        // Save everything
        vault.save()?;

        info!("Initialized vault for team '{}' at {:?}", team_name, path);

        Ok((vault, keypair))
    }

    /// Open an existing vault
    #[instrument(skip_all, fields(vault_path = %path.display()))]
    pub fn open(path: &Path) -> Result<Self> {
        if !path.exists() || !path.join("vault.yaml").exists() {
            return Err(VaultError::NotFound(path.to_path_buf()));
        }

        // Load config
        let config_path = path.join("vault.yaml");
        let config: VaultConfig = serde_yaml::from_str(&std::fs::read_to_string(&config_path)?)?;

        // Load members
        let members_path = path.join("members.yaml");
        let members: HashMap<Uuid, Member> = if members_path.exists() {
            serde_yaml::from_str(&std::fs::read_to_string(&members_path)?)?
        } else {
            HashMap::new()
        };

        // Load identities
        let identities_path = path.join("identities.yaml");
        let identities: HashMap<String, StoredIdentity> = if identities_path.exists() {
            serde_yaml::from_str(&std::fs::read_to_string(&identities_path)?)?
        } else {
            HashMap::new()
        };

        // Load audit log
        let audit_path = path.join("audit.yaml");
        let audit = if audit_path.exists() {
            AuditLog::load(&audit_path)?
        } else {
            AuditLog::new()
        };

        // Load local metadata
        let metadata_path = path.join(".metadata.yaml");
        let metadata: VaultMetadata = if metadata_path.exists() {
            serde_yaml::from_str(&std::fs::read_to_string(&metadata_path)?)?
        } else {
            VaultMetadata::default()
        };

        // Try to load current user's key
        let current_key = std::env::var("CANAVERAL_SIGNING_KEY").ok();

        // Find current member by key
        let current_member = current_key.as_ref().and_then(|key| {
            // Parse key to get public key
            if let Ok(identity) = key.parse::<age::x25519::Identity>() {
                let public_key = identity.to_public().to_string();
                members
                    .values()
                    .find(|m| m.public_key == public_key)
                    .cloned()
            } else {
                None
            }
        });

        debug!("Opened vault at {:?}", path);

        Ok(Self {
            path: path.to_path_buf(),
            config,
            members,
            identities,
            audit,
            metadata,
            current_key,
            current_member,
        })
    }

    /// Save all vault data
    pub fn save(&self) -> Result<()> {
        // Save config
        let config_yaml = serde_yaml::to_string(&self.config)?;
        std::fs::write(self.path.join("vault.yaml"), config_yaml)?;

        // Save members
        let members_yaml = serde_yaml::to_string(&self.members)?;
        std::fs::write(self.path.join("members.yaml"), members_yaml)?;

        // Save identities
        let identities_yaml = serde_yaml::to_string(&self.identities)?;
        std::fs::write(self.path.join("identities.yaml"), identities_yaml)?;

        // Save audit log
        self.audit.save(&self.path.join("audit.yaml"))?;

        // Save local metadata (not committed to git)
        let metadata_yaml = serde_yaml::to_string(&self.metadata)?;
        std::fs::write(self.path.join(".metadata.yaml"), metadata_yaml)?;

        // Create .gitignore for local files
        let gitignore = ".metadata.yaml\n";
        let gitignore_path = self.path.join(".gitignore");
        if !gitignore_path.exists() {
            std::fs::write(gitignore_path, gitignore)?;
        }

        debug!("Saved vault to {:?}", self.path);
        Ok(())
    }

    /// Get the vault path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the team name
    pub fn team_name(&self) -> &str {
        &self.config.team.name
    }

    /// Get the current member
    pub fn current_member(&self) -> Option<&Member> {
        self.current_member.as_ref()
    }

    /// Check if current user has a permission
    fn check_permission(&self, permission: Permission) -> Result<()> {
        let member = self
            .current_member
            .as_ref()
            .ok_or(VaultError::NoCurrentUser)?;

        if !member.role.has_permission(permission) {
            return Err(VaultError::PermissionDenied(format!(
                "Role '{}' does not have permission '{}'",
                member.role, permission
            )));
        }

        Ok(())
    }

    /// Get current actor email for audit
    fn current_actor(&self) -> String {
        self.current_member
            .as_ref()
            .map(|m| m.email.clone())
            .unwrap_or_else(|| "unknown".to_string())
    }

    // ========== Member Management ==========

    /// Add a new member to the vault
    #[instrument(skip(self, public_key), fields(email = email, role = %role))]
    pub fn add_member(&mut self, email: &str, public_key: &str, role: Role) -> Result<&Member> {
        self.check_permission(Permission::AddMembers)?;

        // Check if member already exists
        if self.members.values().any(|m| m.email == email) {
            return Err(VaultError::MemberExists(email.to_string()));
        }

        let member = Member::new(email, public_key, role);
        let id = member.id;

        // Re-encrypt all identities for the new member
        self.reencrypt_identities_for_new_member(&member)?;

        self.members.insert(id, member);

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::MemberAdd {
                email: email.to_string(),
                role: role.to_string(),
            },
        );

        self.save()?;

        info!("Added member {} with role {}", email, role);

        Ok(self.members.get(&id).unwrap())
    }

    /// Remove a member from the vault
    #[instrument(skip(self), fields(email = email))]
    pub fn remove_member(&mut self, email: &str) -> Result<()> {
        self.check_permission(Permission::RemoveMembers)?;

        let member_id = self
            .members
            .values()
            .find(|m| m.email == email)
            .map(|m| m.id)
            .ok_or_else(|| VaultError::MemberNotFound(email.to_string()))?;

        // Don't allow removing the last admin
        let admin_count = self
            .members
            .values()
            .filter(|m| m.role == Role::Admin)
            .count();
        if admin_count == 1 && self.members.get(&member_id).map(|m| m.role) == Some(Role::Admin) {
            return Err(VaultError::PermissionDenied(
                "Cannot remove the last admin".to_string(),
            ));
        }

        self.members.remove(&member_id);

        // Re-encrypt all identities without the removed member
        self.reencrypt_all_identities()?;

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::MemberRemove {
                email: email.to_string(),
            },
        );

        self.save()?;

        info!("Removed member {}", email);

        Ok(())
    }

    /// Change a member's role
    pub fn change_role(&mut self, email: &str, new_role: Role) -> Result<()> {
        self.check_permission(Permission::ChangeRoles)?;

        // First, find the member and get their current role
        let (member_id, old_role) = {
            let member = self
                .members
                .values()
                .find(|m| m.email == email)
                .ok_or_else(|| VaultError::MemberNotFound(email.to_string()))?;
            (member.id, member.role)
        };

        // Don't allow demoting the last admin
        if old_role == Role::Admin && new_role != Role::Admin {
            let admin_count = self
                .members
                .values()
                .filter(|m| m.role == Role::Admin)
                .count();
            if admin_count == 1 {
                return Err(VaultError::PermissionDenied(
                    "Cannot demote the last admin".to_string(),
                ));
            }
        }

        // Now mutate
        if let Some(member) = self.members.get_mut(&member_id) {
            member.role = new_role;
        }

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::MemberRoleChange {
                email: email.to_string(),
                old_role: old_role.to_string(),
                new_role: new_role.to_string(),
            },
        );

        self.save()?;

        info!(
            "Changed role for {} from {} to {}",
            email, old_role, new_role
        );

        Ok(())
    }

    /// List all members
    pub fn list_members(&self) -> Vec<&Member> {
        self.members.values().collect()
    }

    /// Get a member by email
    pub fn get_member(&self, email: &str) -> Option<&Member> {
        self.members.values().find(|m| m.email == email)
    }

    // ========== Identity Management ==========

    /// Import a signing identity
    #[instrument(skip(self, credential_data), fields(id = id, identity_type = ?identity_type))]
    pub fn import_identity(
        &mut self,
        id: &str,
        name: &str,
        identity_type: SigningIdentityType,
        credential_data: CredentialData,
    ) -> Result<&StoredIdentity> {
        self.check_permission(Permission::ImportIdentities)?;

        if self.identities.contains_key(id) {
            return Err(VaultError::IdentityExists(id.to_string()));
        }

        // Serialize credential data
        let credential_json = serde_json::to_vec(&credential_data)?;

        // Encrypt for all active members who can access this identity
        let recipients = self.get_recipients_for_identity(&[Role::Admin, Role::Signer]);
        let encrypted = encrypt_data(&credential_json, &recipients)?;

        let identity = StoredIdentity::new(id, name, identity_type, encrypted);

        self.identities.insert(id.to_string(), identity);

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::IdentityImport {
                identity_id: id.to_string(),
                identity_type: format!("{:?}", identity_type),
            },
        );

        self.save()?;

        info!("Imported identity {} ({})", id, identity_type);

        Ok(self.identities.get(id).unwrap())
    }

    /// Export (decrypt) an identity
    #[instrument(skip(self), fields(id = id))]
    pub fn export_identity(&mut self, id: &str) -> Result<CredentialData> {
        self.check_permission(Permission::ExportIdentities)?;

        let identity = self
            .identities
            .get(id)
            .ok_or_else(|| VaultError::IdentityNotFound(id.to_string()))?;

        // Check if current member can access this identity
        let member = self
            .current_member
            .as_ref()
            .ok_or(VaultError::NoCurrentUser)?;
        if !identity.can_access(&member.role) {
            return Err(VaultError::PermissionDenied(format!(
                "Role '{}' cannot access identity '{}'",
                member.role, id
            )));
        }

        let private_key = self.current_key.as_ref().ok_or(VaultError::NoCurrentUser)?;

        // Decrypt
        let decrypted = decrypt_data(&identity.encrypted_data, private_key)?;
        let credential: CredentialData = serde_json::from_slice(&decrypted)?;

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::IdentityExport {
                identity_id: id.to_string(),
            },
        );

        self.save()?;

        Ok(credential)
    }

    /// Delete an identity
    #[instrument(skip(self), fields(id = id))]
    pub fn delete_identity(&mut self, id: &str) -> Result<()> {
        self.check_permission(Permission::DeleteIdentities)?;

        if !self.identities.contains_key(id) {
            return Err(VaultError::IdentityNotFound(id.to_string()));
        }

        self.identities.remove(id);

        // Audit
        self.audit.add(
            self.current_actor(),
            AuditAction::IdentityDelete {
                identity_id: id.to_string(),
            },
        );

        self.save()?;

        info!("Deleted identity {}", id);

        Ok(())
    }

    /// List all identities (metadata only)
    pub fn list_identities(&self) -> Vec<&StoredIdentity> {
        self.identities.values().collect()
    }

    /// Get an identity by ID
    pub fn get_identity(&self, id: &str) -> Option<&StoredIdentity> {
        self.identities.get(id)
    }

    /// Record a signing operation in the audit log
    pub fn record_signing(&mut self, identity_id: &str, artifact: &str) -> Result<()> {
        self.audit.add(
            self.current_actor(),
            AuditAction::IdentitySign {
                identity_id: identity_id.to_string(),
                artifact: artifact.to_string(),
            },
        );
        self.save()?;
        Ok(())
    }

    // ========== Audit ==========

    /// Get the audit log
    pub fn audit_log(&self) -> &AuditLog {
        &self.audit
    }

    // ========== Internal helpers ==========

    /// Get public keys for members who can access identities with given roles
    fn get_recipients_for_identity(&self, allowed_roles: &[Role]) -> Vec<String> {
        self.members
            .values()
            .filter(|m| m.active && allowed_roles.contains(&m.role))
            .map(|m| m.public_key.clone())
            .collect()
    }

    /// Re-encrypt all identities for a new member
    fn reencrypt_identities_for_new_member(&mut self, new_member: &Member) -> Result<()> {
        let private_key = self.current_key.clone().ok_or(VaultError::NoCurrentUser)?;

        // Collect identity IDs and their allowed roles first
        let identities_to_update: Vec<(String, Vec<Role>)> = self
            .identities
            .values()
            .filter(|id| id.can_access(&new_member.role))
            .map(|id| (id.id.clone(), id.allowed_roles.clone()))
            .collect();

        for (identity_id, allowed_roles) in identities_to_update {
            // Get recipients for this identity
            let mut recipients = self.get_recipients_for_identity(&allowed_roles);
            if !recipients.contains(&new_member.public_key) {
                recipients.push(new_member.public_key.clone());
            }

            // Now mutate the identity
            if let Some(identity) = self.identities.get_mut(&identity_id) {
                let decrypted = decrypt_data(&identity.encrypted_data, &private_key)?;
                identity.encrypted_data = encrypt_data(&decrypted, &recipients)?;
            }
        }

        Ok(())
    }

    /// Re-encrypt all identities (e.g., after removing a member)
    fn reencrypt_all_identities(&mut self) -> Result<()> {
        let private_key = self.current_key.clone().ok_or(VaultError::NoCurrentUser)?;

        // Collect identity IDs and their allowed roles first
        let identities_to_update: Vec<(String, Vec<Role>)> = self
            .identities
            .values()
            .map(|id| (id.id.clone(), id.allowed_roles.clone()))
            .collect();

        for (identity_id, allowed_roles) in identities_to_update {
            // Get recipients for this identity
            let recipients = self.get_recipients_for_identity(&allowed_roles);

            if recipients.is_empty() {
                warn!(
                    "No recipients for identity {}, keeping old encryption",
                    identity_id
                );
                continue;
            }

            // Now mutate the identity
            if let Some(identity) = self.identities.get_mut(&identity_id) {
                let decrypted = decrypt_data(&identity.encrypted_data, &private_key)?;
                identity.encrypted_data = encrypt_data(&decrypted, &recipients)?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_vault_init() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".canaveral/signing");

        let (vault, keypair) = TeamVault::init("TestTeam", &path, "admin@example.com").unwrap();

        assert_eq!(vault.team_name(), "TestTeam");
        assert!(keypair.public_key.starts_with("age1"));
        assert!(path.join("vault.yaml").exists());
    }

    #[test]
    fn test_vault_open() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".canaveral/signing");

        let (_, keypair) = TeamVault::init("TestTeam", &path, "admin@example.com").unwrap();

        // Set env var so we can authenticate
        std::env::set_var("CANAVERAL_SIGNING_KEY", &keypair.private_key);

        let vault = TeamVault::open(&path).unwrap();
        assert_eq!(vault.team_name(), "TestTeam");
        assert!(vault.current_member().is_some());

        std::env::remove_var("CANAVERAL_SIGNING_KEY");
    }

    #[test]
    fn test_member_management() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(".canaveral/signing");

        let (mut vault, _) = TeamVault::init("TestTeam", &path, "admin@example.com").unwrap();

        // Add a member
        let new_keypair = generate_keypair();
        vault
            .add_member("dev@example.com", &new_keypair.public_key, Role::Signer)
            .unwrap();

        assert_eq!(vault.list_members().len(), 2);
        assert!(vault.get_member("dev@example.com").is_some());
    }
}
