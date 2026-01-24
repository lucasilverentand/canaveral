//! Team signing vault - secure credential management for teams
//!
//! This module provides a secure, git-friendly way to share signing credentials
//! across a development team. Credentials are encrypted using Age encryption
//! with each team member's public key.
//!
//! ## Features
//!
//! - **Encrypted storage**: All sensitive data is encrypted at rest
//! - **Role-based access**: Admin, Signer, and Viewer roles
//! - **Audit logging**: Track all signing operations
//! - **Git-friendly**: Vault files can be committed to version control
//! - **Offline-first**: Works without network access
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_signing::team::{TeamVault, Role};
//!
//! // Initialize a new vault
//! let vault = TeamVault::init("MyCompany", "/path/to/repo/.canaveral/signing")?;
//!
//! // Add a team member
//! vault.add_member("dev@company.com", "age1...", Role::Signer)?;
//!
//! // Import a signing identity
//! vault.import_identity("apple-dist", "/path/to/cert.p12", "password")?;
//! ```

mod audit;
mod encryption;
mod roles;
mod types;
mod vault;

pub use audit::{AuditEntry, AuditLog, AuditAction};
pub use encryption::{decrypt_data, encrypt_data, generate_keypair, KeyPair};
pub use roles::{Permission, Role, RolePermissions};
pub use types::{CredentialData, Member, StoredIdentity, VaultConfig, VaultMetadata};
pub use vault::{TeamVault, VaultError};
