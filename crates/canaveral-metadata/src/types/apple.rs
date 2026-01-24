//! Apple App Store metadata types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::common::{Locale, MediaAsset};

/// Character limits for Apple App Store metadata fields.
pub mod limits {
    /// Maximum characters for app name.
    pub const NAME_MAX: usize = 30;
    /// Maximum characters for subtitle.
    pub const SUBTITLE_MAX: usize = 30;
    /// Maximum characters for description.
    pub const DESCRIPTION_MAX: usize = 4000;
    /// Maximum characters for keywords.
    pub const KEYWORDS_MAX: usize = 100;
    /// Maximum characters for what's new.
    pub const WHATS_NEW_MAX: usize = 4000;
    /// Maximum characters for promotional text.
    pub const PROMOTIONAL_TEXT_MAX: usize = 170;
}

/// Full app metadata for Apple App Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppleMetadata {
    /// App bundle identifier.
    pub bundle_id: String,
    /// Primary locale.
    pub primary_locale: Locale,
    /// Localized metadata by locale.
    pub localizations: HashMap<String, AppleLocalizedMetadata>,
    /// App category.
    pub category: Option<AppleCategory>,
    /// Secondary category.
    pub secondary_category: Option<AppleCategory>,
    /// Age rating configuration.
    pub age_rating: Option<AppleAgeRating>,
    /// Screenshots by device type.
    pub screenshots: AppleScreenshotSet,
    /// App previews (videos) by device type.
    pub previews: HashMap<String, Vec<MediaAsset>>,
    /// App icon path.
    pub icon: Option<PathBuf>,
    /// Privacy policy URL.
    pub privacy_policy_url: Option<String>,
    /// Support URL.
    pub support_url: Option<String>,
    /// Marketing URL.
    pub marketing_url: Option<String>,
    /// Copyright text.
    pub copyright: Option<String>,
}

impl AppleMetadata {
    /// Creates new Apple metadata with the given bundle ID.
    pub fn new(bundle_id: impl Into<String>) -> Self {
        Self {
            bundle_id: bundle_id.into(),
            ..Default::default()
        }
    }

    /// Gets localized metadata for a specific locale.
    pub fn get_localization(&self, locale: &str) -> Option<&AppleLocalizedMetadata> {
        self.localizations.get(locale)
    }

    /// Sets localized metadata for a specific locale.
    pub fn set_localization(&mut self, locale: impl Into<String>, metadata: AppleLocalizedMetadata) {
        self.localizations.insert(locale.into(), metadata);
    }
}

/// Locale-specific content for Apple App Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppleLocalizedMetadata {
    /// App name (max 30 characters).
    pub name: String,
    /// App subtitle (max 30 characters).
    pub subtitle: Option<String>,
    /// App description (max 4000 characters).
    pub description: String,
    /// Keywords for search (max 100 characters, comma-separated).
    pub keywords: Option<String>,
    /// What's new in this version (max 4000 characters).
    pub whats_new: Option<String>,
    /// Promotional text (max 170 characters).
    pub promotional_text: Option<String>,
    /// Privacy policy URL.
    pub privacy_policy_url: Option<String>,
    /// Support URL.
    pub support_url: Option<String>,
    /// Marketing URL.
    pub marketing_url: Option<String>,
}

