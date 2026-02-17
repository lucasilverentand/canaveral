//! Apple App Store validation.
//!
//! This module provides validation for Apple App Store metadata, ensuring
//! compliance with Apple's requirements before submission.

use std::collections::HashMap;

use tracing::{debug, info};

use crate::types::apple::{
    limits, AgeRatingLevel, AppleAgeRating, AppleLocalizedMetadata, AppleMetadata,
    AppleScreenshotSet,
};
use crate::types::common::{Dimensions, Locale, MediaAsset};

use super::common::{
    char_count, contains_newlines, has_excess_whitespace, is_blank, validate_keywords_format,
    validate_url,
};
use super::{Severity, ValidationIssue, ValidationResult};

/// Valid screenshot dimensions for iPhone 6.5" display.
const IPHONE_6_5_DIMENSIONS: &[(u32, u32)] = &[
    (1242, 2688), // Portrait
    (2688, 1242), // Landscape
    (1284, 2778), // iPhone 12/13/14 Pro Max Portrait
    (2778, 1284), // iPhone 12/13/14 Pro Max Landscape
    (1290, 2796), // iPhone 14 Pro Max Portrait
    (2796, 1290), // iPhone 14 Pro Max Landscape
];

/// Valid screenshot dimensions for iPhone 5.5" display.
const IPHONE_5_5_DIMENSIONS: &[(u32, u32)] = &[
    (1242, 2208), // Portrait
    (2208, 1242), // Landscape
];

/// Valid screenshot dimensions for iPad Pro 12.9" display.
const IPAD_PRO_12_9_DIMENSIONS: &[(u32, u32)] = &[
    (2048, 2732), // Portrait
    (2732, 2048), // Landscape
];

/// Valid screenshot dimensions for iPad Pro 11" display.
const IPAD_PRO_11_DIMENSIONS: &[(u32, u32)] = &[
    (1668, 2388), // Portrait
    (2388, 1668), // Landscape
];

/// Maximum number of screenshots per device type.
const MAX_SCREENSHOTS_PER_DEVICE: usize = 10;

/// Minimum number of screenshots required per device type (when device type is used).
#[allow(dead_code)]
const MIN_SCREENSHOTS_PER_DEVICE: usize = 1;

/// Apple App Store metadata validator.
///
/// Validates metadata against Apple's App Store requirements, checking for:
/// - Required field presence
/// - Character count limits
/// - URL format validation
/// - Screenshot dimension validation
/// - Screenshot count limits
/// - Primary locale completeness
/// - Keywords format
/// - Whitespace issues
///
/// # Example
///
/// ```rust
/// use canaveral_metadata::validation::AppleValidator;
/// use canaveral_metadata::AppleMetadata;
///
/// let metadata = AppleMetadata::new("com.example.app");
/// let validator = AppleValidator::new(false);
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
pub struct AppleValidator {
    /// When true, treats warnings as errors.
    pub strict: bool,
    /// When true, requires what's_new for updates.
    pub is_update: bool,
    /// When true, requires privacy policy URL.
    pub requires_privacy_policy: bool,
}

impl Default for AppleValidator {
    fn default() -> Self {
        Self {
            strict: false,
            is_update: false,
            requires_privacy_policy: true,
        }
    }
}

impl AppleValidator {
    /// Creates a new Apple validator.
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

    /// Creates a validator for app updates.
    ///
    /// This enables validation of the `whats_new` field as required.
    pub fn for_update(strict: bool) -> Self {
        Self {
            strict,
            is_update: true,
            requires_privacy_policy: true,
        }
    }

    /// Sets whether privacy policy is required.
    pub fn with_privacy_policy_required(mut self, required: bool) -> Self {
        self.requires_privacy_policy = required;
        self
    }

