//! Audit logging for the team vault

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

/// An action that can be audited
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    /// Vault was initialized
    VaultInit,
    /// Vault settings were modified
    VaultModify,

    /// Member was added
    MemberAdd { email: String, role: String },
    /// Member was removed
    MemberRemove { email: String },
    /// Member role was changed
    MemberRoleChange {
        email: String,
        old_role: String,
        new_role: String,
    },

    /// Identity was imported
    IdentityImport {
        identity_id: String,
        identity_type: String,
    },
    /// Identity was exported
    IdentityExport { identity_id: String },
    /// Identity was deleted
    IdentityDelete { identity_id: String },
    /// Identity was used for signing
    IdentitySign {
        identity_id: String,
        artifact: String,
    },

    /// Vault was synced
    VaultSync,

    /// Custom action
    Custom {
        action: String,
        details: Option<String>,
    },
}

impl std::fmt::Display for AuditAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditAction::VaultInit => write!(f, "Vault initialized"),
            AuditAction::VaultModify => write!(f, "Vault settings modified"),
            AuditAction::MemberAdd { email, role } => {
                write!(f, "Member added: {} ({})", email, role)
            }
            AuditAction::MemberRemove { email } => write!(f, "Member removed: {}", email),
            AuditAction::MemberRoleChange {
                email,
                old_role,
                new_role,
            } => {
                write!(
                    f,
                    "Role changed for {}: {} -> {}",
                    email, old_role, new_role
                )
            }
            AuditAction::IdentityImport {
                identity_id,
                identity_type,
            } => {
                write!(f, "Identity imported: {} ({})", identity_id, identity_type)
            }
            AuditAction::IdentityExport { identity_id } => {
                write!(f, "Identity exported: {}", identity_id)
            }
            AuditAction::IdentityDelete { identity_id } => {
                write!(f, "Identity deleted: {}", identity_id)
            }
            AuditAction::IdentitySign {
                identity_id,
                artifact,
            } => {
                write!(f, "Signed {} with {}", artifact, identity_id)
            }
            AuditAction::VaultSync => write!(f, "Vault synced"),
            AuditAction::Custom { action, details } => {
                if let Some(d) = details {
                    write!(f, "{}: {}", action, d)
                } else {
                    write!(f, "{}", action)
                }
            }
        }
    }
}

/// A single audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: Uuid,

    /// When the action occurred
    pub timestamp: DateTime<Utc>,

    /// Who performed the action (member email)
    pub actor: String,

    /// What action was performed
    pub action: AuditAction,

    /// Machine/hostname where action occurred
    pub machine: Option<String>,

    /// Hash of previous entry for integrity chain
    pub prev_hash: Option<String>,

    /// Hash of this entry
    pub hash: String,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(actor: impl Into<String>, action: AuditAction, prev_hash: Option<String>) -> Self {
        let id = Uuid::new_v4();
        let timestamp = Utc::now();
        let actor = actor.into();
        let machine = hostname::get()
            .ok()
            .map(|h| h.to_string_lossy().to_string());

        // Compute hash
        let hash_input = format!(
            "{}:{}:{}:{:?}:{}",
            id,
            timestamp.timestamp_nanos_opt().unwrap_or(0),
            actor,
            action,
            prev_hash.as_deref().unwrap_or("")
        );
        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let hash = format!("{:x}", hasher.finalize());

        Self {
            id,
            timestamp,
            actor,
            action,
            machine,
            prev_hash,
            hash,
        }
    }

    /// Verify the hash of this entry
    pub fn verify_hash(&self) -> bool {
        let hash_input = format!(
            "{}:{}:{}:{:?}:{}",
            self.id,
            self.timestamp.timestamp_nanos_opt().unwrap_or(0),
            self.actor,
            self.action,
            self.prev_hash.as_deref().unwrap_or("")
        );
        let mut hasher = Sha256::new();
        hasher.update(hash_input.as_bytes());
        let computed = format!("{:x}", hasher.finalize());
        computed == self.hash
    }
}

/// Audit log containing multiple entries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditLog {
    /// Schema version
    pub version: u32,

    /// Log entries
    pub entries: Vec<AuditEntry>,
}

impl AuditLog {
    /// Create a new empty audit log
    pub fn new() -> Self {
        Self {
            version: 1,
            entries: Vec::new(),
        }
    }

