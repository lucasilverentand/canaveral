//! Fastlane-compatible storage backend.
//!
//! This module implements a storage backend that uses a directory structure
//! compatible with Fastlane's deliver and supply tools.
//!
//! ## Directory Structure
//!
//! ```text
//! metadata/
//! ├── apple/
//! │   └── {bundle_id}/
//! │       ├── {locale}/
//! │       │   ├── name.txt
//! │       │   ├── subtitle.txt
//! │       │   ├── description.txt
//! │       │   ├── keywords.txt
//! │       │   ├── release_notes.txt
//! │       │   ├── promotional_text.txt
//! │       │   ├── support_url.txt
//! │       │   ├── marketing_url.txt
//! │       │   └── privacy_url.txt
//! │       ├── screenshots/
//! │       │   └── {locale}/
//! │       │       └── *.png
//! │       └── app_store_info.yaml
//! └── google_play/
//!     └── {package_name}/
//!         ├── {locale}/
//!         │   ├── title.txt
//!         │   ├── short_description.txt
//!         │   ├── full_description.txt
//!         │   └── changelogs/
//!         │       └── {version_code}.txt
//!         ├── screenshots/
//!         │   └── {locale}/
//!         │       ├── phone/
//!         │       ├── tablet/
//!         │       └── ...
//!         └── store_info.yaml
//! ```

use super::MetadataStorage;
use crate::{
    AppleAgeRating, AppleCategory, AppleLocalizedMetadata, AppleMetadata, AppleScreenshotSet,
    AssetType, GooglePlayCategory, GooglePlayContentRating, GooglePlayLocalizedMetadata,
    GooglePlayMetadata, GooglePlayScreenshotSet, Locale, MediaAsset, MetadataError, Platform,
    Result,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};

/// Fastlane-compatible storage backend.
///
/// This storage backend uses a directory structure compatible with
/// Fastlane's deliver (for App Store) and supply (for Google Play) tools.
#[derive(Debug, Clone)]
pub struct FastlaneStorage {
    /// Base path for all metadata.
    base_path: PathBuf,
}

impl FastlaneStorage {
    /// Creates a new Fastlane storage backend.
    ///
    /// # Arguments
    ///
    /// * `base_path` - The base directory for metadata storage (typically "metadata/")
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Gets the base path for this storage backend.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    // ========================================================================
    // Apple-specific helpers
    // ========================================================================