    /// Validates Apple App Store metadata.
    ///
    /// # Arguments
    ///
    /// * `metadata` - The metadata to validate
    ///
    /// # Returns
    ///
    /// A `ValidationResult` containing any issues found.
    pub fn validate(&self, metadata: &AppleMetadata) -> ValidationResult {
        debug!(
            bundle_id = %metadata.bundle_id,
            locale_count = metadata.localizations.len(),
            strict = self.strict,
            "validating Apple metadata"
        );
        let mut result = ValidationResult::new();

        // Validate bundle ID
        if metadata.bundle_id.is_empty() {
            result.add_error(
                "bundle_id",
                "Bundle ID is required",
                Some("Provide a valid bundle identifier (e.g., com.example.app)"),
            );
        }

        // Validate primary locale exists in localizations
        let primary_locale_code = metadata.primary_locale.code();
        if !metadata.localizations.contains_key(&primary_locale_code) {
            result.add_error(
                "primary_locale",
                &format!(
                    "Primary locale '{}' not found in localizations",
                    primary_locale_code
                ),
                Some("Add localized metadata for the primary locale"),
            );
        }

        // Validate global URLs
        self.validate_global_urls(metadata, &mut result);

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
            let is_primary = locale_code == &primary_locale_code;
            self.validate_localized(&locale, localized, is_primary, &mut result);
        }

        // Validate screenshots
        self.validate_screenshots(&metadata.screenshots, &mut result);

        // Validate age rating if present
        if let Some(ref age_rating) = metadata.age_rating {
            self.validate_age_rating(age_rating, &mut result);
        }

        // Convert warnings to errors in strict mode
        if self.strict {
            for issue in &mut result.issues {
                if issue.severity == Severity::Warning {
                    issue.severity = Severity::Error;
                }
            }
        }

        info!(
            errors = result.error_count(),
            warnings = result.warning_count(),
            "Apple metadata validation complete"
        );

