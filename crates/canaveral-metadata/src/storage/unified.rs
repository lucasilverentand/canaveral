//! Unified YAML file storage backend.
//!
//! This module implements a storage backend that stores all metadata in a single
//! YAML file per platform per app, providing a more compact and manageable alternative
//! to the Fastlane directory structure.
//!
//! ## File Structure
//!
//! ```text
//! metadata/
//! ├── com.example.app.apple.yaml
//! ├── com.example.app.google_play.yaml
//! └── assets/
//!     ├── icon.png
//!     └── screenshots/
//!         ├── en-US/
//!         │   ├── iphone_6_5_01.png
//!         │   └── iphone_6_5_02.png
//!         └── de-DE/
//!             └── iphone_6_5_01.png
//! ```
//!
//! ## YAML Format (Apple)
//!
//! ```yaml
//! app_id: com.example.app
//! platform: apple
//! default_locale: en-US
//!
//! category:
//!   primary: games
//!   secondary: puzzle
//!
//! age_rating:
//!   alcohol_tobacco_drugs: none
//!   contests: none
//!   # ... etc
//!
//! localizations:
//!   en-US:
//!     name: "My Awesome App"
//!     subtitle: "The best app ever"
//!     description: |
//!       Long description goes here.
//!     keywords: "keyword1,keyword2,keyword3"
//!     whats_new: "Bug fixes"
//!     promotional_text: "Now with new features!"
//!     support_url: "https://example.com/support"
//!     marketing_url: "https://example.com"
//!     privacy_policy_url: "https://example.com/privacy"
//!
//! assets:
//!   icon: assets/icon.png
//!   screenshots:
//!     en-US:
//!       iphone_6_5:
//!         - assets/screenshots/en-US/iphone_6_5_01.png
//!         - assets/screenshots/en-US/iphone_6_5_02.png
//! ```

use super::MetadataStorage;
use crate::{
    AppleAgeRating, AppleCategory, AppleLocalizedMetadata, AppleMetadata, AppleScreenshotSet,
    GooglePlayCategory, GooglePlayContentRating, GooglePlayLocalizedMetadata,
    GooglePlayMetadata, GooglePlayScreenshotSet, Locale, MetadataError, Platform,
    Result,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, warn};

/// Unified YAML file storage backend.
///
/// This storage backend stores all metadata in a single YAML file per platform per app:
/// - `{base_path}/{app_id}.apple.yaml`
/// - `{base_path}/{app_id}.google_play.yaml`
///
/// Assets (screenshots, icons, etc.) are stored relative to the YAML file location.
#[derive(Debug, Clone)]
pub struct UnifiedStorage {
    /// Base path for all metadata.
    base_path: PathBuf,
}

impl UnifiedStorage {
    /// Creates a new Unified storage backend.
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

    /// Get path to the unified YAML file for a specific app and platform.
    ///
    /// # Arguments
    ///
    /// * `app_id` - The app identifier (bundle_id or package_name)
    /// * `platform` - The target platform
    ///
    /// # Returns
    ///
    /// Path in the format `{base_path}/{app_id}.{platform}.yaml`
    pub fn metadata_file_path(&self, app_id: &str, platform: Platform) -> PathBuf {
        let platform_suffix = match platform {
            Platform::Apple => "apple",
            Platform::GooglePlay => "google_play",
        };
        self.base_path.join(format!("{}.{}.yaml", app_id, platform_suffix))
    }

    /// Get path to assets directory for an app.
    fn assets_path(&self, app_id: &str) -> PathBuf {
        self.base_path.join(format!("{}_assets", app_id))
    }

    /// Read a YAML file and deserialize it.
    async fn read_yaml_file<T: for<'de> Deserialize<'de>>(&self, path: &Path) -> Result<T> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                MetadataError::NotFound(format!("File not found: {}", path.display()))
            } else {
                MetadataError::Io(e)
            }
        })?;
        let value: T = serde_yaml::from_str(&content)?;
        Ok(value)
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

    /// Convert relative asset paths in the YAML to absolute paths based on base_path.
    fn resolve_asset_path(&self, relative_path: &str) -> PathBuf {
        self.base_path.join(relative_path)
    }

    /// Convert absolute asset path to relative path for storage in YAML.
    fn make_relative_path(&self, absolute_path: &Path) -> Option<String> {
        absolute_path
            .strip_prefix(&self.base_path)
            .ok()
            .map(|p| p.to_string_lossy().to_string())
    }
}

// =============================================================================
// Intermediate serialization types for YAML format
// =============================================================================

