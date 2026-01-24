//! Storage backends for app store metadata.
//!
//! This module provides the [`MetadataStorage`] trait and implementations
//! for persisting and loading app store metadata.

mod fastlane;

pub use fastlane::FastlaneStorage;

use crate::{AppleMetadata, GooglePlayMetadata, Locale, Platform, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Storage backend for app store metadata.
///
/// This trait defines the interface for loading and saving metadata
/// for both Apple App Store and Google Play Store apps.
#[async_trait]
pub trait MetadataStorage: Send + Sync {
    /// Load Apple metadata for an app.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - The app's bundle identifier (e.g., "com.example.app")
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be loaded.
    async fn load_apple(&self, bundle_id: &str) -> Result<AppleMetadata>;

    /// Load Google Play metadata for an app.
    ///
    /// # Arguments
    ///
    /// * `package_name` - The app's package name (e.g., "com.example.app")
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be loaded.
    async fn load_google_play(&self, package_name: &str) -> Result<GooglePlayMetadata>;

    /// Save Apple metadata for an app.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata to save
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be saved.
    async fn save_apple(&self, metadata: &AppleMetadata) -> Result<()>;

    /// Save Google Play metadata for an app.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata to save
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be saved.
    async fn save_google_play(&self, metadata: &GooglePlayMetadata) -> Result<()>;

    /// Check if Apple metadata exists for an app.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - The app's bundle identifier
    async fn exists_apple(&self, bundle_id: &str) -> Result<bool>;

    /// Check if Google Play metadata exists for an app.
    ///
    /// # Arguments
    ///
    /// * `package_name` - The app's package name
    async fn exists_google_play(&self, package_name: &str) -> Result<bool>;

    /// List available locales for an Apple app.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - The app's bundle identifier
    async fn list_locales_apple(&self, bundle_id: &str) -> Result<Vec<Locale>>;

    /// List available locales for a Google Play app.
    ///
    /// # Arguments
    ///
    /// * `package_name` - The app's package name
    async fn list_locales_google_play(&self, package_name: &str) -> Result<Vec<Locale>>;

    /// Get the base path for Apple metadata.
    ///
    /// # Arguments
    ///
    /// * `bundle_id` - The app's bundle identifier
    fn apple_path(&self, bundle_id: &str) -> PathBuf;

    /// Get the base path for Google Play metadata.
    ///
    /// # Arguments
    ///
    /// * `package_name` - The app's package name
    fn google_play_path(&self, package_name: &str) -> PathBuf;

    /// Initialize directory structure for a new app.
    ///
    /// Creates the necessary directories and template files for storing
    /// metadata for a new app.
    ///
    /// # Arguments
    ///
    /// * `platform` - The target platform
    /// * `app_id` - The app identifier (bundle_id or package_name)
    /// * `locales` - The locales to initialize
    async fn init(&self, platform: Platform, app_id: &str, locales: &[Locale]) -> Result<()>;

    /// Add a new locale, optionally copying from an existing one.
    ///
    /// # Arguments
    ///
    /// * `platform` - The target platform
    /// * `app_id` - The app identifier (bundle_id or package_name)
    /// * `locale` - The new locale to add
    /// * `copy_from` - Optional source locale to copy content from
    async fn add_locale(
        &self,
        platform: Platform,
        app_id: &str,
        locale: &Locale,
        copy_from: Option<&Locale>,
    ) -> Result<()>;

    /// Remove a locale and its directory.
    ///
    /// # Arguments
    ///
    /// * `platform` - The target platform
    /// * `app_id` - The app identifier (bundle_id or package_name)
    /// * `locale` - The locale to remove
    async fn remove_locale(&self, platform: Platform, app_id: &str, locale: &Locale)
        -> Result<()>;
}

/// Storage format type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StorageFormat {
    /// Fastlane-compatible directory structure with individual text files.
    #[default]
    Fastlane,
    /// Unified JSON/YAML format (future).
    Unified,
}
