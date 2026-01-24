//! Google Play Store validation.
//!
//! This module provides validation for Google Play Console metadata, ensuring
//! compliance with Google's requirements before submission.

use std::collections::HashMap;
use std::path::PathBuf;

use crate::types::common::{Dimensions, Locale, MediaAsset};
use crate::types::google_play::{limits, GooglePlayLocalizedMetadata, GooglePlayMetadata, GooglePlayScreenshotSet};

use super::common::{char_count, has_excess_whitespace, is_blank, validate_url};
use super::{Severity, ValidationIssue, ValidationResult};

/// Maximum characters for changelog entries.
const CHANGELOG_MAX: usize = 500;

/// Minimum screenshot dimension (width or height).
const SCREENSHOT_MIN_DIM: u32 = 320;

/// Maximum screenshot dimension (width or height).
const SCREENSHOT_MAX_DIM: u32 = 3840;

/// Recommended TV screenshot dimensions.
const TV_RECOMMENDED_WIDTH: u32 = 1920;
const TV_RECOMMENDED_HEIGHT: u32 = 1080;

/// Minimum Wear OS screenshot dimensions.
const WEAR_MIN_DIM: u32 = 384;

/// Feature graphic required dimensions.
const FEATURE_GRAPHIC_WIDTH: u32 = 1024;
const FEATURE_GRAPHIC_HEIGHT: u32 = 500;

/// Minimum screenshots per active device type.
const MIN_SCREENSHOTS_PER_DEVICE: usize = 2;

/// Maximum screenshots per device type.
const MAX_SCREENSHOTS_PER_DEVICE: usize = 8;

/// Aspect ratio tolerance for 16:9 / 9:16 validation (5% tolerance).
const ASPECT_RATIO_TOLERANCE: f64 = 0.05;

/// Google Play Console metadata validator.
///
/// Validates metadata against Google Play Console requirements, checking for:
/// - Required field presence (title, short_description, full_description)
/// - Character count limits
/// - Screenshot dimension bounds and aspect ratios
/// - Screenshot count limits
/// - Feature graphic dimensions
/// - Changelog length per version
/// - Default locale completeness
/// - Video URL format (YouTube)
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::validation::GooglePlayValidator;
/// use canaveral_metadata::GooglePlayMetadata;
///
/// let metadata = GooglePlayMetadata::new("com.example.app");
/// let validator = GooglePlayValidator::new(false);
/// let result = validator.validate(&metadata);
///
/// if result.is_valid() {
///     println!("Metadata is valid!");
/// } else {
///     for error in result.errors() {
///         eprintln!("{}", error);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GooglePlayValidator {
    /// When true, treats warnings as errors.
    pub strict: bool,
    /// When true, requires feature graphic to be present.
    pub requires_feature_graphic: bool,
}

impl Default for GooglePlayValidator {
    fn default() -> Self {
        Self {
            strict: false,
            requires_feature_graphic: true,
        }
    }
}

impl GooglePlayValidator {
    /// Creates a new Google Play validator.
    ///
    /// # Arguments
    ///
    /// * `strict` - When true, treats warnings as errors
    pub fn new(strict: bool) -> Self {
        Self {
            strict,
            ..Default::default()
        }
    }

    /// Sets whether feature graphic is required.
    pub fn with_feature_graphic_required(mut self, required: bool) -> Self {
        self.requires_feature_graphic = required;
        self
    }