/// Unified Apple metadata YAML structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAppleMetadata {
    /// App bundle identifier.
    pub app_id: String,
    /// Platform identifier (always "apple").
    #[serde(default = "default_apple_platform")]
    pub platform: String,
    /// Default/primary locale code.
    pub default_locale: String,
    /// Category settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<UnifiedCategory>,
    /// Age rating configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age_rating: Option<AppleAgeRating>,
    /// Localized metadata by locale code.
    #[serde(default)]
    pub localizations: HashMap<String, UnifiedAppleLocalization>,
    /// Asset paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<UnifiedAppleAssets>,
    /// Privacy policy URL (app-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
    /// Support URL (app-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_url: Option<String>,
    /// Marketing URL (app-level).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing_url: Option<String>,
    /// Copyright text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copyright: Option<String>,
}

fn default_apple_platform() -> String {
    "apple".to_string()
}

/// Category configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCategory {
    /// Primary category.
    pub primary: Option<AppleCategory>,
    /// Secondary category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secondary: Option<AppleCategory>,
}

/// Localized metadata for Apple.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedAppleLocalization {
    /// App name (max 30 characters).
    pub name: String,
    /// App subtitle (max 30 characters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    /// App description (max 4000 characters).
    pub description: String,
    /// Keywords for search (max 100 characters, comma-separated).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<String>,
    /// What's new in this version (max 4000 characters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub whats_new: Option<String>,
    /// Promotional text (max 170 characters).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promotional_text: Option<String>,
    /// Support URL (locale-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_url: Option<String>,
    /// Marketing URL (locale-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub marketing_url: Option<String>,
    /// Privacy policy URL (locale-specific).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy_policy_url: Option<String>,
}

/// Asset paths for Apple metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedAppleAssets {
    /// App icon path (relative to YAML file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Screenshots organized by locale and device type.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub screenshots: HashMap<String, UnifiedAppleScreenshots>,
    /// App previews (videos) organized by locale and device type.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub previews: HashMap<String, HashMap<String, Vec<String>>>,
}

/// Screenshots organized by device type for a single locale.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedAppleScreenshots {
    /// iPhone 6.5" display screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iphone_6_5: Vec<String>,
    /// iPhone 5.5" display screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iphone_5_5: Vec<String>,
    /// iPad Pro 12.9" display screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ipad_pro_12_9: Vec<String>,
    /// iPad Pro 11" display screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ipad_pro_11: Vec<String>,
    /// Mac screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub mac: Vec<String>,
    /// Apple TV screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apple_tv: Vec<String>,
    /// Apple Watch screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub apple_watch: Vec<String>,
}

// =============================================================================
// Google Play unified format types
// =============================================================================

/// Unified Google Play metadata YAML structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedGooglePlayMetadata {
    /// Application package name.
    pub app_id: String,
    /// Platform identifier (always "google_play").
    #[serde(default = "default_google_play_platform")]
    pub platform: String,
    /// Default locale code.
    pub default_locale: String,
    /// App category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<GooglePlayCategory>,
    /// Content rating configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_rating: Option<GooglePlayContentRating>,
    /// Localized metadata by locale code.
    #[serde(default)]
    pub localizations: HashMap<String, UnifiedGooglePlayLocalization>,
    /// Asset paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assets: Option<UnifiedGooglePlayAssets>,
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

fn default_google_play_platform() -> String {
    "google_play".to_string()
}

/// Localized metadata for Google Play.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedGooglePlayLocalization {
    /// App title (max 50 characters).
    pub title: String,
    /// Short description (max 80 characters).
    pub short_description: String,
    /// Full description (max 4000 characters).
    pub full_description: String,
    /// Changelogs by version code.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub changelogs: HashMap<String, String>,
    /// Video URL (YouTube).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_url: Option<String>,
}

/// Asset paths for Google Play metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedGooglePlayAssets {
    /// App icon path (relative to YAML file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Feature graphic path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feature_graphic: Option<String>,
    /// Promo graphic path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promo_graphic: Option<String>,
    /// TV banner path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tv_banner: Option<String>,
    /// Screenshots organized by locale and device type.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub screenshots: HashMap<String, UnifiedGooglePlayScreenshots>,
}

/// Screenshots organized by device type for a single locale (Google Play).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UnifiedGooglePlayScreenshots {
    /// Phone screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phone: Vec<String>,
    /// 7" tablet screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tablet_7: Vec<String>,
    /// 10" tablet screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tablet_10: Vec<String>,
    /// Android TV screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tv: Vec<String>,
    /// Wear OS screenshots.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wear: Vec<String>,
}

