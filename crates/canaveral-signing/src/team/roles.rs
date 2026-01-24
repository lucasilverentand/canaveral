//! Role-based access control for the team vault

use serde::{Deserialize, Serialize};

/// Role in the team vault
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full access - can manage members, identities, and settings
    Admin,
    /// Can sign artifacts and view identities they have access to
    Signer,
    /// Can only view identity metadata, cannot sign or decrypt
    Viewer,
}

impl Role {
    /// Get the display name for this role
    pub fn display_name(&self) -> &'static str {
        match self {
            Role::Admin => "Admin",
            Role::Signer => "Signer",
            Role::Viewer => "Viewer",
        }
    }

    /// Get a description of this role
    pub fn description(&self) -> &'static str {
        match self {
            Role::Admin => "Full access to manage members, identities, and vault settings",
            Role::Signer => "Can sign artifacts using identities they have access to",
            Role::Viewer => "Can view identity metadata but cannot sign or access credentials",
        }
    }

    /// Check if this role has a specific permission
    pub fn has_permission(&self, permission: Permission) -> bool {
        RolePermissions::for_role(*self).has(permission)
    }

    /// Get all permissions for this role
    pub fn permissions(&self) -> RolePermissions {
        RolePermissions::for_role(*self)
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl std::str::FromStr for Role {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "admin" => Ok(Role::Admin),
            "signer" => Ok(Role::Signer),
            "viewer" | "read" | "readonly" => Ok(Role::Viewer),
            _ => Err(format!("Unknown role: {}. Valid roles: admin, signer, viewer", s)),
        }
    }
}

/// Specific permissions that can be checked
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Permission {
    /// View vault metadata and identity list
    ViewVault,
    /// View identity details (non-sensitive)
    ViewIdentities,
    /// Decrypt and use signing identities
    UseIdentities,
    /// Import new signing identities
    ImportIdentities,
    /// Export signing identities
    ExportIdentities,
    /// Delete signing identities
    DeleteIdentities,
    /// Add new team members
    AddMembers,
    /// Remove team members
    RemoveMembers,
    /// Change member roles
    ChangeRoles,
    /// View audit log
    ViewAudit,
    /// Modify vault settings
    ModifySettings,
    /// Initialize/destroy vault
    ManageVault,
}

impl Permission {
    /// Get all available permissions
    pub fn all() -> &'static [Permission] {
        &[
            Permission::ViewVault,
            Permission::ViewIdentities,
            Permission::UseIdentities,
            Permission::ImportIdentities,
            Permission::ExportIdentities,
            Permission::DeleteIdentities,
            Permission::AddMembers,
            Permission::RemoveMembers,
            Permission::ChangeRoles,
            Permission::ViewAudit,
            Permission::ModifySettings,
            Permission::ManageVault,
        ]
    }
}

impl std::fmt::Display for Permission {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Permission::ViewVault => "view_vault",
            Permission::ViewIdentities => "view_identities",
            Permission::UseIdentities => "use_identities",
            Permission::ImportIdentities => "import_identities",
            Permission::ExportIdentities => "export_identities",
            Permission::DeleteIdentities => "delete_identities",
            Permission::AddMembers => "add_members",
            Permission::RemoveMembers => "remove_members",
            Permission::ChangeRoles => "change_roles",
            Permission::ViewAudit => "view_audit",
            Permission::ModifySettings => "modify_settings",
            Permission::ManageVault => "manage_vault",
        };
        write!(f, "{}", name)
    }
}

/// Permission set for a role
#[derive(Debug, Clone)]
pub struct RolePermissions {
    permissions: Vec<Permission>,
}

impl RolePermissions {
    /// Create permissions for a specific role
    pub fn for_role(role: Role) -> Self {
        let permissions = match role {
            Role::Admin => Permission::all().to_vec(),
            Role::Signer => vec![
                Permission::ViewVault,
                Permission::ViewIdentities,
                Permission::UseIdentities,
                Permission::ViewAudit,
            ],
            Role::Viewer => vec![
                Permission::ViewVault,
                Permission::ViewIdentities,
                Permission::ViewAudit,
            ],
        };
        Self { permissions }
    }

    /// Check if this permission set includes a specific permission
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(&permission)
    }

    /// Get all permissions in this set
    pub fn list(&self) -> &[Permission] {
        &self.permissions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_role_parsing() {
        assert_eq!("admin".parse::<Role>().unwrap(), Role::Admin);
        assert_eq!("ADMIN".parse::<Role>().unwrap(), Role::Admin);
        assert_eq!("signer".parse::<Role>().unwrap(), Role::Signer);
        assert_eq!("viewer".parse::<Role>().unwrap(), Role::Viewer);
        assert_eq!("readonly".parse::<Role>().unwrap(), Role::Viewer);
    }

    #[test]
    fn test_admin_permissions() {
        let role = Role::Admin;
        assert!(role.has_permission(Permission::ManageVault));
        assert!(role.has_permission(Permission::UseIdentities));
        assert!(role.has_permission(Permission::AddMembers));
    }

    #[test]
    fn test_signer_permissions() {
        let role = Role::Signer;
        assert!(role.has_permission(Permission::ViewVault));
        assert!(role.has_permission(Permission::UseIdentities));
        assert!(!role.has_permission(Permission::AddMembers));
        assert!(!role.has_permission(Permission::ManageVault));
    }

    #[test]
    fn test_viewer_permissions() {
        let role = Role::Viewer;
        assert!(role.has_permission(Permission::ViewVault));
        assert!(role.has_permission(Permission::ViewIdentities));
        assert!(!role.has_permission(Permission::UseIdentities));
        assert!(!role.has_permission(Permission::AddMembers));
    }
}