    /// Read a text file, returning None if it doesn't exist.
    async fn read_text_file(&self, path: &Path) -> Result<Option<String>> {
        match fs::read_to_string(path).await {
            Ok(content) => Ok(Some(content.trim().to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(MetadataError::Io(e)),
        }
    }

    /// Write a text file, creating parent directories as needed.
    async fn write_text_file(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        fs::write(path, content).await?;
        Ok(())
    }

    /// Read a YAML file, returning default if it doesn't exist.
    async fn read_yaml_file<T: for<'de> Deserialize<'de> + Default>(
        &self,
        path: &Path,
    ) -> Result<T> {
        match fs::read_to_string(path).await {
            Ok(content) => {
                let value: T = serde_yaml::from_str(&content)?;
                Ok(value)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(T::default()),
            Err(e) => Err(MetadataError::Io(e)),
        }
    }

    /// Write a YAML file, creating parent directories as needed.
    async fn write_yaml_file<T: Serialize>(&self, path: &Path, value: &T) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        let content = serde_yaml::to_string(value)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Load Apple localized metadata for a specific locale.
    async fn load_apple_locale(
        &self,
        bundle_id: &str,
        locale: &Locale,
    ) -> Result<AppleLocalizedMetadata> {
        let locale_path = self.apple_path(bundle_id).join(locale.code());

        let name = self
            .read_text_file(&locale_path.join("name.txt"))
            .await?
            .unwrap_or_default();

        let description = self
            .read_text_file(&locale_path.join("description.txt"))
            .await?
            .unwrap_or_default();

        let subtitle = self
            .read_text_file(&locale_path.join("subtitle.txt"))
            .await?;

        let keywords = self
            .read_text_file(&locale_path.join("keywords.txt"))
            .await?;

        let whats_new = self
            .read_text_file(&locale_path.join("release_notes.txt"))
            .await?;

        let promotional_text = self
            .read_text_file(&locale_path.join("promotional_text.txt"))
            .await?;

        let support_url = self
            .read_text_file(&locale_path.join("support_url.txt"))
            .await?;

        let marketing_url = self
            .read_text_file(&locale_path.join("marketing_url.txt"))
            .await?;

        let privacy_policy_url = self
            .read_text_file(&locale_path.join("privacy_url.txt"))
            .await?;

        Ok(AppleLocalizedMetadata {
            name,
            subtitle,
            description,
            keywords,
            whats_new,
            promotional_text,
            support_url,
            marketing_url,
            privacy_policy_url,
        })
    }

    /// Save Apple localized metadata for a specific locale.
    async fn save_apple_locale(
        &self,
        bundle_id: &str,
        locale: &str,
        metadata: &AppleLocalizedMetadata,
    ) -> Result<()> {
        let locale_path = self.apple_path(bundle_id).join(locale);

        // Always write required fields
        self.write_text_file(&locale_path.join("name.txt"), &metadata.name)
            .await?;
        self.write_text_file(&locale_path.join("description.txt"), &metadata.description)
            .await?;

        // Write optional fields if present
        if let Some(ref subtitle) = metadata.subtitle {
            self.write_text_file(&locale_path.join("subtitle.txt"), subtitle)
                .await?;
        }

        if let Some(ref keywords) = metadata.keywords {
            self.write_text_file(&locale_path.join("keywords.txt"), keywords)
                .await?;
        }

        if let Some(ref whats_new) = metadata.whats_new {
            self.write_text_file(&locale_path.join("release_notes.txt"), whats_new)
                .await?;
        }

        if let Some(ref promotional_text) = metadata.promotional_text {
            self.write_text_file(&locale_path.join("promotional_text.txt"), promotional_text)
                .await?;
        }

        if let Some(ref support_url) = metadata.support_url {
            self.write_text_file(&locale_path.join("support_url.txt"), support_url)
                .await?;
        }

        if let Some(ref marketing_url) = metadata.marketing_url {
            self.write_text_file(&locale_path.join("marketing_url.txt"), marketing_url)
                .await?;
        }

        if let Some(ref privacy_policy_url) = metadata.privacy_policy_url {
            self.write_text_file(&locale_path.join("privacy_url.txt"), privacy_policy_url)
                .await?;
        }

        Ok(())
    }

    /// Load screenshots for Apple app.
    async fn load_apple_screenshots(&self, bundle_id: &str) -> Result<AppleScreenshotSet> {
        let screenshots_path = self.apple_path(bundle_id).join("screenshots");

        // For now, return empty screenshot set - full implementation would scan directories
        // and categorize by device type based on dimensions or directory names
        if !screenshots_path.exists() {
            return Ok(AppleScreenshotSet::default());
        }

        // This is a simplified implementation - a full implementation would:
        // 1. Scan each locale directory under screenshots/
        // 2. Categorize images by device type based on dimensions or naming conventions
        // 3. Create MediaAsset entries with proper paths and metadata

        debug!("Loading Apple screenshots from {:?}", screenshots_path);

        Ok(AppleScreenshotSet::default())
    }

    // ========================================================================
    // Google Play-specific helpers
    // ========================================================================

    /// Load Google Play localized metadata for a specific locale.
    async fn load_google_play_locale(
        &self,
        package_name: &str,
        locale: &Locale,
    ) -> Result<GooglePlayLocalizedMetadata> {
        let locale_path = self.google_play_path(package_name).join(locale.code());

        let title = self
            .read_text_file(&locale_path.join("title.txt"))
            .await?
            .unwrap_or_default();

        let short_description = self
            .read_text_file(&locale_path.join("short_description.txt"))
            .await?
            .unwrap_or_default();

        let full_description = self
            .read_text_file(&locale_path.join("full_description.txt"))
            .await?
            .unwrap_or_default();

        let video_url = self
            .read_text_file(&locale_path.join("video.txt"))
            .await?;

        // Load changelogs
        let changelogs = self.load_changelogs(&locale_path).await?;

        Ok(GooglePlayLocalizedMetadata {
            title,
            short_description,
            full_description,
            changelogs,
            video_url,
        })
    }

    /// Load changelog files for a locale.
    async fn load_changelogs(&self, locale_path: &Path) -> Result<HashMap<String, String>> {
        let changelogs_path = locale_path.join("changelogs");
        let mut changelogs = HashMap::new();

        if !changelogs_path.exists() {
            return Ok(changelogs);
        }

        let mut entries = match fs::read_dir(&changelogs_path).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(changelogs),
            Err(e) => return Err(MetadataError::Io(e)),
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("txt") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    if let Some(content) = self.read_text_file(&path).await? {
                        changelogs.insert(stem.to_string(), content);
                    }
                }
            }
        }

        Ok(changelogs)
    }