// =============================================================================
// Conversion implementations: UnifiedAppleMetadata <-> AppleMetadata
// =============================================================================

impl From<UnifiedAppleMetadata> for AppleMetadata {
    fn from(unified: UnifiedAppleMetadata) -> Self {
        let primary_locale = Locale::new(&unified.default_locale).unwrap_or_default();

        let mut localizations = HashMap::new();
        for (locale_code, loc) in unified.localizations {
            localizations.insert(locale_code, AppleLocalizedMetadata {
                name: loc.name,
                subtitle: loc.subtitle,
                description: loc.description,
                keywords: loc.keywords,
                whats_new: loc.whats_new,
                promotional_text: loc.promotional_text,
                support_url: loc.support_url,
                marketing_url: loc.marketing_url,
                privacy_policy_url: loc.privacy_policy_url,
            });
        }

        let (category, secondary_category) = unified.category.map_or((None, None), |cat| {
            (cat.primary, cat.secondary)
        });

        // Convert screenshot paths to MediaAssets
        // For now, we just track the paths; a full implementation would
        // aggregate screenshots across locales or handle locale-specific sets
        let screenshots = AppleScreenshotSet::default();

        let icon = unified.assets.as_ref().and_then(|a| {
            a.icon.as_ref().map(|p| PathBuf::from(p))
        });

        AppleMetadata {
            bundle_id: unified.app_id,
            primary_locale,
            localizations,
            category,
            secondary_category,
            age_rating: unified.age_rating,
            screenshots,
            previews: HashMap::new(),
            icon,
            privacy_policy_url: unified.privacy_policy_url,
            support_url: unified.support_url,
            marketing_url: unified.marketing_url,
            copyright: unified.copyright,
        }
    }
}

impl From<&AppleMetadata> for UnifiedAppleMetadata {
    fn from(metadata: &AppleMetadata) -> Self {
        let category = if metadata.category.is_some() || metadata.secondary_category.is_some() {
            Some(UnifiedCategory {
                primary: metadata.category,
                secondary: metadata.secondary_category,
            })
        } else {
            None
        };

        let localizations = metadata
            .localizations
            .iter()
            .map(|(locale_code, loc)| {
                (
                    locale_code.clone(),
                    UnifiedAppleLocalization {
                        name: loc.name.clone(),
                        subtitle: loc.subtitle.clone(),
                        description: loc.description.clone(),
                        keywords: loc.keywords.clone(),
                        whats_new: loc.whats_new.clone(),
                        promotional_text: loc.promotional_text.clone(),
                        support_url: loc.support_url.clone(),
                        marketing_url: loc.marketing_url.clone(),
                        privacy_policy_url: loc.privacy_policy_url.clone(),
                    },
                )
            })
            .collect();

        let assets = Some(UnifiedAppleAssets {
            icon: metadata.icon.as_ref().map(|p| p.to_string_lossy().to_string()),
            screenshots: HashMap::new(), // Screenshots would be populated separately
            previews: HashMap::new(),
        });

        UnifiedAppleMetadata {
            app_id: metadata.bundle_id.clone(),
            platform: "apple".to_string(),
            default_locale: metadata.primary_locale.code(),
            category,
            age_rating: metadata.age_rating.clone(),
            localizations,
            assets,
            privacy_policy_url: metadata.privacy_policy_url.clone(),
            support_url: metadata.support_url.clone(),
            marketing_url: metadata.marketing_url.clone(),
            copyright: metadata.copyright.clone(),
        }
    }
}

// =============================================================================
// Conversion implementations: UnifiedGooglePlayMetadata <-> GooglePlayMetadata
// =============================================================================

impl From<UnifiedGooglePlayMetadata> for GooglePlayMetadata {
    fn from(unified: UnifiedGooglePlayMetadata) -> Self {
        let default_locale = Locale::new(&unified.default_locale).unwrap_or_default();

        let mut localizations = HashMap::new();
        for (locale_code, loc) in unified.localizations {
            localizations.insert(locale_code, GooglePlayLocalizedMetadata {
                title: loc.title,
                short_description: loc.short_description,
                full_description: loc.full_description,
                changelogs: loc.changelogs,
                video_url: loc.video_url,
            });
        }

        let icon = unified.assets.as_ref().and_then(|a| {
            a.icon.as_ref().map(|p| PathBuf::from(p))
        });

        let feature_graphic = unified.assets.as_ref().and_then(|a| {
            a.feature_graphic.as_ref().map(|p| PathBuf::from(p))
        });

        let promo_graphic = unified.assets.as_ref().and_then(|a| {
            a.promo_graphic.as_ref().map(|p| PathBuf::from(p))
        });

        let tv_banner = unified.assets.as_ref().and_then(|a| {
            a.tv_banner.as_ref().map(|p| PathBuf::from(p))
        });

        GooglePlayMetadata {
            package_name: unified.app_id,
            default_locale,
            localizations,
            category: unified.category,
            content_rating: unified.content_rating,
            screenshots: GooglePlayScreenshotSet::default(),
            feature_graphic,
            promo_graphic,
            tv_banner,
            icon,
            privacy_policy_url: unified.privacy_policy_url,
            contact_email: unified.contact_email,
            contact_phone: unified.contact_phone,
            contact_website: unified.contact_website,
        }
    }
}

