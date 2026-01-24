//! Google Play Store metadata types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use super::common::{Locale, MediaAsset};

/// Character limits for Google Play Store metadata fields.
pub mod limits {
    /// Maximum characters for app title.
    pub const TITLE_MAX: usize = 50;
    /// Maximum characters for short description.
    pub const SHORT_DESCRIPTION_MAX: usize = 80;
    /// Maximum characters for full description.
    pub const FULL_DESCRIPTION_MAX: usize = 4000;
}

/// Full app metadata for Google Play Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GooglePlayMetadata {
    /// Application package name.
    pub package_name: String,
    /// Default locale.
    pub default_locale: Locale,
    /// Localized metadata by locale.
    pub localizations: HashMap<String, GooglePlayLocalizedMetadata>,
    /// App category.
    pub category: Option<GooglePlayCategory>,
    /// Content rating.
    pub content_rating: Option<GooglePlayContentRating>,
    /// Screenshots by device type.
    pub screenshots: GooglePlayScreenshotSet,
    /// Feature graphic path.
    pub feature_graphic: Option<PathBuf>,
    /// Promo graphic path.
    pub promo_graphic: Option<PathBuf>,
    /// TV banner path.
    pub tv_banner: Option<PathBuf>,
    /// App icon path.
    pub icon: Option<PathBuf>,
    /// Privacy policy URL.
    pub privacy_policy_url: Option<String>,
    /// Contact email.
    pub contact_email: Option<String>,
    /// Contact phone.
    pub contact_phone: Option<String>,
    /// Contact website.
    pub contact_website: Option<String>,
}

impl GooglePlayMetadata {
    /// Creates new Google Play metadata with the given package name.
    pub fn new(package_name: impl Into<String>) -> Self {
        Self {
            package_name: package_name.into(),
            ..Default::default()
        }
    }

    /// Gets localized metadata for a specific locale.
    pub fn get_localization(&self, locale: &str) -> Option<&GooglePlayLocalizedMetadata> {
        self.localizations.get(locale)
    }

    /// Sets localized metadata for a specific locale.
    pub fn set_localization(
        &mut self,
        locale: impl Into<String>,
        metadata: GooglePlayLocalizedMetadata,
    ) {
        self.localizations.insert(locale.into(), metadata);
    }
}

/// Locale-specific content for Google Play Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GooglePlayLocalizedMetadata {
    /// App title (max 50 characters).
    pub title: String,
    /// Short description (max 80 characters).
    pub short_description: String,
    /// Full description (max 4000 characters).
    pub full_description: String,
    /// Changelogs by version code.
    pub changelogs: HashMap<String, String>,
    /// Video URL (YouTube).
    pub video_url: Option<String>,
}

impl GooglePlayLocalizedMetadata {
    /// Creates new localized metadata with required fields.
    pub fn new(
        title: impl Into<String>,
        short_description: impl Into<String>,
        full_description: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            short_description: short_description.into(),
            full_description: full_description.into(),
            ..Default::default()
        }
    }

    /// Validates the metadata against character limits.
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.title.chars().count() > limits::TITLE_MAX {
            errors.push(format!(
                "Title exceeds {} characters ({})",
                limits::TITLE_MAX,
                self.title.chars().count()
            ));
        }

        if self.short_description.chars().count() > limits::SHORT_DESCRIPTION_MAX {
            errors.push(format!(
                "Short description exceeds {} characters ({})",
                limits::SHORT_DESCRIPTION_MAX,
                self.short_description.chars().count()
            ));
        }

        if self.full_description.chars().count() > limits::FULL_DESCRIPTION_MAX {
            errors.push(format!(
                "Full description exceeds {} characters ({})",
                limits::FULL_DESCRIPTION_MAX,
                self.full_description.chars().count()
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Adds a changelog entry for a version.
    pub fn add_changelog(&mut self, version_code: impl Into<String>, changelog: impl Into<String>) {
        self.changelogs.insert(version_code.into(), changelog.into());
    }
}

/// Screenshots organized by device type for Google Play.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GooglePlayScreenshotSet {
    /// Phone screenshots.
    pub phone: Vec<MediaAsset>,
    /// 7" tablet screenshots.
    pub tablet_7: Vec<MediaAsset>,
    /// 10" tablet screenshots.
    pub tablet_10: Vec<MediaAsset>,
    /// Android TV screenshots.
    pub tv: Vec<MediaAsset>,
    /// Wear OS screenshots.
    pub wear: Vec<MediaAsset>,
}

impl GooglePlayScreenshotSet {
    /// Returns the total number of screenshots across all device types.
    pub fn total_count(&self) -> usize {
        self.phone.len()
            + self.tablet_7.len()
            + self.tablet_10.len()
            + self.tv.len()
            + self.wear.len()
    }

    /// Checks if any screenshots are present.
    pub fn is_empty(&self) -> bool {
        self.total_count() == 0
    }
}

/// Google Play Store categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GooglePlayCategory {
    /// Art & Design category.
    ArtAndDesign,
    /// Auto & Vehicles category.
    AutoAndVehicles,
    /// Beauty category.
    Beauty,
    /// Books & Reference category.
    BooksAndReference,
    /// Business category.
    Business,
    /// Comics category.
    Comics,
    /// Communication category.
    Communication,
    /// Dating category.
    Dating,
    /// Education category.
    Education,
    /// Entertainment category.
    Entertainment,
    /// Events category.
    Events,
    /// Finance category.
    Finance,
    /// Food & Drink category.
    FoodAndDrink,
    /// Games category.
    Games,
    /// Health & Fitness category.
    HealthAndFitness,
    /// House & Home category.
    HouseAndHome,
    /// Libraries & Demo category.
    LibrariesAndDemo,
    /// Lifestyle category.
    Lifestyle,
    /// Maps & Navigation category.
    MapsAndNavigation,
    /// Medical category.
    Medical,
    /// Music & Audio category.
    MusicAndAudio,
    /// News & Magazines category.
    NewsAndMagazines,
    /// Parenting category.
    Parenting,
    /// Personalization category.
    Personalization,
    /// Photography category.
    Photography,
    /// Productivity category.
    Productivity,
    /// Shopping category.
    Shopping,
    /// Social category.
    Social,
    /// Sports category.
    Sports,
    /// Tools category.
    Tools,
    /// Travel & Local category.
    TravelAndLocal,
    /// Video Players & Editors category.
    VideoPlayersAndEditors,
    /// Weather category.
    Weather,
}