    /// Save Google Play localized metadata for a specific locale.
    async fn save_google_play_locale(
        &self,
        package_name: &str,
        locale: &str,
        metadata: &GooglePlayLocalizedMetadata,
    ) -> Result<()> {
        let locale_path = self.google_play_path(package_name).join(locale);

        // Write required fields
        self.write_text_file(&locale_path.join("title.txt"), &metadata.title)
            .await?;
        self.write_text_file(
            &locale_path.join("short_description.txt"),
            &metadata.short_description,
        )
        .await?;
        self.write_text_file(
            &locale_path.join("full_description.txt"),
            &metadata.full_description,
        )
        .await?;

        // Write optional video URL
        if let Some(ref video_url) = metadata.video_url {
            self.write_text_file(&locale_path.join("video.txt"), video_url)
                .await?;
        }

        // Write changelogs
        let changelogs_path = locale_path.join("changelogs");
        for (version_code, changelog) in &metadata.changelogs {
            self.write_text_file(&changelogs_path.join(format!("{}.txt", version_code)), changelog)
                .await?;
        }

        Ok(())
    }

    /// Load screenshots for Google Play app.
    async fn load_google_play_screenshots(
        &self,
        package_name: &str,
    ) -> Result<GooglePlayScreenshotSet> {
        let screenshots_path = self.google_play_path(package_name).join("screenshots");

        if !screenshots_path.exists() {
            return Ok(GooglePlayScreenshotSet::default());
        }

        // Simplified implementation - would scan locale directories and device type subdirs
        debug!(
            "Loading Google Play screenshots from {:?}",
            screenshots_path
        );

        Ok(GooglePlayScreenshotSet::default())
    }

    /// Discover screenshots in a directory.
    ///
    /// Scans the given directory for image files (PNG, JPG, JPEG)
    /// and returns them as `MediaAsset` entries sorted by filename.
    ///
    /// # Arguments
    ///
    /// * `dir` - The directory to scan for screenshots
    pub async fn discover_screenshots(&self, dir: &Path) -> Result<Vec<MediaAsset>> {
        let mut screenshots = Vec::new();

        if !dir.exists() {
            return Ok(screenshots);
        }

        let mut entries = match fs::read_dir(dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(screenshots),
            Err(e) => return Err(MetadataError::Io(e)),
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
                let ext_lower = ext.to_lowercase();
                if ext_lower == "png" || ext_lower == "jpg" || ext_lower == "jpeg" {
                    screenshots.push(MediaAsset::new(path, AssetType::Screenshot));
                }
            }
        }