        result
    }

    /// Validates global URLs (at the metadata level).
    fn validate_global_urls(&self, metadata: &AppleMetadata, result: &mut ValidationResult) {
        // Support URL is required
        match &metadata.support_url {
            Some(url) if !url.is_empty() => {
                if !validate_url(url) {
                    result.add_error(
                        "support_url",
                        "Support URL is not a valid URL",
                        Some("Provide a valid HTTP or HTTPS URL"),
                    );
                }
            }
            _ => {
                result.add_error(
                    "support_url",
                    "Support URL is required",
                    Some("Provide a support URL for your app"),
                );
            }
        }

        // Marketing URL is optional but must be valid if present
        if let Some(ref url) = metadata.marketing_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    "marketing_url",
                    "Marketing URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }

        // Privacy policy URL
        if self.requires_privacy_policy {
            match &metadata.privacy_policy_url {
                Some(url) if !url.is_empty() => {
                    if !validate_url(url) {
                        result.add_error(
                            "privacy_policy_url",
                            "Privacy policy URL is not a valid URL",
                            Some("Provide a valid HTTP or HTTPS URL"),
                        );
                    }
                }
                _ => {
                    result.add_error(
                        "privacy_policy_url",
                        "Privacy policy URL is required for most app categories",
                        Some("Provide a privacy policy URL"),
                    );
                }
            }
        } else if let Some(ref url) = metadata.privacy_policy_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    "privacy_policy_url",
                    "Privacy policy URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }
    }

    /// Validates localized metadata for a specific locale.
    fn validate_localized(
        &self,
        locale: &Locale,
        meta: &AppleLocalizedMetadata,
        is_primary: bool,
        result: &mut ValidationResult,
    ) {
        let locale_code = locale.code();
        let field = |name: &str| format!("{}.{}", locale_code, name);

        // Name is required
        if is_blank(&meta.name) {
            result.add_error(
                &field("name"),
                "App name is required",
                Some("Provide a name for your app"),
            );
        } else {
            // Check length
            let name_len = char_count(&meta.name);
            if name_len > limits::NAME_MAX {
                result.add_error(
                    &field("name"),
                    &format!(
                        "App name exceeds {} characters ({} chars)",
                        limits::NAME_MAX,
                        name_len
                    ),
                    Some(&format!(
                        "Shorten the name to {} characters or less",
                        limits::NAME_MAX
                    )),
                );
            }

            // Check whitespace
            if has_excess_whitespace(&meta.name) {
                result.add_warning(
                    &field("name"),
                    "App name has leading or trailing whitespace",
                    Some("Trim whitespace from the name"),
                );
            }

            // Check for newlines (not allowed in name)
            if contains_newlines(&meta.name) {
                result.add_error(
                    &field("name"),
                    "App name cannot contain newlines",
                    Some("Remove newlines from the name"),
                );
            }
        }

        // Description is required
        if is_blank(&meta.description) {
            result.add_error(
                &field("description"),
                "App description is required",
                Some("Provide a description for your app"),
            );
        } else {
            let desc_len = char_count(&meta.description);
            if desc_len > limits::DESCRIPTION_MAX {
                result.add_error(
                    &field("description"),
                    &format!(
                        "Description exceeds {} characters ({} chars)",
                        limits::DESCRIPTION_MAX,
                        desc_len
                    ),
                    Some(&format!(
                        "Shorten the description to {} characters or less",
                        limits::DESCRIPTION_MAX
                    )),
                );
            }

            if has_excess_whitespace(&meta.description) {
                result.add_warning(
                    &field("description"),
                    "Description has leading or trailing whitespace",
                    Some("Trim whitespace from the description"),
                );
            }
        }

        // Subtitle (optional, max 30 chars)
        if let Some(ref subtitle) = meta.subtitle {
            if !subtitle.is_empty() {
                let subtitle_len = char_count(subtitle);
                if subtitle_len > limits::SUBTITLE_MAX {
                    result.add_error(
                        &field("subtitle"),
                        &format!(
                            "Subtitle exceeds {} characters ({} chars)",
                            limits::SUBTITLE_MAX,
                            subtitle_len
                        ),
                        Some(&format!(
                            "Shorten the subtitle to {} characters or less",
                            limits::SUBTITLE_MAX
                        )),
                    );
                }

                if has_excess_whitespace(subtitle) {
                    result.add_warning(
                        &field("subtitle"),
                        "Subtitle has leading or trailing whitespace",
                        Some("Trim whitespace from the subtitle"),
                    );
                }

                if contains_newlines(subtitle) {
                    result.add_error(
                        &field("subtitle"),
                        "Subtitle cannot contain newlines",
                        Some("Remove newlines from the subtitle"),
                    );
                }
            }
        }

        // Keywords (optional, max 100 chars)
        if let Some(ref keywords) = meta.keywords {
            if !keywords.is_empty() {
                let keywords_len = char_count(keywords);
                if keywords_len > limits::KEYWORDS_MAX {
                    result.add_error(
                        &field("keywords"),
                        &format!(
                            "Keywords exceed {} characters ({} chars)",
                            limits::KEYWORDS_MAX,
                            keywords_len
                        ),
                        Some(&format!(
                            "Shorten keywords to {} characters or less",
                            limits::KEYWORDS_MAX
                        )),
                    );
                }

                let (valid, has_spaces) = validate_keywords_format(keywords);
                if !valid {
                    result.add_error(
                        &field("keywords"),
                        "Keywords format is invalid",
                        Some("Keywords should be comma-separated without leading/trailing commas"),
                    );
                }
                if has_spaces {
                    result.add_info(
                        &field("keywords"),
                        "Keywords have spaces after commas",
                        Some("Remove spaces after commas to maximize character usage"),
                    );
                }

                if contains_newlines(keywords) {
                    result.add_error(
                        &field("keywords"),
                        "Keywords cannot contain newlines",
                        Some("Remove newlines from keywords"),
                    );
                }
            }
        }

        // What's new (required for updates, max 4000 chars)
        if let Some(ref whats_new) = meta.whats_new {
            if !whats_new.is_empty() {
                let whats_new_len = char_count(whats_new);
                if whats_new_len > limits::WHATS_NEW_MAX {
                    result.add_error(
                        &field("whats_new"),
                        &format!(
                            "What's new exceeds {} characters ({} chars)",
                            limits::WHATS_NEW_MAX,
                            whats_new_len
                        ),
                        Some(&format!(
                            "Shorten what's new to {} characters or less",
                            limits::WHATS_NEW_MAX
                        )),
                    );
                }

                if has_excess_whitespace(whats_new) {
                    result.add_warning(
                        &field("whats_new"),
                        "What's new has leading or trailing whitespace",
                        Some("Trim whitespace"),
                    );
                }
            }
        } else if self.is_update && is_primary {
            result.add_error(
                &field("whats_new"),
                "What's new is required for app updates",
                Some("Describe what changed in this version"),
            );
        }

        // Promotional text (optional, max 170 chars)
        if let Some(ref promo) = meta.promotional_text {
            if !promo.is_empty() {
                let promo_len = char_count(promo);
                if promo_len > limits::PROMOTIONAL_TEXT_MAX {
                    result.add_error(
                        &field("promotional_text"),
                        &format!(
                            "Promotional text exceeds {} characters ({} chars)",
                            limits::PROMOTIONAL_TEXT_MAX,
                            promo_len
                        ),
                        Some(&format!(
                            "Shorten promotional text to {} characters or less",
                            limits::PROMOTIONAL_TEXT_MAX
                        )),
                    );
                }

                if has_excess_whitespace(promo) {
                    result.add_warning(
                        &field("promotional_text"),
                        "Promotional text has leading or trailing whitespace",
                        Some("Trim whitespace"),
                    );
                }
            }
        }

        // Localized URLs (override global)
        if let Some(ref url) = meta.support_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    &field("support_url"),
                    "Support URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }

        if let Some(ref url) = meta.marketing_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    &field("marketing_url"),
                    "Marketing URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }

        if let Some(ref url) = meta.privacy_policy_url {
            if !url.is_empty() && !validate_url(url) {
                result.add_error(
                    &field("privacy_policy_url"),
                    "Privacy policy URL is not a valid URL",
                    Some("Provide a valid HTTP or HTTPS URL"),
                );
            }
        }
    }

    /// Validates screenshots.
    fn validate_screenshots(
        &self,
        screenshots: &AppleScreenshotSet,
        result: &mut ValidationResult,
    ) {
        // iPhone 6.5"
        self.validate_screenshot_set(
            &screenshots.iphone_6_5,
            "screenshots.iphone_6_5",
            IPHONE_6_5_DIMENSIONS,
            result,
        );

        // iPhone 5.5"
        self.validate_screenshot_set(
            &screenshots.iphone_5_5,
            "screenshots.iphone_5_5",
            IPHONE_5_5_DIMENSIONS,
            result,
        );

        // iPad Pro 12.9"
        self.validate_screenshot_set(
            &screenshots.ipad_pro_12_9,
            "screenshots.ipad_pro_12_9",
            IPAD_PRO_12_9_DIMENSIONS,
            result,
        );

        // iPad Pro 11"
        self.validate_screenshot_set(
            &screenshots.ipad_pro_11,
            "screenshots.ipad_pro_11",
            IPAD_PRO_11_DIMENSIONS,
            result,
        );

        // For now, we don't validate dimensions for Mac, Apple TV, Apple Watch
        // as they have many valid sizes
        self.validate_screenshot_count(&screenshots.mac, "screenshots.mac", result);
        self.validate_screenshot_count(&screenshots.apple_tv, "screenshots.apple_tv", result);
        self.validate_screenshot_count(&screenshots.apple_watch, "screenshots.apple_watch", result);

        // Check if any screenshots exist for required device types
        if screenshots.is_empty() {
            result.add_warning(
                "screenshots",
                "No screenshots provided",
                Some("Add screenshots for at least iPhone 6.5\" and iPhone 5.5\" displays"),
            );
        }
    }

    /// Validates a set of screenshots for a specific device type.
    fn validate_screenshot_set(
        &self,
        screenshots: &[MediaAsset],
        field: &str,
        valid_dimensions: &[(u32, u32)],
        result: &mut ValidationResult,
    ) {
        // Check count
        self.validate_screenshot_count(screenshots, field, result);

        // Check dimensions for each screenshot
        for (i, screenshot) in screenshots.iter().enumerate() {
            if let Some(ref dims) = screenshot.dimensions {
                if !is_valid_dimension(dims, valid_dimensions) {
                    let valid_dims_str = valid_dimensions
                        .iter()
                        .map(|(w, h)| format!("{}x{}", w, h))
                        .collect::<Vec<_>>()
                        .join(", ");

                    result.add_error(
                        &format!("{}[{}]", field, i),
                        &format!(
                            "Screenshot has invalid dimensions {}x{}",
                            dims.width, dims.height
                        ),
                        Some(&format!("Valid dimensions: {}", valid_dims_str)),
                    );
                }
            } else {
                result.add_warning(
                    &format!("{}[{}]", field, i),
                    "Screenshot dimensions not specified",
                    Some("Specify dimensions for better validation"),
                );
            }
        }
    }

    /// Validates screenshot count for a device type.
    fn validate_screenshot_count(
        &self,
        screenshots: &[MediaAsset],
        field: &str,
        result: &mut ValidationResult,
    ) {
        let count = screenshots.len();

        if count > MAX_SCREENSHOTS_PER_DEVICE {
            result.add_error(
                field,
                &format!(
                    "Too many screenshots ({}, max {})",
                    count, MAX_SCREENSHOTS_PER_DEVICE
                ),
                Some(&format!(
                    "Remove {} screenshot(s)",
                    count - MAX_SCREENSHOTS_PER_DEVICE
                )),
            );
        }
    }

    /// Validates age rating configuration.
    fn validate_age_rating(&self, rating: &AppleAgeRating, result: &mut ValidationResult) {
        // Check for potentially dangerous combinations
        if rating.gambling == AgeRatingLevel::FrequentOrIntense && !rating.gambling_and_contests {
            result.add_warning(
                "age_rating.gambling",
                "Gambling is marked as frequent/intense but gambling_and_contests is not enabled",
                Some("Enable gambling_and_contests if your app contains gambling features"),
            );
        }

        // Unrestricted web access should be noted
        if rating.unrestricted_web_access {
            result.add_info(
                "age_rating.unrestricted_web_access",
                "App has unrestricted web access enabled",
                Some("This may affect your age rating"),
            );
        }
    }
}

