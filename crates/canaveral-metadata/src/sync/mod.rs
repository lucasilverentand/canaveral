//! Metadata synchronization with app stores.
//!
//! This module provides functionality for syncing app store metadata
//! between local storage and remote app stores (Apple App Store Connect,
//! Google Play Console).
//!
//! ## Example
//!
//! ```no_run
//! use canaveral_metadata::sync::{AppleMetadataSync, AppleSyncConfig, MetadataSync};
//! use std::path::PathBuf;
//!
//! # async fn example() -> canaveral_metadata::Result<()> {
//! let config = AppleSyncConfig {
//!     api_key_id: "XXXXXXXXXX".to_string(),
//!     api_issuer_id: "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx".to_string(),
//!     api_private_key: std::fs::read_to_string("AuthKey_XXXXXXXXXX.p8")?,
//!     team_id: None,
//! };
//!
//! let sync = AppleMetadataSync::new(config, PathBuf::from("metadata")).await?;
//!
//! // Pull metadata from App Store Connect
//! sync.pull("com.example.app", None).await?;
//!
//! // Check for differences
//! let diff = sync.diff("com.example.app").await?;
//! for change in diff.changes {
//!     println!("{}: {} -> {:?}", change.locale, change.field, change.change_type);
//! }
//!
//! // Push local changes
//! let result = sync.push("com.example.app", None, false).await?;
//! println!("Updated {} locales", result.updated_locales.len());
//! # Ok(())
//! # }
//! ```

mod apple;
mod google_play;

pub use apple::{AppleMetadataSync, AppleSyncConfig};
pub use google_play::{
    image_types, GooglePlayMetadataSync, GooglePlaySyncConfig, Image, Listing, ListingUpdate,
};

use crate::{Locale, Result};
use async_trait::async_trait;
use std::fmt;

/// Metadata sync operations for app stores.
///
/// This trait defines the common interface for synchronizing metadata
/// between local storage and remote app stores.
#[async_trait]
pub trait MetadataSync: Send + Sync {
    /// Pull metadata from the app store to local storage.
    ///
    /// Downloads the current metadata from the remote app store and saves
    /// it to local storage, optionally filtering by locales.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The app identifier (bundle ID for Apple, package name for Google)
    /// * `locales` - Optional list of locales to pull. If None, pulls all available locales.
    ///
    /// # Errors
    ///
    /// Returns an error if the API request fails or metadata cannot be saved.
    async fn pull(&self, app_id: &str, locales: Option<&[Locale]>) -> Result<()>;

    /// Push metadata from local storage to the app store.
    ///
    /// Uploads local metadata changes to the remote app store.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The app identifier
    /// * `locales` - Optional list of locales to push. If None, pushes all locales.
    /// * `dry_run` - If true, only validates changes without actually pushing.
    ///
    /// # Returns
    ///
    /// A `PushResult` containing details about what was updated.
    ///
    /// # Errors
    ///
    /// Returns an error if validation fails or the API request fails.
    async fn push(
        &self,
        app_id: &str,
        locales: Option<&[Locale]>,
        dry_run: bool,
    ) -> Result<PushResult>;

    /// Get the diff between local and remote metadata.
    ///
    /// Compares local metadata with what's currently on the app store
    /// and returns a list of differences.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The app identifier
    ///
    /// # Returns
    ///
    /// A `MetadataDiff` containing all detected changes.
    async fn diff(&self, app_id: &str) -> Result<MetadataDiff>;
}

/// Result of a push operation.
#[derive(Debug, Clone, Default)]
pub struct PushResult {
    /// List of locale codes that were updated.
    pub updated_locales: Vec<String>,
    /// List of field names that were updated.
    pub updated_fields: Vec<String>,
    /// Number of screenshots that were uploaded.
    pub screenshots_uploaded: usize,
    /// Number of screenshots that were removed.
    pub screenshots_removed: usize,
    /// Any warnings that occurred during the push.
    pub warnings: Vec<String>,
}