        // Sort by filename for consistent ordering
        screenshots.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(screenshots)
    }

    /// List locale directories under a path.
    async fn list_locale_directories(&self, base_path: &Path) -> Result<Vec<Locale>> {
        let mut locales = Vec::new();

        if !base_path.exists() {
            return Ok(locales);
        }

        let mut entries = fs::read_dir(base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    // Skip non-locale directories
                    if name == "screenshots" || name.starts_with('.') {
                        continue;
                    }

                    // Try to parse as locale
                    match Locale::new(name) {
                        Ok(locale) => locales.push(locale),
                        Err(e) => {
                            warn!("Skipping invalid locale directory '{}': {}", name, e);
                        }
                    }
                }
            }
        }

        Ok(locales)
    }
}

/// Non-localized Apple App Store info stored in YAML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct AppleStoreInfo {
    /// Primary locale code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_locale: Option<String>,
    /// App category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<AppleCategory>,
    /// Secondary category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary_category: Option<AppleCategory>,
    /// Age rating configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<AppleAgeRating>,
    /// Copyright text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
    /// Privacy policy URL (app-level, not locale-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
    /// Support URL (app-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_url: Option<String>,
    /// Marketing URL (app-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing_url: Option<String>,
}

/// Non-localized Google Play Store info stored in YAML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct GooglePlayStoreInfo {
    /// Default locale code.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_locale: Option<String>,
    /// App category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<GooglePlayCategory>,
    /// Content rating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_rating: Option<GooglePlayContentRating>,
    /// Privacy policy URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
    /// Contact email.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_email: Option<String>,
    /// Contact phone.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_phone: Option<String>,
    /// Contact website.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contact_website: Option<String>,
}

#[async_trait]
impl MetadataStorage for FastlaneStorage {
    async fn load_apple(&self, bundle_id: &str) -> Result<AppleMetadata> {
        let app_path = self.apple_path(bundle_id);

        if !app_path.exists() {
            return Err(MetadataError::NotFound(format!(
                "Apple metadata not found for bundle_id: {}",
                bundle_id
            )));
        }

        // Load app store info
        let store_info: AppleStoreInfo = self
            .read_yaml_file(&app_path.join("app_store_info.yaml"))
            .await?;

        // Determine primary locale
        let primary_locale = store_info
            .primary_locale
            .as_ref()
            .and_then(|s| Locale::new(s).ok())
            .unwrap_or_default();

        // Load localizations
        let locales = self.list_locales_apple(bundle_id).await?;
        let mut localizations = HashMap::new();

        for locale in &locales {
            let localized = self.load_apple_locale(bundle_id, locale).await?;
            localizations.insert(locale.code(), localized);
        }

        // Load screenshots
        let screenshots = self.load_apple_screenshots(bundle_id).await?;

        Ok(AppleMetadata {
            bundle_id: bundle_id.to_string(),
            primary_locale,
            localizations,
            category: store_info.category,
            secondary_category: store_info.secondary_category,
            age_rating: store_info.age_rating,
            screenshots,
            previews: HashMap::new(),
            icon: None,
            privacy_policy_url: store_info.privacy_policy_url,
            support_url: store_info.support_url,
            marketing_url: store_info.marketing_url,
            copyright: store_info.copyright,
        })
    }