impl From<&GooglePlayMetadata> for UnifiedGooglePlayMetadata {
    fn from(metadata: &GooglePlayMetadata) -> Self {
        let localizations = metadata
            .localizations
            .iter()
            .map(|(locale_code, loc)| {
                (
                    locale_code.clone(),
                    UnifiedGooglePlayLocalization {
                        title: loc.title.clone(),
                        short_description: loc.short_description.clone(),
                        full_description: loc.full_description.clone(),
                        changelogs: loc.changelogs.clone(),
                        video_url: loc.video_url.clone(),
                    },
                )
            })
            .collect();

        let assets = Some(UnifiedGooglePlayAssets {
            icon: metadata.icon.as_ref().map(|p| p.to_string_lossy().to_string()),
            feature_graphic: metadata.feature_graphic.as_ref().map(|p| p.to_string_lossy().to_string()),
            promo_graphic: metadata.promo_graphic.as_ref().map(|p| p.to_string_lossy().to_string()),
            tv_banner: metadata.tv_banner.as_ref().map(|p| p.to_string_lossy().to_string()),
            screenshots: HashMap::new(),
        });

        UnifiedGooglePlayMetadata {
            app_id: metadata.package_name.clone(),
            platform: "google_play".to_string(),
            default_locale: metadata.default_locale.code(),
            category: metadata.category,
            content_rating: metadata.content_rating.clone(),
            localizations,
            assets,
            privacy_policy_url: metadata.privacy_policy_url.clone(),
            contact_email: metadata.contact_email.clone(),
            contact_phone: metadata.contact_phone.clone(),
            contact_website: metadata.contact_website.clone(),
        }
    }
}

// =============================================================================
// MetadataStorage implementation
// =============================================================================

#[async_trait]
impl MetadataStorage for UnifiedStorage {
    async fn load_apple(&self, bundle_id: &str) -> Result<AppleMetadata> {
        let file_path = self.metadata_file_path(bundle_id, Platform::Apple);
        debug!("Loading Apple metadata from {:?}", file_path);

        let unified: UnifiedAppleMetadata = self.read_yaml_file(&file_path).await?;
        let mut metadata: AppleMetadata = unified.into();

        // Resolve relative paths to absolute paths
        if let Some(ref icon) = metadata.icon {
            if !icon.is_absolute() {
                metadata.icon = Some(self.resolve_asset_path(&icon.to_string_lossy()));
            }
        }

        Ok(metadata)
    }

    async fn load_google_play(&self, package_name: &str) -> Result<GooglePlayMetadata> {
        let file_path = self.metadata_file_path(package_name, Platform::GooglePlay);
        debug!("Loading Google Play metadata from {:?}", file_path);

        let unified: UnifiedGooglePlayMetadata = self.read_yaml_file(&file_path).await?;
        let mut metadata: GooglePlayMetadata = unified.into();

        // Resolve relative paths to absolute paths
        if let Some(ref icon) = metadata.icon {
            if !icon.is_absolute() {
                metadata.icon = Some(self.resolve_asset_path(&icon.to_string_lossy()));
            }
        }
        if let Some(ref feature_graphic) = metadata.feature_graphic {
            if !feature_graphic.is_absolute() {
                metadata.feature_graphic = Some(self.resolve_asset_path(&feature_graphic.to_string_lossy()));
            }
        }
        if let Some(ref promo_graphic) = metadata.promo_graphic {
            if !promo_graphic.is_absolute() {
                metadata.promo_graphic = Some(self.resolve_asset_path(&promo_graphic.to_string_lossy()));
            }
        }
        if let Some(ref tv_banner) = metadata.tv_banner {
            if !tv_banner.is_absolute() {
                metadata.tv_banner = Some(self.resolve_asset_path(&tv_banner.to_string_lossy()));
            }
        }

        Ok(metadata)
    }