impl Default for GooglePlayCategory {
    fn default() -> Self {
        GooglePlayCategory::Tools
    }
}

/// Content rating for Google Play Store.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GooglePlayContentRating {
    /// Violence level.
    pub violence: ContentRatingLevel,
    /// Sexual content level.
    pub sexual_content: ContentRatingLevel,
    /// Language level (profanity).
    pub language: ContentRatingLevel,
    /// Controlled substance references.
    pub controlled_substance: ContentRatingLevel,
    /// User-generated content.
    pub user_generated_content: bool,
    /// Shares user location.
    pub shares_location: bool,
    /// Contains ads.
    pub contains_ads: bool,
    /// Digital purchases.
    pub digital_purchases: bool,
}

impl GooglePlayContentRating {
    /// Creates a content rating with all values set to none.
    pub fn none() -> Self {
        Self::default()
    }

    /// Creates a content rating suitable for everyone.
    pub fn everyone() -> Self {
        Self {
            violence: ContentRatingLevel::None,
            sexual_content: ContentRatingLevel::None,
            language: ContentRatingLevel::None,
            controlled_substance: ContentRatingLevel::None,
            user_generated_content: false,
            shares_location: false,
            contains_ads: false,
            digital_purchases: false,
        }
    }
}

/// Content rating level for Google Play.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContentRatingLevel {
    /// No content of this type.
    #[default]
    None,
    /// Mild content.
    Mild,
    /// Moderate content.
    Moderate,
    /// Intense content.
    Intense,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localized_metadata_validation() {
        let valid = GooglePlayLocalizedMetadata::new(
            "My App",
            "A short description",
            "A longer full description of the app.",
        );
        assert!(valid.validate().is_ok());

        let invalid = GooglePlayLocalizedMetadata {
            title: "A".repeat(100),
            short_description: "Test".to_string(),
            full_description: "Test".to_string(),
            ..Default::default()
        };
        assert!(invalid.validate().is_err());
    }

    #[test]
    fn test_screenshot_set_count() {
        let mut screenshots = GooglePlayScreenshotSet::default();
        assert!(screenshots.is_empty());

        screenshots.phone.push(MediaAsset::default());
        screenshots.tablet_7.push(MediaAsset::default());
        screenshots.tv.push(MediaAsset::default());
        assert_eq!(screenshots.total_count(), 3);
        assert!(!screenshots.is_empty());
    }

    #[test]
    fn test_changelog_management() {
        let mut metadata = GooglePlayLocalizedMetadata::new("App", "Short", "Full");
        metadata.add_changelog("100", "Initial release");
        metadata.add_changelog("101", "Bug fixes");

        assert_eq!(metadata.changelogs.get("100"), Some(&"Initial release".to_string()));
        assert_eq!(metadata.changelogs.get("101"), Some(&"Bug fixes".to_string()));
    }
}