    async fn load_google_play(&self, package_name: &str) -> Result<GooglePlayMetadata> {
        let app_path = self.google_play_path(package_name);

        if !app_path.exists() {
            return Err(MetadataError::NotFound(format!(
                "Google Play metadata not found for package: {}",
                package_name
            )));
        }

        // Load store info
        let store_info: GooglePlayStoreInfo = self
            .read_yaml_file(&app_path.join("store_info.yaml"))
            .await?;

        // Determine default locale
        let default_locale = store_info
            .default_locale
            .as_ref()
            .and_then(|s| Locale::new(s).ok())
            .unwrap_or_default();

        // Load localizations
        let locales = self.list_locales_google_play(package_name).await?;
        let mut localizations = HashMap::new();

        for locale in &locales {
            let localized = self.load_google_play_locale(package_name, locale).await?;
            localizations.insert(locale.code(), localized);
        }

        // Load screenshots
        let screenshots = self.load_google_play_screenshots(package_name).await?;

        Ok(GooglePlayMetadata {
            package_name: package_name.to_string(),
            default_locale,
            localizations,
            category: store_info.category,
            content_rating: store_info.content_rating,
            screenshots,
            feature_graphic: None,
            promo_graphic: None,
            tv_banner: None,
            icon: None,
            privacy_policy_url: store_info.privacy_policy_url,
            contact_email: store_info.contact_email,
            contact_phone: store_info.contact_phone,
            contact_website: store_info.contact_website,
        })
    }

    async fn save_apple(&self, metadata: &AppleMetadata) -> Result<()> {
        let app_path = self.apple_path(&metadata.bundle_id);

        // Save app store info
        let store_info = AppleStoreInfo {
            primary_locale: Some(metadata.primary_locale.code()),
            category: metadata.category,
            secondary_category: metadata.secondary_category,
            age_rating: metadata.age_rating.clone(),
            copyright: metadata.copyright.clone(),
            privacy_policy_url: metadata.privacy_policy_url.clone(),
            support_url: metadata.support_url.clone(),
            marketing_url: metadata.marketing_url.clone(),
        };

        self.write_yaml_file(&app_path.join("app_store_info.yaml"), &store_info)
            .await?;

        // Save localizations
        for (locale, localized) in &metadata.localizations {
            self.save_apple_locale(&metadata.bundle_id, locale, localized)
                .await?;
        }

        Ok(())
    }

    async fn save_google_play(&self, metadata: &GooglePlayMetadata) -> Result<()> {
        let app_path = self.google_play_path(&metadata.package_name);

        // Save store info
        let store_info = GooglePlayStoreInfo {
            default_locale: Some(metadata.default_locale.code()),
            category: metadata.category,
            content_rating: metadata.content_rating.clone(),
            privacy_policy_url: metadata.privacy_policy_url.clone(),
            contact_email: metadata.contact_email.clone(),
            contact_phone: metadata.contact_phone.clone(),
            contact_website: metadata.contact_website.clone(),
        };

        self.write_yaml_file(&app_path.join("store_info.yaml"), &store_info)
            .await?;

        // Save localizations
        for (locale, localized) in &metadata.localizations {
            self.save_google_play_locale(&metadata.package_name, locale, localized)
                .await?;
        }

        Ok(())
    }

    async fn exists_apple(&self, bundle_id: &str) -> Result<bool> {
        let path = self.apple_path(bundle_id);
        Ok(path.exists())
    }

    async fn exists_google_play(&self, package_name: &str) -> Result<bool> {
        let path = self.google_play_path(package_name);
        Ok(path.exists())
    }

    async fn list_locales_apple(&self, bundle_id: &str) -> Result<Vec<Locale>> {
        let app_path = self.apple_path(bundle_id);
        self.list_locale_directories(&app_path).await
    }

    async fn list_locales_google_play(&self, package_name: &str) -> Result<Vec<Locale>> {
        let app_path = self.google_play_path(package_name);
        self.list_locale_directories(&app_path).await
    }

    fn apple_path(&self, bundle_id: &str) -> PathBuf {
        self.base_path.join("apple").join(bundle_id)
    }

    fn google_play_path(&self, package_name: &str) -> PathBuf {
        self.base_path.join("google_play").join(package_name)
    }