    async fn save_apple(&self, metadata: &AppleMetadata) -> Result<()> {
        let file_path = self.metadata_file_path(&metadata.bundle_id, Platform::Apple);
        debug!("Saving Apple metadata to {:?}", file_path);

        let mut unified: UnifiedAppleMetadata = metadata.into();

        // Convert absolute paths to relative paths
        if let Some(ref mut assets) = unified.assets {
            if let Some(ref icon) = metadata.icon {
                assets.icon = self.make_relative_path(icon);
            }
        }

        self.write_yaml_file(&file_path, &unified).await
    }

    async fn save_google_play(&self, metadata: &GooglePlayMetadata) -> Result<()> {
        let file_path = self.metadata_file_path(&metadata.package_name, Platform::GooglePlay);
        debug!("Saving Google Play metadata to {:?}", file_path);

        let mut unified: UnifiedGooglePlayMetadata = metadata.into();

        // Convert absolute paths to relative paths
        if let Some(ref mut assets) = unified.assets {
            if let Some(ref icon) = metadata.icon {
                assets.icon = self.make_relative_path(icon);
            }
            if let Some(ref fg) = metadata.feature_graphic {
                assets.feature_graphic = self.make_relative_path(fg);
            }
            if let Some(ref pg) = metadata.promo_graphic {
                assets.promo_graphic = self.make_relative_path(pg);
            }
            if let Some(ref tb) = metadata.tv_banner {
                assets.tv_banner = self.make_relative_path(tb);
            }
        }

        self.write_yaml_file(&file_path, &unified).await
    }

    async fn exists_apple(&self, bundle_id: &str) -> Result<bool> {
        let path = self.metadata_file_path(bundle_id, Platform::Apple);
        Ok(path.exists())
    }

    async fn exists_google_play(&self, package_name: &str) -> Result<bool> {
        let path = self.metadata_file_path(package_name, Platform::GooglePlay);
        Ok(path.exists())
    }

    async fn list_locales_apple(&self, bundle_id: &str) -> Result<Vec<Locale>> {
        let file_path = self.metadata_file_path(bundle_id, Platform::Apple);

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let unified: UnifiedAppleMetadata = self.read_yaml_file(&file_path).await?;
        let mut locales = Vec::new();

        for locale_code in unified.localizations.keys() {
            match Locale::new(locale_code) {
                Ok(locale) => locales.push(locale),
                Err(e) => {
                    warn!("Skipping invalid locale '{}': {}", locale_code, e);
                }
            }
        }

        Ok(locales)
    }

    async fn list_locales_google_play(&self, package_name: &str) -> Result<Vec<Locale>> {
        let file_path = self.metadata_file_path(package_name, Platform::GooglePlay);

        if !file_path.exists() {
            return Ok(Vec::new());
        }

        let unified: UnifiedGooglePlayMetadata = self.read_yaml_file(&file_path).await?;
        let mut locales = Vec::new();

        for locale_code in unified.localizations.keys() {
            match Locale::new(locale_code) {
                Ok(locale) => locales.push(locale),
                Err(e) => {
                    warn!("Skipping invalid locale '{}': {}", locale_code, e);
                }
            }
        }

        Ok(locales)
    }

    fn apple_path(&self, bundle_id: &str) -> PathBuf {
        self.metadata_file_path(bundle_id, Platform::Apple)
    }

    fn google_play_path(&self, package_name: &str) -> PathBuf {
        self.metadata_file_path(package_name, Platform::GooglePlay)
    }