/// Checks if dimensions match any valid dimension set.
fn is_valid_dimension(dims: &Dimensions, valid: &[(u32, u32)]) -> bool {
    valid
        .iter()
        .any(|(w, h)| dims.width == *w && dims.height == *h)
}

/// Validates screenshots organized by locale.
pub fn validate_localized_screenshots(
    screenshots: &HashMap<Locale, AppleScreenshotSet>,
    result: &mut ValidationResult,
) {
    for (locale, screenshot_set) in screenshots {
        let validator = AppleValidator::new(false);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::apple::AppleLocalizedMetadata;
    use crate::types::common::AssetType;
    use std::path::PathBuf;

    fn create_valid_metadata() -> AppleMetadata {
        let mut metadata = AppleMetadata::new("com.example.app");
        metadata.primary_locale = Locale::default();
        metadata.support_url = Some("https://example.com/support".to_string());
        metadata.privacy_policy_url = Some("https://example.com/privacy".to_string());

        let localized = AppleLocalizedMetadata {
            name: "My App".to_string(),
            description: "A great app for testing".to_string(),
            subtitle: Some("The best app".to_string()),
            keywords: Some("test,app,great".to_string()),
            whats_new: Some("Bug fixes and improvements".to_string()),
            promotional_text: None,
            privacy_policy_url: None,
            support_url: None,
            marketing_url: None,
        };

        metadata
            .localizations
            .insert("en-US".to_string(), localized);
        metadata
    }

    #[test]
    fn test_valid_metadata() {
        let metadata = create_valid_metadata();
        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        // Should only have warnings/info, no errors
        assert!(result.is_valid(), "Errors: {:?}", result.errors());
    }

    #[test]
    fn test_missing_required_fields() {
        let mut metadata = AppleMetadata::new("");
        metadata.primary_locale = Locale::default();

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.field == "bundle_id"));
        assert!(result.errors().iter().any(|e| e.field == "support_url"));
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field == "privacy_policy_url"));
    }

    #[test]
    fn test_name_too_long() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.name = "A".repeat(35); // Exceeds 30 char limit
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("name") && e.message.contains("exceeds")));
    }

    #[test]
    fn test_invalid_url() {
        let mut metadata = create_valid_metadata();
        metadata.support_url = Some("not-a-url".to_string());

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field == "support_url" && e.message.contains("not a valid URL")));
    }

    #[test]
    fn test_screenshot_dimensions() {
        let mut metadata = create_valid_metadata();

        // Add valid screenshot
        let valid_screenshot = MediaAsset::new(PathBuf::from("screen1.png"), AssetType::Screenshot)
            .with_dimensions(1242, 2688);
        metadata.screenshots.iphone_6_5.push(valid_screenshot);

        // Add invalid screenshot
        let invalid_screenshot =
            MediaAsset::new(PathBuf::from("screen2.png"), AssetType::Screenshot)
                .with_dimensions(1000, 2000);
        metadata.screenshots.iphone_6_5.push(invalid_screenshot);

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("iphone_6_5") && e.message.contains("invalid dimensions")));
    }

    #[test]
    fn test_screenshot_count_limit() {
        let mut metadata = create_valid_metadata();

        // Add too many screenshots
        for i in 0..12 {
            let screenshot = MediaAsset::new(
                PathBuf::from(format!("screen{}.png", i)),
                AssetType::Screenshot,
            )
            .with_dimensions(1242, 2688);
            metadata.screenshots.iphone_6_5.push(screenshot);
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("iphone_6_5") && e.message.contains("Too many screenshots")));
    }

    #[test]
    fn test_whitespace_warnings() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.name = " My App ".to_string(); // Leading/trailing whitespace
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(result.is_valid()); // Whitespace is a warning, not an error
        assert!(result
            .warnings()
            .iter()
            .any(|w| w.field.contains("name") && w.message.contains("whitespace")));
    }

    #[test]
    fn test_strict_mode() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.name = " My App ".to_string(); // Whitespace warning
        }

        let validator = AppleValidator::new(true); // Strict mode
        let result = validator.validate(&metadata);

        // In strict mode, warnings become errors
        assert!(!result.is_valid());
    }

    #[test]
    fn test_update_requires_whats_new() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.whats_new = None; // Remove what's new
        }

        // Normal validation should pass
        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);
        assert!(result.is_valid());

        // Update validation should fail
        let validator = AppleValidator::for_update(false);
        let result = validator.validate(&metadata);
        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("whats_new")));
    }

    #[test]
    fn test_keywords_format() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.keywords = Some("a, b, c".to_string()); // Spaces after commas
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(result.is_valid());
        assert!(result
            .infos()
            .iter()
            .any(|i| i.field.contains("keywords") && i.message.contains("spaces after commas")));
    }

    #[test]
    fn test_newlines_in_single_line_fields() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            localized.name = "My\nApp".to_string(); // Newline in name
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result
            .errors()
            .iter()
            .any(|e| e.field.contains("name") && e.message.contains("newlines")));
    }

    #[test]
    fn test_unicode_character_counting() {
        let mut metadata = create_valid_metadata();
        if let Some(localized) = metadata.localizations.get_mut("en-US") {
            // 30 Unicode characters (emojis count as 1 char each)
            localized.name = "A".repeat(28) + "\u{1F600}\u{1F600}";
        }

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        // Should be valid (exactly 30 chars)
        assert!(result.is_valid(), "Errors: {:?}", result.errors());
    }

    #[test]
    fn test_primary_locale_missing() {
        let mut metadata = create_valid_metadata();
        metadata.primary_locale = Locale::new("de-DE").unwrap();
        // But we only have en-US in localizations

        let validator = AppleValidator::new(false);
        let result = validator.validate(&metadata);

        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.field == "primary_locale"));
    }
}