    /// Validates Google Play Store metadata.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata to validate
    ///
    /// # Returns
    ///
    /// A `ValidationResult` containing any issues found.
    pub fn validate(&self, metadata: &GooglePlayMetadata) -> ValidationResult {
        let mut result = ValidationResult::new();

        // Validate package name
        if metadata.package_name.is_empty() {
            result.add_error(
                "package_name",
                "Package name is required",
                Some("Provide a valid package name (e.g., com.example.app)"),
            );
        }

        // Validate default locale exists in localizations
        let default_locale_code = metadata.default_locale.code();
        if !metadata.localizations.contains_key(&default_locale_code) {
            result.add_error(
                "default_locale",
                &format!(
                    "Default locale '{}' not found in localizations",
                    default_locale_code
                ),
                Some("Add localized metadata for the default locale"),
            );
        }

        // Validate each localization
        for (locale_code, localized) in &metadata.localizations {
            let locale = match Locale::new(locale_code) {
                Ok(l) => l,
                Err(_) => {
                    result.add_error(
                        locale_code,
                        &format!("Invalid locale code: {}", locale_code),
                        None,
                    );
                    continue;
                }
            };
            let is_default = locale_code == &default_locale_code;
            self.validate_localized(&locale, localized, is_default, &mut result);
        }

        // Validate screenshots
        self.validate_screenshots(&metadata.screenshots, &mut result);

        // Validate feature graphic
        self.validate_feature_graphic(&metadata.feature_graphic, &mut result);

        // Validate privacy policy URL if present
        if let Some(ref url) = metadata.privacy_policy_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    "privacy_policy_url",
                    "Privacy policy URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }

        // Validate contact website if present
        if let Some(ref url) = metadata.contact_website {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    "contact_website",
                    "Contact website is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }

        // Convert warnings to errors in strict mode
        if self.strict {
            for issue in &mut result.issues {
                if issue.severity == Severity::Warning {
                    issue.severity = Severity::Error;
                }
            }
        }

        result
    }

    /// Validates localized metadata for a specific locale.
    fn validate_localized(
        &self,
        locale: &Locale,
        meta: &GooglePlayLocalizedMetadata,
        is_default: bool,
        result: &mut ValidationResult,
    ) {
        let locale_code = locale.code();
        let field = |name: &str| format!("{}.{}", locale_code, name);

        // Title is required
        if is_blank(&meta.title) {
            result.add_error(
                &field("title"),
                "App title is required",
                Some("Provide a title for your app"),
            );
        } else {
            let title_len = char_count(&meta.title);
            if title_len > limits::TITLE_MAX {
                result.add_error(
                    &field("title"),
                    &format!(
                        "Title exceeds {} characters ({} chars)",
                        limits::TITLE_MAX,
                        title_len
                    ),
                    Some(&format!(
                        "Shorten the title to {} characters or less",
                        limits::TITLE_MAX
                    )),
                );
            }

            if has_excess_whitespace(&meta.title) {
                result.add_warning(
                    &field("title"),
                    "Title has leading or trailing whitespace",
                    Some("Trim whitespace from the title"),
                );
            }
        }

        // Short description is required
        if is_blank(&meta.short_description) {
            result.add_error(
                &field("short_description"),
                "Short description is required",
                Some("Provide a short description for your app"),
            );
        } else {
            let short_desc_len = char_count(&meta.short_description);
            if short_desc_len > limits::SHORT_DESCRIPTION_MAX {
                result.add_error(
                    &field("short_description"),
                    &format!(
                        "Short description exceeds {} characters ({} chars)",
                        limits::SHORT_DESCRIPTION_MAX,
                        short_desc_len
                    ),
                    Some(&format!(
                        "Shorten the short description to {} characters or less",
                        limits::SHORT_DESCRIPTION_MAX
                    )),
                );
            }

            if has_excess_whitespace(&meta.short_description) {
                result.add_warning(
                    &field("short_description"),
                    "Short description has leading or trailing whitespace",
                    Some("Trim whitespace from the short description"),
                );
            }
        }

        // Full description is required
        if is_blank(&meta.full_description) {
            result.add_error(
                &field("full_description"),
                "Full description is required",
                Some("Provide a full description for your app"),
            );
        } else {
            let full_desc_len = char_count(&meta.full_description);
            if full_desc_len > limits::FULL_DESCRIPTION_MAX {
                result.add_error(
                    &field("full_description"),
                    &format!(
                        "Full description exceeds {} characters ({} chars)",
                        limits::FULL_DESCRIPTION_MAX,
                        full_desc_len
                    ),
                    Some(&format!(
                        "Shorten the full description to {} characters or less",
                        limits::FULL_DESCRIPTION_MAX
                    )),
                );
            }

            if has_excess_whitespace(&meta.full_description) {
                result.add_warning(
                    &field("full_description"),
                    "Full description has leading or trailing whitespace",
                    Some("Trim whitespace from the full description"),
                );
            }
        }

        // Validate changelogs
        for (version, changelog) in &meta.changelogs {
            let changelog_len = char_count(changelog);
            if changelog_len > CHANGELOG_MAX {
                result.add_error(
                    &field(&format!("changelogs.{}", version)),
                    &format!(
                        "Changelog for version {} exceeds {} characters ({} chars)",
                        version, CHANGELOG_MAX, changelog_len
                    ),
                    Some(&format!(
                        "Shorten the changelog to {} characters or less",
                        CHANGELOG_MAX
                    )),
                );
            }
        }

        // Validate video URL if present (must be YouTube)
        if let Some(ref video_url) = meta.video_url {
            if !video_url.is_empty() {
                if !validate_youtube_url(video_url) {
                    result.add_error(
                        &field("video_url"),
                        "Video URL must be a valid YouTube URL",
                        Some("Provide a YouTube URL (e.g., https://www.youtube.com/watch?v=... or https://youtu.be/...)"),
                    );
                }
            }
        }

        // For default locale, ensure all required fields are complete
        if is_default && (is_blank(&meta.title) || is_blank(&meta.short_description) || is_blank(&meta.full_description)) {
            result.add_error(
                "default_locale",
                "Default locale must have all required fields (title, short_description, full_description)",
                Some("Complete all required fields for the default locale"),
            );
        }
    }

    /// Validates screenshots for all device types.
    fn validate_screenshots(
        &self,
        screenshots: &GooglePlayScreenshotSet,
        result: &mut ValidationResult,
    ) {
        // Validate phone screenshots
        self.validate_screenshot_set(
            &screenshots.phone,
            "screenshots.phone",
            DeviceType::Phone,
            result,
        );

        // Validate 7" tablet screenshots
        self.validate_screenshot_set(
            &screenshots.tablet_7,
            "screenshots.tablet_7",
            DeviceType::Tablet,
            result,
        );

        // Validate 10" tablet screenshots
        self.validate_screenshot_set(
            &screenshots.tablet_10,
            "screenshots.tablet_10",
            DeviceType::Tablet,
            result,
        );

        // Validate TV screenshots
        self.validate_screenshot_set(
            &screenshots.tv,
            "screenshots.tv",
            DeviceType::Tv,
            result,
        );

        // Validate Wear screenshots
        self.validate_screenshot_set(
            &screenshots.wear,
            "screenshots.wear",
            DeviceType::Wear,
            result,
        );

        // Check if any screenshots exist
        if screenshots.is_empty() {
            result.add_warning(
                "screenshots",
                "No screenshots provided",
                Some("Add at least 2 phone screenshots for your app listing"),
            );
        }
    }

    /// Validates a set of screenshots for a specific device type.
    fn validate_screenshot_set(
        &self,
        screenshots: &[MediaAsset],
        field: &str,
        device_type: DeviceType,
        result: &mut ValidationResult,
    ) {
        let count = screenshots.len();

        // Check minimum count if device type is active (has any screenshots)
        if count > 0 && count < MIN_SCREENSHOTS_PER_DEVICE {
            result.add_error(
                field,
                &format!(
                    "Too few screenshots ({}, minimum {})",
                    count, MIN_SCREENSHOTS_PER_DEVICE
                ),
                Some(&format!(
                    "Add at least {} more screenshot(s)",
                    MIN_SCREENSHOTS_PER_DEVICE - count
                )),
            );
        }

        // Check maximum count
        if count > MAX_SCREENSHOTS_PER_DEVICE {
            result.add_error(
                field,
                &format!(
                    "Too many screenshots ({}, maximum {})",
                    count, MAX_SCREENSHOTS_PER_DEVICE
                ),
                Some(&format!(
                    "Remove {} screenshot(s)",
                    count - MAX_SCREENSHOTS_PER_DEVICE
                )),
            );
        }

        // Check dimensions for each screenshot
        for (i, screenshot) in screenshots.iter().enumerate() {
            if let Some(ref dims) = screenshot.dimensions {
                self.validate_screenshot_dimensions(
                    dims,
                    &format!("{}[{}]", field, i),
                    device_type,
                    result,
                );
            } else {
                result.add_warning(
                    &format!("{}[{}]", field, i),
                    "Screenshot dimensions not specified",
                    Some("Specify dimensions for better validation"),
                );
            }
        }
    }

    /// Validates screenshot dimensions based on device type.
    fn validate_screenshot_dimensions(
        &self,
        dims: &Dimensions,
        field: &str,
        device_type: DeviceType,
        result: &mut ValidationResult,
    ) {
        match device_type {
            DeviceType::Phone | DeviceType::Tablet => {
                // Check min/max bounds
                if dims.width < SCREENSHOT_MIN_DIM || dims.height < SCREENSHOT_MIN_DIM {
                    result.add_error(
                        field,
                        &format!(
                            "Screenshot dimensions {}x{} below minimum {}px",
                            dims.width, dims.height, SCREENSHOT_MIN_DIM
                        ),
                        Some(&format!(
                            "Ensure both width and height are at least {}px",
                            SCREENSHOT_MIN_DIM
                        )),
                    );
                }

                if dims.width > SCREENSHOT_MAX_DIM || dims.height > SCREENSHOT_MAX_DIM {
                    result.add_error(
                        field,
                        &format!(
                            "Screenshot dimensions {}x{} exceed maximum {}px",
                            dims.width, dims.height, SCREENSHOT_MAX_DIM
                        ),
                        Some(&format!(
                            "Ensure both width and height are at most {}px",
                            SCREENSHOT_MAX_DIM
                        )),
                    );
                }

                // Check aspect ratio (approximately 16:9 or 9:16)
                if !is_valid_aspect_ratio(dims) {
                    result.add_warning(
                        field,
                        &format!(
                            "Screenshot aspect ratio {}x{} is not approximately 16:9 or 9:16",
                            dims.width, dims.height
                        ),
                        Some("Consider using 16:9 (landscape) or 9:16 (portrait) aspect ratio"),
                    );
                }
            }
            DeviceType::Tv => {
                // TV: 1920x1080 recommended
                if dims.width != TV_RECOMMENDED_WIDTH || dims.height != TV_RECOMMENDED_HEIGHT {
                    result.add_warning(
                        field,
                        &format!(
                            "TV screenshot dimensions {}x{} differ from recommended {}x{}",
                            dims.width, dims.height, TV_RECOMMENDED_WIDTH, TV_RECOMMENDED_HEIGHT
                        ),
                        Some(&format!(
                            "Use {}x{} for TV screenshots",
                            TV_RECOMMENDED_WIDTH, TV_RECOMMENDED_HEIGHT
                        )),
                    );
                }
            }
            DeviceType::Wear => {
                // Wear: 384x384 minimum
                if dims.width < WEAR_MIN_DIM || dims.height < WEAR_MIN_DIM {
                    result.add_error(
                        field,
                        &format!(
                            "Wear screenshot dimensions {}x{} below minimum {}x{}",
                            dims.width, dims.height, WEAR_MIN_DIM, WEAR_MIN_DIM
                        ),
                        Some(&format!(
                            "Ensure both width and height are at least {}px",
                            WEAR_MIN_DIM
                        )),
                    );
                }
            }
        }
    }

    /// Validates the feature graphic.
    fn validate_feature_graphic(
        &self,
        path: &Option<PathBuf>,
        result: &mut ValidationResult,
    ) {
        match path {
            Some(p) if !p.as_os_str().is_empty() => {
                // Feature graphic exists - we would need to check its dimensions
                // For now, we just note that it's present
                result.add_info(
                    "feature_graphic",
                    &format!(
                        "Feature graphic must be exactly {}x{} pixels",
                        FEATURE_GRAPHIC_WIDTH, FEATURE_GRAPHIC_HEIGHT
                    ),
                    Some("Verify the feature graphic dimensions before upload"),
                );
            }
            _ => {
                if self.requires_feature_graphic {
                    result.add_error(
                        "feature_graphic",
                        "Feature graphic is required",
                        Some(&format!(
                            "Provide a {}x{} PNG or JPEG image",
                            FEATURE_GRAPHIC_WIDTH, FEATURE_GRAPHIC_HEIGHT
                        )),
                    );
                }
            }
        }
    }
}

/// Validates screenshots organized by locale.
pub fn validate_localized_google_play_screenshots(
    screenshots: &HashMap<Locale, GooglePlayScreenshotSet>,
    result: &mut ValidationResult,
) {
    for (locale, screenshot_set) in screenshots {
        let validator = GooglePlayValidator::new(false);
        let mut locale_result = ValidationResult::new();
        validator.validate_screenshots(screenshot_set, &mut locale_result);

        // Prefix all issues with the locale
        for issue in locale_result.issues {
            result.add(ValidationIssue {
                severity: issue.severity,
                field: format!("{}.{}", locale.code(), issue.field),
                message: issue.message,
                suggestion: issue.suggestion,
            });
        }
    }
}

/// Device types for screenshot validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeviceType {
    Phone,
    Tablet,
    Tv,
    Wear,
}