    async fn init(&self, platform: Platform, app_id: &str, locales: &[Locale]) -> Result<()> {
        match platform {
            Platform::Apple => {
                let primary_locale = locales.first().cloned().unwrap_or_default();

                let mut localizations = HashMap::new();
                for locale in locales {
                    localizations.insert(
                        locale.code(),
                        UnifiedAppleLocalization {
                            name: String::new(),
                            subtitle: None,
                            description: String::new(),
                            keywords: None,
                            whats_new: None,
                            promotional_text: None,
                            support_url: None,
                            marketing_url: None,
                            privacy_policy_url: None,
                        },
                    );
                }

                let unified = UnifiedAppleMetadata {
                    app_id: app_id.to_string(),
                    platform: "apple".to_string(),
                    default_locale: primary_locale.code(),
                    category: None,
                    age_rating: None,
                    localizations,
                    assets: Some(UnifiedAppleAssets::default()),
                    privacy_policy_url: None,
                    support_url: None,
                    marketing_url: None,
                    copyright: None,
                };

                let file_path = self.metadata_file_path(app_id, Platform::Apple);
                self.write_yaml_file(&file_path, &unified).await?;

                // Create assets directory structure
                let assets_path = self.assets_path(app_id);
                fs::create_dir_all(&assets_path).await?;
                for locale in locales {
                    fs::create_dir_all(assets_path.join("screenshots").join(locale.code())).await?;
                }
            }
            Platform::GooglePlay => {
                let default_locale = locales.first().cloned().unwrap_or_default();

                let mut localizations = HashMap::new();
                for locale in locales {
                    localizations.insert(
                        locale.code(),
                        UnifiedGooglePlayLocalization {
                            title: String::new(),
                            short_description: String::new(),
                            full_description: String::new(),
                            changelogs: HashMap::new(),
                            video_url: None,
                        },
                    );
                }

                let unified = UnifiedGooglePlayMetadata {
                    app_id: app_id.to_string(),
                    platform: "google_play".to_string(),
                    default_locale: default_locale.code(),
                    category: None,
                    content_rating: None,
                    localizations,
                    assets: Some(UnifiedGooglePlayAssets::default()),
                    privacy_policy_url: None,
                    contact_email: None,
                    contact_phone: None,
                    contact_website: None,
                };

                let file_path = self.metadata_file_path(app_id, Platform::GooglePlay);
                self.write_yaml_file(&file_path, &unified).await?;

                // Create assets directory structure
                let assets_path = self.assets_path(app_id);
                fs::create_dir_all(&assets_path).await?;
                for locale in locales {
                    let locale_path = assets_path.join("screenshots").join(locale.code());
                    fs::create_dir_all(locale_path.join("phone")).await?;
                    fs::create_dir_all(locale_path.join("tablet")).await?;
                    fs::create_dir_all(locale_path.join("tv")).await?;
                    fs::create_dir_all(locale_path.join("wear")).await?;
                }
            }
        }

        Ok(())
    }

    async fn add_locale(
        &self,
        platform: Platform,
        app_id: &str,
        locale: &Locale,
        copy_from: Option<&Locale>,
    ) -> Result<()> {
        match platform {
            Platform::Apple => {
                let file_path = self.metadata_file_path(app_id, Platform::Apple);

                if !file_path.exists() {
                    return Err(MetadataError::NotFound(format!(
                        "Apple metadata not found for: {}",
                        app_id
                    )));
                }

                let mut unified: UnifiedAppleMetadata = self.read_yaml_file(&file_path).await?;

                if unified.localizations.contains_key(&locale.code()) {
                    return Err(MetadataError::InvalidFormat(format!(
                        "Locale '{}' already exists",
                        locale.code()
                    )));
                }

                let new_localization = if let Some(source_locale) = copy_from {
                    unified
                        .localizations
                        .get(&source_locale.code())
                        .cloned()
                        .ok_or_else(|| {
                            MetadataError::NotFound(format!(
                                "Source locale '{}' not found",
                                source_locale.code()
                            ))
                        })?
                } else {
                    UnifiedAppleLocalization {
                        name: String::new(),
                        subtitle: None,
                        description: String::new(),
                        keywords: None,
                        whats_new: None,
                        promotional_text: None,
                        support_url: None,
                        marketing_url: None,
                        privacy_policy_url: None,
                    }
                };

                unified.localizations.insert(locale.code(), new_localization);
                self.write_yaml_file(&file_path, &unified).await?;

                // Create screenshots directory for new locale
                let assets_path = self.assets_path(app_id);
                fs::create_dir_all(assets_path.join("screenshots").join(locale.code())).await?;
            }
            Platform::GooglePlay => {
                let file_path = self.metadata_file_path(app_id, Platform::GooglePlay);

                if !file_path.exists() {
                    return Err(MetadataError::NotFound(format!(
                        "Google Play metadata not found for: {}",
                        app_id
                    )));
                }

                let mut unified: UnifiedGooglePlayMetadata = self.read_yaml_file(&file_path).await?;

                if unified.localizations.contains_key(&locale.code()) {
                    return Err(MetadataError::InvalidFormat(format!(
                        "Locale '{}' already exists",
                        locale.code()
                    )));
                }

                let new_localization = if let Some(source_locale) = copy_from {
                    unified
                        .localizations
                        .get(&source_locale.code())
                        .cloned()
                        .ok_or_else(|| {
                            MetadataError::NotFound(format!(
                                "Source locale '{}' not found",
                                source_locale.code()
                            ))
                        })?
                } else {
                    UnifiedGooglePlayLocalization {
                        title: String::new(),
                        short_description: String::new(),
                        full_description: String::new(),
                        changelogs: HashMap::new(),
                        video_url: None,
                    }
                };

                unified.localizations.insert(locale.code(), new_localization);
                self.write_yaml_file(&file_path, &unified).await?;

                // Create screenshots directory structure for new locale
                let assets_path = self.assets_path(app_id);
                let locale_path = assets_path.join("screenshots").join(locale.code());
                fs::create_dir_all(locale_path.join("phone")).await?;
                fs::create_dir_all(locale_path.join("tablet")).await?;
                fs::create_dir_all(locale_path.join("tv")).await?;
                fs::create_dir_all(locale_path.join("wear")).await?;
            }
        }

        Ok(())
    }

