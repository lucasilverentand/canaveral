//! Provisioning profile installation and lifecycle management
//!
//! Manages profiles in `~/Library/MobileDevice/Provisioning Profiles/`,
//! the standard macOS location where Xcode looks for provisioning profiles.

use std::path::{Path, PathBuf};

use tracing::{debug, info, instrument, warn};

use crate::error::{Result, SigningError};
use crate::sync::ProfileType;

use super::parser::parse_mobileprovision;
use super::ProvisioningProfile;

/// Manages provisioning profile installation and discovery.
///
/// Profiles are stored in `~/Library/MobileDevice/Provisioning Profiles/`
/// using the UUID as the filename: `{UUID}.mobileprovision`.
pub struct ProfileManager {
    /// Directory where profiles are installed
    profiles_dir: PathBuf,
}

impl ProfileManager {
    /// Create a profile manager using the default system profiles directory.
    pub fn new() -> Self {
        let profiles_dir = dirs::home_dir()
            .map(|h| h.join("Library/MobileDevice/Provisioning Profiles"))
            .unwrap_or_else(|| PathBuf::from("/tmp/canaveral/profiles"));

        Self { profiles_dir }
    }

    /// Create a profile manager with a custom profiles directory (useful for testing).
    pub fn with_dir(profiles_dir: PathBuf) -> Self {
        Self { profiles_dir }
    }

    /// Get the profiles directory path.
    pub fn profiles_dir(&self) -> &Path {
        &self.profiles_dir
    }

    /// Install a provisioning profile from a file path.
    ///
    /// Parses the profile, copies it to the profiles directory using
    /// `{UUID}.mobileprovision` as the filename, and returns the parsed profile.
    #[instrument(skip(self), fields(path = %profile_path.display()))]
    pub fn install(&self, profile_path: &Path) -> Result<ProvisioningProfile> {
        // Parse the profile first to get its UUID
        let mut profile = parse_mobileprovision(profile_path)?;

        // Ensure the profiles directory exists
        std::fs::create_dir_all(&self.profiles_dir).map_err(SigningError::Io)?;

        // Copy to installation directory
        let dest = self
            .profiles_dir
            .join(format!("{}.mobileprovision", profile.uuid));
        std::fs::copy(profile_path, &dest).map_err(SigningError::Io)?;

        profile.path = Some(dest.clone());

        info!(
            uuid = %profile.uuid,
            name = %profile.name,
            dest = %dest.display(),
            "Installed provisioning profile"
        );

        Ok(profile)
    }

    /// Install a provisioning profile from raw bytes.
    ///
    /// Writes the data to a temporary file, parses it, then copies
    /// to the profiles directory.
    #[instrument(skip(self, data), fields(data_len = data.len()))]
    pub fn install_from_bytes(&self, data: &[u8]) -> Result<ProvisioningProfile> {
        // Write to a temporary file
        let temp_dir = tempfile::tempdir().map_err(SigningError::Io)?;
        let temp_path = temp_dir.path().join("profile.mobileprovision");
        std::fs::write(&temp_path, data).map_err(SigningError::Io)?;

        self.install(&temp_path)
    }

    /// Uninstall a provisioning profile by UUID.
    #[instrument(skip(self), fields(uuid = %uuid))]
    pub fn uninstall(&self, uuid: &str) -> Result<()> {
        let profile_path = self.profiles_dir.join(format!("{}.mobileprovision", uuid));

        if profile_path.exists() {
            std::fs::remove_file(&profile_path).map_err(SigningError::Io)?;
            info!(uuid = %uuid, "Uninstalled provisioning profile");
        } else {
            warn!(uuid = %uuid, "Profile not found for uninstall");
        }

        Ok(())
    }

    /// List all installed provisioning profiles.
    ///
    /// Reads and parses every `.mobileprovision` file in the profiles directory.
    /// Profiles that fail to parse are skipped with a warning.
    #[instrument(skip(self))]
    pub fn list_installed(&self) -> Result<Vec<ProvisioningProfile>> {
        let mut profiles = Vec::new();

        if !self.profiles_dir.exists() {
            debug!("Profiles directory does not exist, returning empty list");
            return Ok(profiles);
        }

        let entries = std::fs::read_dir(&self.profiles_dir).map_err(SigningError::Io)?;

        for entry in entries {
            let entry = entry.map_err(SigningError::Io)?;
            let path = entry.path();

            if path.extension().and_then(|e| e.to_str()) != Some("mobileprovision") {
                continue;
            }

            match parse_mobileprovision(&path) {
                Ok(profile) => profiles.push(profile),
                Err(e) => {
                    warn!(
                        path = %path.display(),
                        error = %e,
                        "Failed to parse installed profile, skipping"
                    );
                }
            }
        }

        debug!(count = profiles.len(), "Listed installed profiles");
        Ok(profiles)
    }