    /// Add an entry to the log
    pub fn add(&mut self, actor: impl Into<String>, action: AuditAction) {
        let prev_hash = self.entries.last().map(|e| e.hash.clone());
        let entry = AuditEntry::new(actor, action, prev_hash);
        self.entries.push(entry);
    }

    /// Get entries within a time range
    pub fn entries_between(&self, start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| e.timestamp >= start && e.timestamp <= end)
            .collect()
    }

    /// Get entries for a specific actor
    pub fn entries_by_actor(&self, actor: &str) -> Vec<&AuditEntry> {
        self.entries.iter().filter(|e| e.actor == actor).collect()
    }

    /// Get entries for a specific identity
    pub fn entries_for_identity(&self, identity_id: &str) -> Vec<&AuditEntry> {
        self.entries
            .iter()
            .filter(|e| match &e.action {
                AuditAction::IdentityImport {
                    identity_id: id, ..
                }
                | AuditAction::IdentityExport { identity_id: id }
                | AuditAction::IdentityDelete { identity_id: id }
                | AuditAction::IdentitySign {
                    identity_id: id, ..
                } => id == identity_id,
                _ => false,
            })
            .collect()
    }

    /// Get the last N entries
    pub fn last_n(&self, n: usize) -> Vec<&AuditEntry> {
        self.entries.iter().rev().take(n).collect()
    }

    /// Verify the integrity of the entire log
    pub fn verify_integrity(&self) -> Result<(), AuditIntegrityError> {
        let mut prev_hash: Option<String> = None;

        for (i, entry) in self.entries.iter().enumerate() {
            // Verify entry's own hash
            if !entry.verify_hash() {
                return Err(AuditIntegrityError::InvalidHash { index: i });
            }

            // Verify chain
            if entry.prev_hash != prev_hash {
                return Err(AuditIntegrityError::BrokenChain { index: i });
            }

            prev_hash = Some(entry.hash.clone());
        }

        Ok(())
    }

    /// Prune entries older than a certain date
    pub fn prune_before(&mut self, date: DateTime<Utc>) -> usize {
        let before_count = self.entries.len();
        self.entries.retain(|e| e.timestamp >= date);
        before_count - self.entries.len()
    }

    /// Load from a file
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        let content = std::fs::read_to_string(path)?;
        serde_yaml::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    /// Save to a file
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, content)
    }
}

/// Errors when verifying audit log integrity
#[derive(Debug, thiserror::Error)]
pub enum AuditIntegrityError {
    #[error("Invalid hash at entry index {index}")]
    InvalidHash { index: usize },

    #[error("Broken chain at entry index {index}")]
    BrokenChain { index: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_hash() {
        let entry = AuditEntry::new("test@example.com", AuditAction::VaultInit, None);
        assert!(entry.verify_hash());
    }

    #[test]
    fn test_audit_log_chain() {
        let mut log = AuditLog::new();

        log.add("user1@example.com", AuditAction::VaultInit);
        log.add(
            "user1@example.com",
            AuditAction::MemberAdd {
                email: "user2@example.com".to_string(),
                role: "signer".to_string(),
            },
        );
        log.add(
            "user2@example.com",
            AuditAction::IdentitySign {
                identity_id: "apple-dist".to_string(),
                artifact: "MyApp.app".to_string(),
            },
        );

        assert!(log.verify_integrity().is_ok());
    }

    #[test]
    fn test_audit_log_tampering() {
        let mut log = AuditLog::new();
        log.add("user@example.com", AuditAction::VaultInit);
        log.add("user@example.com", AuditAction::VaultModify);

        // Tamper with an entry
        log.entries[0].actor = "hacker@evil.com".to_string();

        // Integrity check should fail
        assert!(log.verify_integrity().is_err());
    }

    #[test]
    fn test_audit_queries() {
        let mut log = AuditLog::new();
        log.add("user1@example.com", AuditAction::VaultInit);
        log.add(
            "user2@example.com",
            AuditAction::IdentitySign {
                identity_id: "cert1".to_string(),
                artifact: "app1.app".to_string(),
            },
        );
        log.add(
            "user1@example.com",
            AuditAction::IdentitySign {
                identity_id: "cert1".to_string(),
                artifact: "app2.app".to_string(),
            },
        );

        assert_eq!(log.entries_by_actor("user1@example.com").len(), 2);
        assert_eq!(log.entries_for_identity("cert1").len(), 2);
    }
}