    async fn remove_locale(
        &self,
        platform: Platform,
        app_id: &str,
        locale: &Locale,
    ) -> Result<()> {
        match platform {
            Platform::Apple => {
                let file_path = self.metadata_file_path(app_id, Platform::Apple);

                if !file_path.exists() {
                    return Err(MetadataError::NotFound(format!(
                        "Apple metadata not found for: {}",
                        app_id
                    )));
                }

                let mut unified: UnifiedAppleMetadata = self.read_yaml_file(&file_path).await?;

                if !unified.localizations.contains_key(&locale.code()) {
                    return Err(MetadataError::NotFound(format!(
                        "Locale '{}' not found",
                        locale.code()
                    )));
                }

                unified.localizations.remove(&locale.code());

                // Also remove from assets if present
                if let Some(ref mut assets) = unified.assets {
                    assets.screenshots.remove(&locale.code());
                    assets.previews.remove(&locale.code());
                }

                self.write_yaml_file(&file_path, &unified).await?;

                // Remove screenshots directory
                let screenshots_path = self
                    .assets_path(app_id)
                    .join("screenshots")
                    .join(locale.code());
                if screenshots_path.exists() {
                    fs::remove_dir_all(&screenshots_path).await?;
                }
            }
            Platform::GooglePlay => {
                let file_path = self.metadata_file_path(app_id, Platform::GooglePlay);

                if !file_path.exists() {
                    return Err(MetadataError::NotFound(format!(
                        "Google Play metadata not found for: {}",
                        app_id
                    )));
                }

                let mut unified: UnifiedGooglePlayMetadata = self.read_yaml_file(&file_path).await?;

                if !unified.localizations.contains_key(&locale.code()) {
                    return Err(MetadataError::NotFound(format!(
                        "Locale '{}' not found",
                        locale.code()
                    )));
                }

                unified.localizations.remove(&locale.code());

                // Also remove from assets if present
                if let Some(ref mut assets) = unified.assets {
                    assets.screenshots.remove(&locale.code());
                }

                self.write_yaml_file(&file_path, &unified).await?;

                // Remove screenshots directory
                let screenshots_path = self
                    .assets_path(app_id)
                    .join("screenshots")
                    .join(locale.code());
                if screenshots_path.exists() {
                    fs::remove_dir_all(&screenshots_path).await?;
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::apple::AgeRatingLevel;
    use tempfile::TempDir;

    async fn setup_test_storage() -> (UnifiedStorage, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let storage = UnifiedStorage::new(temp_dir.path().join("metadata"));
        (storage, temp_dir)
    }

    #[tokio::test]
    async fn test_metadata_file_path() {
        let (storage, _temp) = setup_test_storage().await;

        let apple_path = storage.metadata_file_path("com.example.app", Platform::Apple);
        assert!(apple_path.to_string_lossy().ends_with("com.example.app.apple.yaml"));

        let google_path = storage.metadata_file_path("com.example.app", Platform::GooglePlay);
        assert!(google_path.to_string_lossy().ends_with("com.example.app.google_play.yaml"));
    }

    #[tokio::test]
    async fn test_init_apple_creates_yaml_file() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap(), Locale::new("de-DE").unwrap()];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        // Check YAML file exists
        let file_path = storage.metadata_file_path("com.example.app", Platform::Apple);
        assert!(file_path.exists());

        // Check assets directory structure
        let assets_path = storage.assets_path("com.example.app");
        assert!(assets_path.join("screenshots/en-US").exists());
        assert!(assets_path.join("screenshots/de-DE").exists());
    }

    #[tokio::test]
    async fn test_init_google_play_creates_yaml_file() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap()];

        storage
            .init(Platform::GooglePlay, "com.example.app", &locales)
            .await
            .unwrap();

        // Check YAML file exists
        let file_path = storage.metadata_file_path("com.example.app", Platform::GooglePlay);
        assert!(file_path.exists());

        // Check assets directory structure
        let assets_path = storage.assets_path("com.example.app");
        assert!(assets_path.join("screenshots/en-US/phone").exists());
        assert!(assets_path.join("screenshots/en-US/tablet").exists());
    }