    /// Find an installed profile matching a bundle ID and profile type.
    ///
    /// Returns the first non-expired profile that matches. Prefers exact
    /// bundle ID matches over wildcard matches.
    #[instrument(skip(self), fields(bundle_id = %bundle_id, profile_type = %profile_type))]
    pub fn find_matching(
        &self,
        bundle_id: &str,
        profile_type: ProfileType,
    ) -> Result<Option<ProvisioningProfile>> {
        let profiles = self.list_installed()?;

        let mut exact_match: Option<ProvisioningProfile> = None;
        let mut wildcard_match: Option<ProvisioningProfile> = None;

        for profile in profiles {
            // Skip expired profiles
            if profile.is_expired() {
                continue;
            }

            // Skip wrong profile type
            if profile.profile_type != profile_type {
                continue;
            }

            if profile.bundle_id == bundle_id {
                // Exact match - prefer the one that expires latest
                if exact_match
                    .as_ref()
                    .map_or(true, |m| profile.expiration_date > m.expiration_date)
                {
                    exact_match = Some(profile);
                }
            } else if profile.matches_bundle_id(bundle_id) {
                // Wildcard match
                if wildcard_match
                    .as_ref()
                    .map_or(true, |m| profile.expiration_date > m.expiration_date)
                {
                    wildcard_match = Some(profile);
                }
            }
        }

        // Prefer exact match over wildcard
        Ok(exact_match.or(wildcard_match))
    }

    /// Remove all expired profiles and return the UUIDs of removed profiles.
    #[instrument(skip(self))]
    pub fn cleanup_expired(&self) -> Result<Vec<String>> {
        let profiles = self.list_installed()?;
        let mut removed = Vec::new();

        for profile in profiles {
            if profile.is_expired() {
                if let Some(path) = &profile.path {
                    if path.exists() {
                        std::fs::remove_file(path).map_err(SigningError::Io)?;
                        info!(
                            uuid = %profile.uuid,
                            name = %profile.name,
                            expired = %profile.expiration_date,
                            "Removed expired profile"
                        );
                        removed.push(profile.uuid.clone());
                    }
                }
            }
        }

        info!(count = removed.len(), "Cleaned up expired profiles");
        Ok(removed)
    }
}

impl Default for ProfileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profiles_dir_default() {
        let manager = ProfileManager::new();
        assert!(manager
            .profiles_dir()
            .to_string_lossy()
            .contains("Provisioning Profiles"));
    }

    #[test]
    fn test_profiles_dir_custom() {
        let custom = PathBuf::from("/tmp/test-profiles");
        let manager = ProfileManager::with_dir(custom.clone());
        assert_eq!(manager.profiles_dir(), custom);
    }

    #[test]
    fn test_list_installed_empty_dir() {
        let temp = tempfile::tempdir().unwrap();
        let manager = ProfileManager::with_dir(temp.path().to_path_buf());
        let profiles = manager.list_installed().unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_list_installed_nonexistent_dir() {
        let manager = ProfileManager::with_dir(PathBuf::from("/nonexistent/path"));
        let profiles = manager.list_installed().unwrap();
        assert!(profiles.is_empty());
    }

    #[test]
    fn test_uninstall_nonexistent() {
        let temp = tempfile::tempdir().unwrap();
        let manager = ProfileManager::with_dir(temp.path().to_path_buf());
        // Should not error, just warn
        manager.uninstall("nonexistent-uuid").unwrap();
    }

    #[test]
    fn test_cleanup_empty() {
        let temp = tempfile::tempdir().unwrap();
        let manager = ProfileManager::with_dir(temp.path().to_path_buf());
        let removed = manager.cleanup_expired().unwrap();
        assert!(removed.is_empty());
    }
}