impl PushResult {
    /// Returns true if any changes were made.
    pub fn has_changes(&self) -> bool {
        !self.updated_locales.is_empty()
            || !self.updated_fields.is_empty()
            || self.screenshots_uploaded > 0
            || self.screenshots_removed > 0
    }
}

impl fmt::Display for PushResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.has_changes() {
            return write!(f, "No changes pushed");
        }

        let mut parts = Vec::new();

        if !self.updated_locales.is_empty() {
            parts.push(format!("{} locale(s) updated", self.updated_locales.len()));
        }

        if !self.updated_fields.is_empty() {
            parts.push(format!("{} field(s) updated", self.updated_fields.len()));
        }

        if self.screenshots_uploaded > 0 {
            parts.push(format!(
                "{} screenshot(s) uploaded",
                self.screenshots_uploaded
            ));
        }

        if self.screenshots_removed > 0 {
            parts.push(format!(
                "{} screenshot(s) removed",
                self.screenshots_removed
            ));
        }

        write!(f, "{}", parts.join(", "))
    }
}

/// Diff between local and remote metadata.
#[derive(Debug, Clone, Default)]
pub struct MetadataDiff {
    /// List of individual changes detected.
    pub changes: Vec<MetadataChange>,
}

impl MetadataDiff {
    /// Returns true if there are any changes.
    pub fn has_changes(&self) -> bool {
        !self.changes.is_empty()
    }

    /// Returns the number of changes.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Returns true if there are no changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Filters changes by locale.
    pub fn for_locale(&self, locale: &str) -> Vec<&MetadataChange> {
        self.changes.iter().filter(|c| c.locale == locale).collect()
    }

    /// Filters changes by change type.
    pub fn by_type(&self, change_type: ChangeType) -> Vec<&MetadataChange> {
        self.changes
            .iter()
            .filter(|c| c.change_type == change_type)
            .collect()
    }

    /// Gets all unique locales with changes.
    pub fn affected_locales(&self) -> Vec<String> {
        let mut locales: Vec<String> = self.changes.iter().map(|c| c.locale.clone()).collect();
        locales.sort();
        locales.dedup();
        locales
    }
}

impl fmt::Display for MetadataDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return write!(f, "No differences found");
        }

        writeln!(f, "Found {} change(s):", self.len())?;

        for change in &self.changes {
            writeln!(f, "  {}", change)?;
        }

        Ok(())
    }
}

/// A single metadata change.
#[derive(Debug, Clone)]
pub struct MetadataChange {
    /// The locale for this change.
    pub locale: String,
    /// The field that changed.
    pub field: String,
    /// The local value (None if removed or not present locally).
    pub local_value: Option<String>,
    /// The remote value (None if added or not present remotely).
    pub remote_value: Option<String>,
    /// The type of change.
    pub change_type: ChangeType,
}

impl MetadataChange {
    /// Creates a new change indicating a field was added locally.
    pub fn added(locale: impl Into<String>, field: impl Into<String>, value: String) -> Self {
        Self {
            locale: locale.into(),
            field: field.into(),
            local_value: Some(value),
            remote_value: None,
            change_type: ChangeType::Added,
        }
    }

    /// Creates a new change indicating a field was modified.
    pub fn modified(
        locale: impl Into<String>,
        field: impl Into<String>,
        local: String,
        remote: String,
    ) -> Self {
        Self {
            locale: locale.into(),
            field: field.into(),
            local_value: Some(local),
            remote_value: Some(remote),
            change_type: ChangeType::Modified,
        }
    }

    /// Creates a new change indicating a field was removed locally.
    pub fn removed(locale: impl Into<String>, field: impl Into<String>, value: String) -> Self {
        Self {
            locale: locale.into(),
            field: field.into(),
            local_value: None,
            remote_value: Some(value),
            change_type: ChangeType::Removed,
        }
    }
}