/// Validates if a URL is a valid YouTube URL.
fn validate_youtube_url(url: &str) -> bool {
    let url = url.trim().to_lowercase();

    // Check for valid URL format first
    if !validate_url(&url) {
        return false;
    }

    // Check for YouTube domains
    let youtube_patterns = [
        "youtube.com/watch",
        "www.youtube.com/watch",
        "m.youtube.com/watch",
        "youtu.be/",
        "youtube.com/embed/",
        "www.youtube.com/embed/",
    ];

    youtube_patterns.iter().any(|pattern| url.contains(pattern))
}

/// Checks if dimensions have approximately 16:9 or 9:16 aspect ratio.
fn is_valid_aspect_ratio(dims: &Dimensions) -> bool {
    if dims.width == 0 || dims.height == 0 {
        return false;
    }

    let ratio = dims.width as f64 / dims.height as f64;

    // 16:9 ratio = 1.777...
    let ratio_16_9 = 16.0 / 9.0;
    // 9:16 ratio = 0.5625
    let ratio_9_16 = 9.0 / 16.0;

    let diff_16_9 = (ratio - ratio_16_9).abs() / ratio_16_9;
    let diff_9_16 = (ratio - ratio_9_16).abs() / ratio_9_16;

    diff_16_9 <= ASPECT_RATIO_TOLERANCE || diff_9_16 <= ASPECT_RATIO_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::common::AssetType;
    use std::path::PathBuf;

    fn create_valid_metadata() -> GooglePlayMetadata {
        let mut metadata = GooglePlayMetadata::new("com.example.app");
        metadata.default_locale = Locale::default();
        metadata.feature_graphic = Some(PathBuf::from("feature_graphic.png"));

        let localized = GooglePlayLocalizedMetadata::new(
            "My App",
            "A short description of my app",
            "A longer full description that explains what my app does in detail.",
        );

        metadata.localizations.insert("en-US".to_string(), localized);
        metadata
    }

    #[test]
    fn test_valid_metadata() {
        let metadata = create_valid_metadata();
        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        // Should only have warnings/info, no errors
        assert!(result.is_valid(), "Errors: {:?}", result.errors());
    }

    #[test]
    fn test_missing_required_fields() {
        let mut metadata = GooglePlayMetadata::new("");
        metadata.default_locale = Locale::default();

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.field == "package_name"));
    }

    #[test]
    fn test_title_too_long() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.title = "A".repeat(55); // Exceeds 50 char limit
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("title") && e.message.contains("exceeds")));
    }

    #[test]
    fn test_short_description_too_long() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.short_description = "A".repeat(85); // Exceeds 80 char limit
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("short_description") && e.message.contains("exceeds")));
    }

    #[test]
    fn test_full_description_too_long() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.full_description = "A".repeat(4500); // Exceeds 4000 char limit
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("full_description") && e.message.contains("exceeds")));
    }

    #[test]
    fn test_changelog_too_long() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.changelogs.insert("100".to_string(), "A".repeat(550)); // Exceeds 500 char limit
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("changelogs") && e.message.contains("exceeds")));
    }

    #[test]
    fn test_valid_youtube_url() {
        assert!(validate_youtube_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(validate_youtube_url("https://youtube.com/watch?v=dQw4w9WgXcQ"));
        assert!(validate_youtube_url("https://youtu.be/dQw4w9WgXcQ"));
        assert!(validate_youtube_url("https://www.youtube.com/embed/dQw4w9WgXcQ"));
    }

    #[test]
    fn test_invalid_youtube_url() {
        assert!(!validate_youtube_url("https://vimeo.com/12345"));
        assert!(!validate_youtube_url("https://example.com/video"));
        assert!(!validate_youtube_url("not-a-url"));
        assert!(!validate_youtube_url(""));
    }

    #[test]
    fn test_video_url_validation() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.video_url = Some("https://vimeo.com/12345".to_string()); // Not YouTube
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("video_url") && e.message.contains("YouTube")));
    }

    #[test]
    fn test_screenshot_dimensions_valid() {
        let mut metadata = create_valid_metadata();

        // Add valid 16:9 screenshot
        let screenshot = MediaAsset::new(PathBuf::from("screen1.png"), AssetType::Screenshot)
            .with_dimensions(1920, 1080);
        metadata.screenshots.phone.push(screenshot);

        // Add valid 9:16 screenshot
        let screenshot = MediaAsset::new(PathBuf::from("screen2.png"), AssetType::Screenshot)
            .with_dimensions(1080, 1920);
        metadata.screenshots.phone.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        // Should have no errors about dimensions
        assert!(result.is_valid(), "Errors: {:?}", result.errors());
    }

    #[test]
    fn test_screenshot_dimensions_too_small() {
        let mut metadata = create_valid_metadata();

        // Add screenshot that's too small
        let screenshot = MediaAsset::new(PathBuf::from("screen1.png"), AssetType::Screenshot)
            .with_dimensions(200, 300);
        metadata.screenshots.phone.push(screenshot.clone());
        metadata.screenshots.phone.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("phone") && e.message.contains("below minimum")));
    }

    #[test]
    fn test_screenshot_dimensions_too_large() {
        let mut metadata = create_valid_metadata();

        // Add screenshot that's too large
        let screenshot = MediaAsset::new(PathBuf::from("screen1.png"), AssetType::Screenshot)
            .with_dimensions(5000, 4000);
        metadata.screenshots.phone.push(screenshot.clone());
        metadata.screenshots.phone.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("phone") && e.message.contains("exceed maximum")));
    }

    #[test]
    fn test_screenshot_count_too_few() {
        let mut metadata = create_valid_metadata();

        // Add only 1 screenshot (minimum is 2)
        let screenshot = MediaAsset::new(PathBuf::from("screen1.png"), AssetType::Screenshot)
            .with_dimensions(1920, 1080);
        metadata.screenshots.phone.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("phone") && e.message.contains("Too few screenshots")));
    }

    #[test]
    fn test_screenshot_count_too_many() {
        let mut metadata = create_valid_metadata();

        // Add 10 screenshots (maximum is 8)
        for i in 0..10 {
            let screenshot = MediaAsset::new(
                PathBuf::from(format!("screen{}.png", i)),
                AssetType::Screenshot,
            )
            .with_dimensions(1920, 1080);
            metadata.screenshots.phone.push(screenshot);
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("phone") && e.message.contains("Too many screenshots")));
    }

    #[test]
    fn test_missing_feature_graphic() {
        let mut metadata = create_valid_metadata();
        metadata.feature_graphic = None;

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field == "feature_graphic" && e.message.contains("required")));
    }

    #[test]
    fn test_feature_graphic_not_required() {
        let mut metadata = create_valid_metadata();
        metadata.feature_graphic = None;

        let validator = GooglePlayValidator::new(false).with_feature_graphic_required(false);
        let result = validator.validate(&metadata);

        // Should be valid when feature graphic is not required
        assert!(result.is_valid(), "Errors: {:?}", result.errors());
    }

    #[test]
    fn test_aspect_ratio_validation() {
        // Exact 16:9
        assert!(is_valid_aspect_ratio(&Dimensions::new(1920, 1080)));
        // Exact 9:16
        assert!(is_valid_aspect_ratio(&Dimensions::new(1080, 1920)));
        // Within tolerance
        assert!(is_valid_aspect_ratio(&Dimensions::new(1280, 720)));
        // Outside tolerance (1:1)
        assert!(!is_valid_aspect_ratio(&Dimensions::new(1000, 1000)));
        // Outside tolerance (4:3)
        assert!(!is_valid_aspect_ratio(&Dimensions::new(1024, 768)));
    }

    #[test]
    fn test_whitespace_warnings() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.title = " My App ".to_string(); // Leading/trailing whitespace
        }

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(result.is_valid()); // Whitespace is a warning, not an error
        assert!(result
            .warnings()
            .iter()
            .any(|w| w.field.contains("title") && w.message.contains("whitespace")));
    }

    #[test]
    fn test_strict_mode() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.title = " My App ".to_string(); // Whitespace warning
        }

        let validator = GooglePlayValidator::new(true); // Strict mode
        let result = validator.validate(&metadata);

        // In strict mode, warnings become errors
        assert!(!result.is_valid());
    }

    #[test]
    fn test_default_locale_missing() {
        let mut metadata = create_valid_metadata();
        metadata.default_locale = Locale::new("de-DE").unwrap();
        // But we only have en-US in localizations

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.field == "default_locale"));
    }

    #[test]
    fn test_wear_screenshot_dimensions() {
        let mut metadata = create_valid_metadata();

        // Add wear screenshot that's too small
        let screenshot = MediaAsset::new(PathBuf::from("wear1.png"), AssetType::Screenshot)
            .with_dimensions(300, 300);
        metadata.screenshots.wear.push(screenshot.clone());
        metadata.screenshots.wear.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("wear") && e.message.contains("below minimum")));
    }

    #[test]
    fn test_tv_screenshot_dimensions_warning() {
        let mut metadata = create_valid_metadata();

        // Add TV screenshot with non-recommended dimensions
        let screenshot = MediaAsset::new(PathBuf::from("tv1.png"), AssetType::Screenshot)
            .with_dimensions(1280, 720);
        metadata.screenshots.tv.push(screenshot.clone());
        metadata.screenshots.tv.push(screenshot);

        let validator = GooglePlayValidator::new(false);
        let result = validator.validate(&metadata);

        // Should be valid but have a warning
        assert!(result.is_valid());
        assert!(result
            .warnings()
            .iter()
            .any(|w| w.field.contains("tv") && w.message.contains("recommended")));
    }
}
