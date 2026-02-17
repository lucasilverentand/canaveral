//! Common types shared across platforms.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::MetadataError;

/// Supported publishing platforms (app stores and package registries).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum Platform {
    /// Apple App Store (iOS, macOS, tvOS, watchOS).
    #[default]
    Apple,
    /// Google Play Store (Android).
    GooglePlay,
    /// NPM package registry (JavaScript/TypeScript).
    Npm,
    /// Crates.io Rust package registry.
    Crates,
    /// Python Package Index (PyPI).
    PyPI,
}

/// A locale identifier with validation.
///
/// Represents a BCP 47 language tag (e.g., "en-US", "de-DE", "ja").
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Locale {
    /// The language code (e.g., "en", "de", "ja").
    language: String,
    /// Optional region code (e.g., "US", "DE", "JP").
    region: Option<String>,
}

impl Locale {
    /// Creates a new locale from a language code.
    ///
    /// # Arguments
    ///
    /// * `code` - A BCP 47 language tag (e.g., "en-US", "de-DE", "ja")
    ///
    /// # Errors
    ///
    /// Returns an error if the locale code is invalid.
    pub fn new(code: &str) -> Result<Self, MetadataError> {
        let code = code.trim();
        if code.is_empty() {
            return Err(MetadataError::InvalidFormat(
                "Locale code cannot be empty".to_string(),
            ));
        }

        let parts: Vec<&str> = code.split(&['-', '_'][..]).collect();

        let language = parts[0].to_lowercase();
        if language.len() < 2 || language.len() > 3 {
            return Err(MetadataError::InvalidFormat(format!(
                "Invalid language code: {}",
                language
            )));
        }

        let region = if parts.len() > 1 {
            let region = parts[1].to_uppercase();
            if region.len() != 2 {
                return Err(MetadataError::InvalidFormat(format!(
                    "Invalid region code: {}",
                    region
                )));
            }
            Some(region)
        } else {
            None
        };

        Ok(Self { language, region })
    }

    /// Returns the language code.
    pub fn language(&self) -> &str {
        &self.language
    }

    /// Returns the region code, if present.
    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    /// Returns the full locale code (e.g., "en-US").
    pub fn code(&self) -> String {
        match &self.region {
            Some(region) => format!("{}-{}", self.language, region),
            None => self.language.clone(),
        }
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            region: Some("US".to_string()),
        }
    }
}

impl TryFrom<String> for Locale {
    type Error = MetadataError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Locale::new(&value)
    }
}

impl From<Locale> for String {
    fn from(locale: Locale) -> Self {
        locale.code()
    }
}

impl std::fmt::Display for Locale {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Types of media assets for app store listings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum AssetType {
    /// App icon.
    Icon,
    /// Screenshot image.
    #[default]
    Screenshot,
    /// Video preview.
    Preview,
    /// Feature graphic (Google Play).
    FeatureGraphic,
}

/// Dimensions for a media asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct Dimensions {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Dimensions {
    /// Creates new dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

/// A media asset for app store listings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaAsset {
    /// Path to the asset file.
    pub path: PathBuf,
    /// Asset dimensions.
    pub dimensions: Option<Dimensions>,
    /// Locale for the asset.
    pub locale: Option<Locale>,
    /// Asset type.
    pub asset_type: AssetType,
}

impl MediaAsset {
    /// Creates a new media asset.
    pub fn new(path: PathBuf, asset_type: AssetType) -> Self {
        Self {
            path,
            dimensions: None,
            locale: None,
            asset_type,
        }
    }

    /// Sets the dimensions for the asset.
    pub fn with_dimensions(mut self, width: u32, height: u32) -> Self {
        self.dimensions = Some(Dimensions::new(width, height));
        self
    }

    /// Sets the locale for the asset.
    pub fn with_locale(mut self, locale: Locale) -> Self {
        self.locale = Some(locale);
        self
    }
}

impl Default for MediaAsset {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            dimensions: None,
            locale: None,
            asset_type: AssetType::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locale_parsing() {
        let locale = Locale::new("en-US").unwrap();
        assert_eq!(locale.language(), "en");
        assert_eq!(locale.region(), Some("US"));
        assert_eq!(locale.code(), "en-US");

        let locale = Locale::new("de_DE").unwrap();
        assert_eq!(locale.language(), "de");
        assert_eq!(locale.region(), Some("DE"));

        let locale = Locale::new("ja").unwrap();
        assert_eq!(locale.language(), "ja");
        assert_eq!(locale.region(), None);
    }

    #[test]
    fn test_locale_validation() {
        assert!(Locale::new("").is_err());
        assert!(Locale::new("x").is_err());
        assert!(Locale::new("en-USA").is_err());
    }
}