impl AppleLocalizedMetadata {
    /// Creates new localized metadata with required fields.
    pub fn new(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            ..Default::default()
        }
    }

    /// Validates the metadata against character limits.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.name.chars().count() > limits::NAME_MAX {
            errors.push(format!(
                "Name exceeds {} characters ({})",
                limits::NAME_MAX,
                self.name.chars().count()
            ));
        }

        if let Some(ref subtitle) = self.subtitle {
            if subtitle.chars().count() > limits::SUBTITLE_MAX {
                errors.push(format!(
                    "Subtitle exceeds {} characters ({})",
                    limits::SUBTITLE_MAX,
                    subtitle.chars().count()
                ));
            }
        }

        if self.description.chars().count() > limits::DESCRIPTION_MAX {
            errors.push(format!(
                "Description exceeds {} characters ({})",
                limits::DESCRIPTION_MAX,
                self.description.chars().count()
            ));
        }

        if let Some(ref keywords) = self.keywords {
            if keywords.chars().count() > limits::KEYWORDS_MAX {
                errors.push(format!(
                    "Keywords exceed {} characters ({})",
                    limits::KEYWORDS_MAX,
                    keywords.chars().count()
                ));
            }
        }

        if let Some(ref whats_new) = self.whats_new {
            if whats_new.chars().count() > limits::WHATS_NEW_MAX {
                errors.push(format!(
                    "What's new exceeds {} characters ({})",
                    limits::WHATS_NEW_MAX,
                    whats_new.chars().count()
                ));
            }
        }

        if let Some(ref promo) = self.promotional_text {
            if promo.chars().count() > limits::PROMOTIONAL_TEXT_MAX {
                errors.push(format!(
                    "Promotional text exceeds {} characters ({})",
                    limits::PROMOTIONAL_TEXT_MAX,
                    promo.chars().count()
                ));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// Screenshots organized by device type for Apple platforms.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppleScreenshotSet {
    /// iPhone 6.5" display (iPhone 14 Pro Max, etc.).
    pub iphone_6_5: Vec<MediaAsset>,
    /// iPhone 5.5" display (iPhone 8 Plus, etc.).
    pub iphone_5_5: Vec<MediaAsset>,
    /// iPad Pro 12.9" display.
    pub ipad_pro_12_9: Vec<MediaAsset>,
    /// iPad Pro 11" display.
    pub ipad_pro_11: Vec<MediaAsset>,
    /// Mac screenshots.
    pub mac: Vec<MediaAsset>,
    /// Apple TV screenshots.
    pub apple_tv: Vec<MediaAsset>,
    /// Apple Watch screenshots.
    pub apple_watch: Vec<MediaAsset>,
}

impl AppleScreenshotSet {
    /// Returns the total number of screenshots across all device types.
    pub fn total_count(&self) -> usize {
        self.iphone_6_5.len()
            + self.iphone_5_5.len()
            + self.ipad_pro_12_9.len()
            + self.ipad_pro_11.len()
            + self.mac.len()
            + self.apple_tv.len()
            + self.apple_watch.len()
    }

    /// Checks if any screenshots are present.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }
}

/// Apple App Store categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppleCategory {
    /// Books category.
    Books,
    /// Business category.
    Business,
    /// Developer Tools category.
    DeveloperTools,
    /// Education category.
    Education,
    /// Entertainment category.
    Entertainment,
    /// Finance category.
    Finance,
    /// Food & Drink category.
    FoodAndDrink,
    /// Games category.
    Games,
    /// Graphics & Design category.
    GraphicsAndDesign,
    /// Health & Fitness category.
    HealthAndFitness,
    /// Kids category.
    Kids,
    /// Lifestyle category.
    Lifestyle,
    /// Magazines & Newspapers category.
    MagazinesAndNewspapers,
    /// Medical category.
    Medical,
    /// Music category.
    Music,
    /// Navigation category.
    Navigation,
    /// News category.
    News,
    /// Photo & Video category.
    PhotoAndVideo,
    /// Productivity category.
    Productivity,
    /// Reference category.
    Reference,
    /// Shopping category.
    Shopping,
    /// Social Networking category.
    SocialNetworking,
    /// Sports category.
    Sports,
    /// Stickers category.
    Stickers,
    /// Travel category.
    Travel,
    /// Utilities category.
    Utilities,
    /// Weather category.
    Weather,
}

impl Default for AppleCategory {
    fn default() -> Self {
        AppleCategory::Utilities
    }
}

/// Age rating configuration for Apple App Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppleAgeRating {
    /// Alcohol, tobacco, or drug use references.
    pub alcohol_tobacco_drugs: AgeRatingLevel,
    /// Contests.
    pub contests: AgeRatingLevel,
    /// Gambling or simulated gambling.
    pub gambling: AgeRatingLevel,
    /// Horror/fear themes.
    pub horror: AgeRatingLevel,
    /// Mature/suggestive themes.
    pub mature_suggestive: AgeRatingLevel,
    /// Medical/treatment information.
    pub medical: AgeRatingLevel,
    /// Profanity or crude humor.
    pub profanity: AgeRatingLevel,
    /// Sexual content/nudity.
    pub sexual_content: AgeRatingLevel,
    /// Graphic violence.
    pub violence_graphic: AgeRatingLevel,
    /// Cartoon/fantasy violence.
    pub violence_cartoon: AgeRatingLevel,
    /// Realistic violence.
    pub violence_realistic: AgeRatingLevel,
    /// Unrestricted web access.
    pub unrestricted_web_access: bool,
    /// Gambling and contests.
    pub gambling_and_contests: bool,
}

/// Level of content for age rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AgeRatingLevel {
    /// No content of this type.
    #[default]
    None,
    /// Infrequent or mild content.
    InfrequentOrMild,
    /// Frequent or intense content.
    FrequentOrIntense,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localized_metadata_validation() {
        let valid = AppleLocalizedMetadata::new("My App", "A great app description");
        assert!(valid.validate().is_ok());

        let invalid = AppleLocalizedMetadata {
            name: "A".repeat(50),
            description: "Test".to_string(),
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_screenshot_set_count() {
        let mut screenshots = AppleScreenshotSet::default();
        assert!(screenshots.is_empty());

        screenshots.iphone_6_5.push(MediaAsset::default());
        screenshots.ipad_pro_12_9.push(MediaAsset::default());
        assert_eq!(screenshots.total_count(), 2);
        assert!(!screenshots.is_empty());
    }
}