    #[tokio::test]
    async fn test_save_and_load_apple_metadata() {
        let (storage, _temp) = setup_test_storage().await;

        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.primary_locale = Locale::new("en-US").unwrap();
        metadata.category = Some(AppleCategory::Productivity);
        metadata.copyright = Some("2024 Example Inc.".to_string());

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
        assert_eq!(loaded.copyright.as_deref(), Some("2024 Example Inc."));

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
    async fn test_exists_returns_true_after_save() {
        let (storage, _temp) = setup_test_storage().await;

        let metadata = AppleMetadata::new("com.example.app");
        storage.save_apple(&metadata).await.unwrap();

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
    async fn test_add_locale() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![Locale::new("en-US").unwrap()];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        // Add new locale
        let new_locale = Locale::new("de-DE").unwrap();
        storage
            .add_locale(Platform::Apple, "com.example.app", &new_locale, None)
            .await
            .unwrap();

        let found_locales = storage.list_locales_apple("com.example.app").await.unwrap();
        assert_eq!(found_locales.len(), 2);

        let locale_codes: Vec<String> = found_locales.iter().map(|l| l.code()).collect();
        assert!(locale_codes.contains(&"de-DE".to_string()));
    }

    #[tokio::test]
    async fn test_add_locale_with_copy() {
        let (storage, _temp) = setup_test_storage().await;

        // Create metadata with content
        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.primary_locale = Locale::new("en-US").unwrap();
        let localized = AppleLocalizedMetadata {
            name: "My App".to_string(),
            subtitle: Some("The best app".to_string()),
            description: "A great description".to_string(),
            keywords: Some("app,great,best".to_string()),
            ..Default::default()
        };
        metadata.set_localization("en-US", localized);
        storage.save_apple(&metadata).await.unwrap();

        // Add new locale copying from en-US
        let en_locale = Locale::new("en-US").unwrap();
        let de_locale = Locale::new("de-DE").unwrap();
        storage
            .add_locale(Platform::Apple, "com.example.app", &de_locale, Some(&en_locale))
            .await
            .unwrap();

        // Load and verify
        let loaded = storage.load_apple("com.example.app").await.unwrap();
        let de_loc = loaded.get_localization("de-DE").unwrap();
        assert_eq!(de_loc.name, "My App");
        assert_eq!(de_loc.description, "A great description");
    }

    #[tokio::test]
    async fn test_remove_locale() {
        let (storage, _temp) = setup_test_storage().await;
        let locales = vec![
            Locale::new("en-US").unwrap(),
            Locale::new("de-DE").unwrap(),
        ];

        storage
            .init(Platform::Apple, "com.example.app", &locales)
            .await
            .unwrap();

        // Remove de-DE locale
        let de_locale = Locale::new("de-DE").unwrap();
        storage
            .remove_locale(Platform::Apple, "com.example.app", &de_locale)
            .await
            .unwrap();

        let found_locales = storage.list_locales_apple("com.example.app").await.unwrap();
        assert_eq!(found_locales.len(), 1);
        assert_eq!(found_locales[0].code(), "en-US");
    }

    #[tokio::test]
    async fn test_load_missing_returns_not_found() {
        let (storage, _temp) = setup_test_storage().await;

        let result = storage.load_apple("com.nonexistent.app").await;
        assert!(matches!(result, Err(MetadataError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_yaml_roundtrip_preserves_age_rating() {
        let (storage, _temp) = setup_test_storage().await;

        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.primary_locale = Locale::new("en-US").unwrap();
        metadata.age_rating = Some(AppleAgeRating {
            alcohol_tobacco_drugs: AgeRatingLevel::InfrequentOrMild,
            violence_cartoon: AgeRatingLevel::FrequentOrIntense,
            unrestricted_web_access: true,
            ..Default::default()
        });
        metadata.set_localization("en-US", AppleLocalizedMetadata::new("My App", "Description"));

        storage.save_apple(&metadata).await.unwrap();
        let loaded = storage.load_apple("com.example.app").await.unwrap();

        let age_rating = loaded.age_rating.unwrap();
        assert_eq!(age_rating.alcohol_tobacco_drugs, AgeRatingLevel::InfrequentOrMild);
        assert_eq!(age_rating.violence_cartoon, AgeRatingLevel::FrequentOrIntense);
        assert!(age_rating.unrestricted_web_access);
    }
}