    async fn init(&self, platform: Platform, app_id: &str, locales: &[Locale]) -> Result<()> {
        match platform {
            Platform::Apple => {
                let app_path = self.apple_path(app_id);

                // Create locale directories with template files
                for locale in locales {
                    let locale_path = app_path.join(locale.code());
                    fs::create_dir_all(&locale_path).await?;

                    // Create empty template files
                    self.write_text_file(&locale_path.join("name.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("subtitle.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("description.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("keywords.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("release_notes.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("promotional_text.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("support_url.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("marketing_url.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("privacy_url.txt"), "")
                        .await?;
                }

                // Create screenshots directory
                let screenshots_path = app_path.join("screenshots");
                for locale in locales {
                    fs::create_dir_all(screenshots_path.join(locale.code())).await?;
                }

                // Create default app store info
                let primary_locale = locales.first().cloned().unwrap_or_default();
                let store_info = AppleStoreInfo {
                    primary_locale: Some(primary_locale.code()),
                    ..Default::default()
                };
                self.write_yaml_file(&app_path.join("app_store_info.yaml"), &store_info)
                    .await?;
            }
            Platform::GooglePlay => {
                let app_path = self.google_play_path(app_id);

                // Create locale directories with template files
                for locale in locales {
                    let locale_path = app_path.join(locale.code());
                    fs::create_dir_all(&locale_path).await?;

                    // Create empty template files
                    self.write_text_file(&locale_path.join("title.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("short_description.txt"), "")
                        .await?;
                    self.write_text_file(&locale_path.join("full_description.txt"), "")
                        .await?;

                    // Create changelogs directory
                    fs::create_dir_all(locale_path.join("changelogs")).await?;
                }

                // Create screenshots directory structure
                let screenshots_path = app_path.join("screenshots");
                for locale in locales {
                    let locale_screenshots = screenshots_path.join(locale.code());
                    fs::create_dir_all(locale_screenshots.join("phone")).await?;
                    fs::create_dir_all(locale_screenshots.join("tablet")).await?;
                    fs::create_dir_all(locale_screenshots.join("tv")).await?;
                    fs::create_dir_all(locale_screenshots.join("wear")).await?;
                }

                // Create default store info
                let default_locale = locales.first().cloned().unwrap_or_default();
                let store_info = GooglePlayStoreInfo {
                    default_locale: Some(default_locale.code()),
                    ..Default::default()
                };
                self.write_yaml_file(&app_path.join("store_info.yaml"), &store_info)
                    .await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_storage() -> (FastlaneStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = FastlaneStorage::new(temp_dir.path().join("metadata"));
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_init_apple_creates_structure() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap(), Locale::new("de-DE").unwrap()];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        // Check directory structure
        let app_path = storage.apple_path("com.example.app");
        assert!(app_path.exists());
        assert!(app_path.join("en-US").exists());
        assert!(app_path.join("de-DE").exists());
        assert!(app_path.join("en-US/name.txt").exists());
        assert!(app_path.join("screenshots/en-US").exists());
        assert!(app_path.join("app_store_info.yaml").exists());
    }

    #[tokio::test]
    async fn test_init_google_play_creates_structure() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap()];

        storage
            .init(Platform::GooglePlay, "com.example.app", &locales)
            .await
            .unwrap();

        // Check directory structure
        let app_path = storage.google_play_path("com.example.app");
        assert!(app_path.exists());
        assert!(app_path.join("en-US").exists());
        assert!(app_path.join("en-US/title.txt").exists());
        assert!(app_path.join("en-US/changelogs").exists());
        assert!(app_path.join("screenshots/en-US/phone").exists());
        assert!(app_path.join("store_info.yaml").exists());
    }

    #[tokio::test]
    async fn test_save_and_load_apple_metadata() {
        let (storage, _temp) = setup_test_storage().await;

        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.primary_locale = Locale::new("en-US").unwrap();
        metadata.category = Some(AppleCategory::Productivity);

        let localized = AppleLocalizedMetadata {
            name: "My App".to_string(),
            subtitle: Some("The best app".to_string()),
            description: "A great description".to_string(),
            keywords: Some("app,great,best".to_string()),
            whats_new: Some("Bug fixes".to_string()),
            promotional_text: Some("Try it now!".to_string()),
            support_url: Some("https://example.com/support".to_string()),
            marketing_url: Some("https://example.com".to_string()),
            privacy_policy_url: Some("https://example.com/privacy".to_string()),
        };
        metadata.set_localization("en-US", localized);

        // Save
        storage.save_apple(&metadata).await.unwrap();

        // Load
        let loaded = storage.load_apple("com.example.app").await.unwrap();

        assert_eq!(loaded.bundle_id, "com.example.app");
        assert_eq!(loaded.primary_locale.code(), "en-US");
        assert_eq!(loaded.category, Some(AppleCategory::Productivity));

        let loc = loaded.get_localization("en-US").unwrap();
        assert_eq!(loc.name, "My App");
        assert_eq!(loc.subtitle.as_deref(), Some("The best app"));
        assert_eq!(loc.description, "A great description");
    }

    #[tokio::test]
    async fn test_save_and_load_google_play_metadata() {
        let (storage, _temp) = setup_test_storage().await;

        let mut metadata = GooglePlayMetadata::new("com.example.app");
        metadata.default_locale = Locale::new("en-US").unwrap();
        metadata.category = Some(GooglePlayCategory::Productivity);
        metadata.contact_email = Some("dev@example.com".to_string());

        let mut localized = GooglePlayLocalizedMetadata::new(
            "My App",
            "A short description",
            "A full description of the app",
        );
        localized.add_changelog("100", "Initial release");
        localized.add_changelog("101", "Bug fixes and improvements");
        metadata.set_localization("en-US", localized);

        // Save
        storage.save_google_play(&metadata).await.unwrap();

        // Load
        let loaded = storage.load_google_play("com.example.app").await.unwrap();

        assert_eq!(loaded.package_name, "com.example.app");
        assert_eq!(loaded.default_locale.code(), "en-US");
        assert_eq!(loaded.category, Some(GooglePlayCategory::Productivity));
        assert_eq!(loaded.contact_email.as_deref(), Some("dev@example.com"));

        let loc = loaded.get_localization("en-US").unwrap();
        assert_eq!(loc.title, "My App");
        assert_eq!(loc.short_description, "A short description");
        assert_eq!(loc.changelogs.get("100"), Some(&"Initial release".to_string()));
        assert_eq!(
            loc.changelogs.get("101"),
            Some(&"Bug fixes and improvements".to_string())
        );
    }

    #[tokio::test]
    async fn test_exists_returns_false_for_missing() {
        let (storage, _temp) = setup_test_storage().await;

        assert!(!storage
            .exists_apple("com.nonexistent.app")
            .await
            .unwrap());
        assert!(!storage
            .exists_google_play("com.nonexistent.app")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn test_exists_returns_true_after_init() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap()];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        assert!(storage.exists_apple("com.example.app").await.unwrap());
    }

    #[tokio::test]
    async fn test_list_locales() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![
            Locale::new("en-US").unwrap(),
            Locale::new("de-DE").unwrap(),
            Locale::new("ja").unwrap(),
        ];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        let found_locales = storage.list_locales_apple("com.example.app").await.unwrap();
        assert_eq!(found_locales.len(), 3);

        let locale_codes: Vec<String> = found_locales.iter().map(|l| l.code()).collect();
        assert!(locale_codes.contains(&"en-US".to_string()));
        assert!(locale_codes.contains(&"de-DE".to_string()));
        assert!(locale_codes.contains(&"ja".to_string()));
    }

    #[tokio::test]
    async fn test_load_missing_returns_not_found() {
        let (storage, _temp) = setup_test_storage().await;

        let result = storage.load_apple("com.nonexistent.app").await;
        assert!(matches!(result, Err(MetadataError::NotFound(_))));
    }
}