impl fmt::Display for MetadataChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let symbol = match self.change_type {
            ChangeType::Added => "+",
            ChangeType::Modified => "~",
            ChangeType::Removed => "-",
        };

        write!(f, "[{}] {}/{}: ", symbol, self.locale, self.field)?;

        match self.change_type {
            ChangeType::Added => {
                write!(f, "(new) {:?}", self.local_value.as_deref().unwrap_or(""))
            }
            ChangeType::Modified => {
                let local = self.local_value.as_deref().unwrap_or("");
                let remote = self.remote_value.as_deref().unwrap_or("");
                // Truncate long values for display
                let local_display = if local.len() > 50 {
                    format!("{}...", &local[..50])
                } else {
                    local.to_string()
                };
                let remote_display = if remote.len() > 50 {
                    format!("{}...", &remote[..50])
                } else {
                    remote.to_string()
                };
                write!(f, "{:?} -> {:?}", remote_display, local_display)
            }
            ChangeType::Removed => {
                write!(
                    f,
                    "(removed) {:?}",
                    self.remote_value.as_deref().unwrap_or("")
                )
            }
        }
    }
}

/// Type of metadata change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChangeType {
    /// A new field/locale was added locally.
    Added,
    /// An existing field was modified.
    Modified,
    /// A field/locale was removed locally.
    Removed,
}

impl fmt::Display for ChangeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChangeType::Added => write!(f, "added"),
            ChangeType::Modified => write!(f, "modified"),
            ChangeType::Removed => write!(f, "removed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_push_result_display() {
        let empty = PushResult::default();
        assert_eq!(empty.to_string(), "No changes pushed");

        let result = PushResult {
            updated_locales: vec!["en-US".to_string(), "de-DE".to_string()],
            updated_fields: vec!["name".to_string()],
            screenshots_uploaded: 3,
            screenshots_removed: 1,
            warnings: vec![],
        };
        assert!(result.has_changes());
        let display = result.to_string();
        assert!(display.contains("2 locale(s) updated"));
        assert!(display.contains("3 screenshot(s) uploaded"));
    }

    #[test]
    fn test_metadata_diff() {
        let mut diff = MetadataDiff::default();
        assert!(diff.is_empty());
        assert!(!diff.has_changes());

        diff.changes
            .push(MetadataChange::added("en-US", "name", "My App".to_string()));
        diff.changes.push(MetadataChange::modified(
            "en-US",
            "description",
            "New desc".to_string(),
            "Old desc".to_string(),
        ));
        diff.changes.push(MetadataChange::removed(
            "de-DE",
            "subtitle",
            "Old subtitle".to_string(),
        ));

        assert_eq!(diff.len(), 3);
        assert!(diff.has_changes());

        let en_changes = diff.for_locale("en-US");
        assert_eq!(en_changes.len(), 2);

        let added = diff.by_type(ChangeType::Added);
        assert_eq!(added.len(), 1);

        let locales = diff.affected_locales();
        assert_eq!(locales.len(), 2);
        assert!(locales.contains(&"de-DE".to_string()));
        assert!(locales.contains(&"en-US".to_string()));
    }

    #[test]
    fn test_metadata_change_display() {
        let added = MetadataChange::added("en-US", "name", "My App".to_string());
        let display = added.to_string();
        assert!(display.contains("[+]"));
        assert!(display.contains("en-US/name"));

        let modified = MetadataChange::modified(
            "en-US",
            "description",
            "New description".to_string(),
            "Old description".to_string(),
        );
        let display = modified.to_string();
        assert!(display.contains("[~]"));

        let removed = MetadataChange::removed("de-DE", "subtitle", "Old subtitle".to_string());
        let display = removed.to_string();
        assert!(display.contains("[-]"));
        assert!(display.contains("(removed)"));
    }
}
