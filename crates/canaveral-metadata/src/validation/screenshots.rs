//! Screenshot file validation utilities.
//!
//! This module provides functions to validate screenshot files for Apple App Store
//! and Google Play Store, including dimension validation against platform requirements.

use std::path::Path;

use tracing::debug;

use crate::types::common::{Dimensions, Platform};
use crate::validation::{Severity, ValidationIssue, ValidationResult};
use crate::{MetadataError, Result};

// =============================================================================
// Apple Screenshot Dimension Constants
// =============================================================================

/// Valid dimensions for iPhone 6.5" display (iPhone 14 Pro Max, 13 Pro Max, 12 Pro Max, 11 Pro Max, XS Max)
pub const APPLE_IPHONE_6_5_DIMS: &[(u32, u32)] = &[
    (1242, 2688),
    (2688, 1242), // Standard
    (1284, 2778),
    (2778, 1284), // iPhone 12/13/14 Pro Max
    (1290, 2796),
    (2796, 1290), // iPhone 14 Pro Max (newer)
];

/// Valid dimensions for iPhone 5.5" display (iPhone 8 Plus, 7 Plus, 6s Plus)
pub const APPLE_IPHONE_5_5_DIMS: &[(u32, u32)] = &[(1242, 2208), (2208, 1242)];

/// Valid dimensions for iPhone 6.7" display (iPhone 14 Plus, 14 Pro Max)
pub const APPLE_IPHONE_6_7_DIMS: &[(u32, u32)] = &[(1290, 2796), (2796, 1290)];

/// Valid dimensions for iPhone 6.1" display (iPhone 14, 14 Pro, 13, 13 Pro, 12, 12 Pro)
pub const APPLE_IPHONE_6_1_DIMS: &[(u32, u32)] = &[
    (1170, 2532),
    (2532, 1170), // iPhone 12/13/14
    (1179, 2556),
    (2556, 1179), // iPhone 14 Pro
];

/// Valid dimensions for iPad Pro 12.9" display
pub const APPLE_IPAD_PRO_12_9_DIMS: &[(u32, u32)] = &[(2048, 2732), (2732, 2048)];

/// Valid dimensions for iPad Pro 11" display
pub const APPLE_IPAD_PRO_11_DIMS: &[(u32, u32)] = &[(1668, 2388), (2388, 1668)];

/// Valid dimensions for iPad 10.5" display
pub const APPLE_IPAD_10_5_DIMS: &[(u32, u32)] = &[(1668, 2224), (2224, 1668)];

/// Valid dimensions for Apple Watch Series 9 (45mm)
pub const APPLE_WATCH_SERIES_9_DIMS: &[(u32, u32)] = &[(396, 484), (484, 396)];

/// Valid dimensions for Apple TV
pub const APPLE_TV_DIMS: &[(u32, u32)] = &[(1920, 1080), (3840, 2160)];

// =============================================================================
// Google Play Screenshot Dimension Constants
// =============================================================================

/// Required dimensions for Google Play feature graphic
pub const GOOGLE_PLAY_FEATURE_GRAPHIC_DIMS: (u32, u32) = (1024, 500);

/// Minimum dimension for Google Play screenshots
pub const GOOGLE_PLAY_SCREENSHOT_MIN: u32 = 320;

/// Maximum dimension for Google Play screenshots
pub const GOOGLE_PLAY_SCREENSHOT_MAX: u32 = 3840;

/// Google Play phone screenshot recommended dimensions
pub const GOOGLE_PLAY_PHONE_DIMS: &[(u32, u32)] = &[
    (1080, 1920),
    (1920, 1080), // 16:9
    (1080, 2160),
    (2160, 1080), // 18:9
    (1080, 2340),
    (2340, 1080), // 19.5:9
    (1080, 2400),
    (2400, 1080), // 20:9
];

/// Google Play 7" tablet screenshot recommended dimensions
pub const GOOGLE_PLAY_TABLET_7_DIMS: &[(u32, u32)] = &[(1200, 1920), (1920, 1200)];

/// Google Play 10" tablet screenshot recommended dimensions
pub const GOOGLE_PLAY_TABLET_10_DIMS: &[(u32, u32)] = &[(1600, 2560), (2560, 1600)];

/// Google Play TV screenshot dimensions
pub const GOOGLE_PLAY_TV_DIMS: &[(u32, u32)] = &[(1920, 1080), (3840, 2160)];

// =============================================================================
// Core Functions
// =============================================================================

/// Read dimensions from an image file.
///
/// Uses `image::image_dimensions()` which is fast and doesn't load the full image
/// into memory - it only reads the header to determine dimensions.
///
/// # Arguments
///
/// * `path` - Path to the image file
///
/// # Returns
///
/// Returns the image dimensions or an error if the file cannot be read.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use canaveral_metadata::validation::screenshots::read_image_dimensions;
///
/// let dims = read_image_dimensions(Path::new("screenshot.png")).unwrap();
/// println!("Image is {}x{}", dims.width, dims.height);
/// ```
pub fn read_image_dimensions(path: &Path) -> Result<Dimensions> {
    let (width, height) = image::image_dimensions(path).map_err(|e| {
        MetadataError::Io(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Failed to read image dimensions from {:?}: {}", path, e),
        ))
    })?;

    Ok(Dimensions {
        width: width as u32,
        height: height as u32,
    })
}

/// Get valid dimensions for an Apple device type.
///
/// # Arguments
///
/// * `device_type` - Device type identifier (e.g., "iphone_6_5", "ipad_pro_12_9")
///
/// # Returns
///
/// Returns a slice of valid dimension tuples for the device type, or `None` if unknown.
pub fn get_apple_valid_dimensions(device_type: &str) -> Option<&'static [(u32, u32)]> {
    match device_type {
        "iphone_6_5" => Some(APPLE_IPHONE_6_5_DIMS),
        "iphone_5_5" => Some(APPLE_IPHONE_5_5_DIMS),
        "iphone_6_7" => Some(APPLE_IPHONE_6_7_DIMS),
        "iphone_6_1" => Some(APPLE_IPHONE_6_1_DIMS),
        "ipad_pro_12_9" => Some(APPLE_IPAD_PRO_12_9_DIMS),
        "ipad_pro_11" => Some(APPLE_IPAD_PRO_11_DIMS),
        "ipad_10_5" => Some(APPLE_IPAD_10_5_DIMS),
        "watch_series_9" => Some(APPLE_WATCH_SERIES_9_DIMS),
        "apple_tv" => Some(APPLE_TV_DIMS),
        _ => None,
    }
}

/// Get valid dimensions for a Google Play device type.
///
/// # Arguments
///
/// * `device_type` - Device type identifier (e.g., "phone", "tablet_7", "tablet_10", "tv")
///
/// # Returns
///
/// Returns a slice of valid dimension tuples for the device type, or `None` if unknown.
pub fn get_google_play_valid_dimensions(device_type: &str) -> Option<&'static [(u32, u32)]> {
    match device_type {
        "phone" => Some(GOOGLE_PLAY_PHONE_DIMS),
        "tablet_7" => Some(GOOGLE_PLAY_TABLET_7_DIMS),
        "tablet_10" => Some(GOOGLE_PLAY_TABLET_10_DIMS),
        "tv" => Some(GOOGLE_PLAY_TV_DIMS),
        _ => None,
    }
}

// =============================================================================
// Apple Validation Functions
// =============================================================================

/// Validate a screenshot file exists and has valid dimensions for Apple App Store.
///
/// # Arguments
///
/// * `path` - Path to the screenshot file
/// * `device_type` - Device type (e.g., "iphone_6_5", "ipad_pro_12_9")
///
/// # Returns
///
/// Returns a validation result with any issues found.
///
/// # Example
///
/// ```no_run
/// use std::path::Path;
/// use canaveral_metadata::validation::screenshots::validate_apple_screenshot_file;
///
/// let result = validate_apple_screenshot_file(Path::new("screenshot.png"), "iphone_6_5");
/// if result.is_valid() {
///     println!("Screenshot is valid!");
/// }
/// ```
pub fn validate_apple_screenshot_file(path: &Path, device_type: &str) -> ValidationResult {
    debug!(path = %path.display(), device_type, "validating Apple screenshot file");
    let mut result = ValidationResult::new();
    let field = path.display().to_string();

    // Check if file exists
    if !path.exists() {
        result.add(ValidationIssue::error(
            &field,
            format!("Screenshot file does not exist: {:?}", path),
        ));
        return result;
    }

    // Check if it's a file (not a directory)
    if !path.is_file() {
        result.add(ValidationIssue::error(
            &field,
            format!("Path is not a file: {:?}", path),
        ));
        return result;
    }

    // Read dimensions
    let dimensions = match read_image_dimensions(path) {
        Ok(dims) => dims,
        Err(e) => {
            result.add(ValidationIssue::error(
                &field,
                format!("Failed to read image: {}", e),
            ));
            return result;
        }
    };

    // Get valid dimensions for device type
    let valid_dims = match get_apple_valid_dimensions(device_type) {
        Some(dims) => dims,
        None => {
            result.add(ValidationIssue::warning(
                &field,
                format!("Unknown Apple device type: {}. Cannot validate dimensions.", device_type),
            ));
            return result;
        }
    };

    // Check if dimensions match any valid option
    let dims_tuple = (dimensions.width, dimensions.height);
    if !valid_dims.contains(&dims_tuple) {
        let valid_dims_str: Vec<String> = valid_dims
            .iter()
            .map(|(w, h)| format!("{}x{}", w, h))
            .collect();
        result.add(ValidationIssue::with_suggestion(
            Severity::Error,
            &field,
            format!(
                "Invalid dimensions {}x{} for {} screenshots",
                dimensions.width, dimensions.height, device_type
            ),
            format!("Valid dimensions are: {}", valid_dims_str.join(", ")),
        ));
    }

    result
}

// =============================================================================
// Google Play Validation Functions
// =============================================================================

/// Validate a screenshot file exists and has valid dimensions for Google Play.
///
/// # Arguments
///
/// * `path` - Path to the screenshot file
/// * `device_type` - Device type (e.g., "phone", "tablet_7", "tablet_10", "tv")
///
/// # Returns
///
/// Returns a validation result with any issues found.
pub fn validate_google_play_screenshot_file(path: &Path, device_type: &str) -> ValidationResult {
    debug!(path = %path.display(), device_type, "validating Google Play screenshot file");
    let mut result = ValidationResult::new();
    let field = path.display().to_string();

    // Check if file exists
    if !path.exists() {
        result.add(ValidationIssue::error(
            &field,
            format!("Screenshot file does not exist: {:?}", path),
        ));
        return result;
    }

    // Check if it's a file (not a directory)
    if !path.is_file() {
        result.add(ValidationIssue::error(
            &field,
            format!("Path is not a file: {:?}", path),
        ));
        return result;
    }

    // Read dimensions
    let dimensions = match read_image_dimensions(path) {
        Ok(dims) => dims,
        Err(e) => {
            result.add(ValidationIssue::error(
                &field,
                format!("Failed to read image: {}", e),
            ));
            return result;
        }
    };

    // Check min/max bounds first (Google Play requirement)
    if dimensions.width < GOOGLE_PLAY_SCREENSHOT_MIN
        || dimensions.height < GOOGLE_PLAY_SCREENSHOT_MIN
    {
        result.add(ValidationIssue::error(
            &field,
            format!(
                "Screenshot dimensions {}x{} are below minimum {}px",
                dimensions.width, dimensions.height, GOOGLE_PLAY_SCREENSHOT_MIN
            ),
        ));
    }

    if dimensions.width > GOOGLE_PLAY_SCREENSHOT_MAX
        || dimensions.height > GOOGLE_PLAY_SCREENSHOT_MAX
    {
        result.add(ValidationIssue::error(
            &field,
            format!(
                "Screenshot dimensions {}x{} exceed maximum {}px",
                dimensions.width, dimensions.height, GOOGLE_PLAY_SCREENSHOT_MAX
            ),
        ));
    }

    // Get recommended dimensions for device type
    if let Some(valid_dims) = get_google_play_valid_dimensions(device_type) {
        let dims_tuple = (dimensions.width, dimensions.height);
        if !valid_dims.contains(&dims_tuple) {
            let valid_dims_str: Vec<String> = valid_dims
                .iter()
                .map(|(w, h)| format!("{}x{}", w, h))
                .collect();
            result.add(ValidationIssue::with_suggestion(
                Severity::Warning,
                &field,
                format!(
                    "Dimensions {}x{} are not in recommended sizes for {} screenshots",
                    dimensions.width, dimensions.height, device_type
                ),
                format!("Recommended dimensions are: {}", valid_dims_str.join(", ")),
            ));
        }
    } else {
        result.add(ValidationIssue::warning(
            &field,
            format!(
                "Unknown Google Play device type: {}. Cannot validate recommended dimensions.",
                device_type
            ),
        ));
    }

    result
}

/// Validate a feature graphic file for Google Play.
///
/// Feature graphics must be exactly 1024x500 pixels.
///
/// # Arguments
///
/// * `path` - Path to the feature graphic file
///
/// # Returns
///
/// Returns a validation result with any issues found.
pub fn validate_feature_graphic_file(path: &Path) -> ValidationResult {
    debug!(path = %path.display(), "validating feature graphic file");
    let mut result = ValidationResult::new();
    let field = path.display().to_string();

    // Check if file exists
    if !path.exists() {
        result.add(ValidationIssue::error(
            &field,
            format!("Feature graphic file does not exist: {:?}", path),
        ));
        return result;
    }

    // Check if it's a file
    if !path.is_file() {
        result.add(ValidationIssue::error(
            &field,
            format!("Path is not a file: {:?}", path),
        ));
        return result;
    }

    // Read dimensions
    let dimensions = match read_image_dimensions(path) {
        Ok(dims) => dims,
        Err(e) => {
            result.add(ValidationIssue::error(
                &field,
                format!("Failed to read image: {}", e),
            ));
            return result;
        }
    };

    // Feature graphic must be exactly 1024x500
    let (required_width, required_height) = GOOGLE_PLAY_FEATURE_GRAPHIC_DIMS;
    if dimensions.width != required_width || dimensions.height != required_height {
        result.add(ValidationIssue::with_suggestion(
            Severity::Error,
            &field,
            format!(
                "Feature graphic has invalid dimensions {}x{}",
                dimensions.width, dimensions.height
            ),
            format!(
                "Feature graphic must be exactly {}x{} pixels",
                required_width, required_height
            ),
        ));
    }

    result
}

// =============================================================================
// Batch Validation Functions
// =============================================================================

/// Batch validate all screenshots in a directory.
///
/// Scans the directory for image files and validates each one against the
/// platform and device type requirements.
///
/// # Arguments
///
/// * `dir` - Path to the directory containing screenshots
/// * `platform` - Target platform (Apple or GooglePlay)
/// * `device_type` - Device type for dimension validation
///
/// # Returns
///
/// Returns a validation result with all issues found across all files.
pub async fn validate_screenshot_directory(
    dir: &Path,
    platform: Platform,
    device_type: &str,
) -> ValidationResult {
    let mut result = ValidationResult::new();
    let field = dir.display().to_string();

    // Check if directory exists
    if !dir.exists() {
        result.add(ValidationIssue::error(
            &field,
            format!("Screenshot directory does not exist: {:?}", dir),
        ));
        return result;
    }

    // Check if it's a directory
    if !dir.is_dir() {
        result.add(ValidationIssue::error(
            &field,
            format!("Path is not a directory: {:?}", dir),
        ));
        return result;
    }

    // Read directory entries
    let entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(e) => {
            result.add(ValidationIssue::error(
                &field,
                format!("Failed to read directory: {}", e),
            ));
            return result;
        }
    };

    // Collect all entries
    let mut entries_vec = Vec::new();
    let mut read_dir = entries;
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        entries_vec.push(entry);
    }

    // Count valid images
    let mut image_count = 0;

    // Validate each file
    for entry in entries_vec {
        let path = entry.path();

        // Skip non-files
        if !path.is_file() {
            continue;
        }

        // Check if it's an image file by extension
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());

        match extension.as_deref() {
            Some("png") | Some("jpg") | Some("jpeg") => {
                image_count += 1;
                let file_result = match platform {
                    Platform::Apple => validate_apple_screenshot_file(&path, device_type),
                    Platform::GooglePlay => validate_google_play_screenshot_file(&path, device_type),
                    // Package registries don't use screenshot validation
                    Platform::Npm | Platform::Crates | Platform::PyPI => ValidationResult::new(),
                };
                result.merge(file_result);
            }
            _ => {
                // Skip non-image files silently
            }
        }
    }

    // Warn if no images found
    if image_count == 0 {
        result.add(ValidationIssue::warning(
            &field,
            "No image files (PNG, JPG) found in directory",
        ));
    }

    // Check screenshot count requirements
    match platform {
        Platform::Apple => {
            if image_count > 10 {
                result.add(ValidationIssue::error(
                    &field,
                    format!(
                        "Too many screenshots ({}). Apple allows maximum 10 per device type.",
                        image_count
                    ),
                ));
            }
        }
        Platform::GooglePlay => {
            if image_count > 8 {
                result.add(ValidationIssue::error(
                    &field,
                    format!(
                        "Too many screenshots ({}). Google Play allows maximum 8 per device type.",
                        image_count
                    ),
                ));
            }
        }
        // Package registries don't have screenshot count requirements
        Platform::Npm | Platform::Crates | Platform::PyPI => {}
    }

    result
}

/// List of supported Apple device types for screenshots.
pub const APPLE_DEVICE_TYPES: &[&str] = &[
    "iphone_6_5",
    "iphone_5_5",
    "iphone_6_7",
    "iphone_6_1",
    "ipad_pro_12_9",
    "ipad_pro_11",
    "ipad_10_5",
    "watch_series_9",
    "apple_tv",
];

/// List of supported Google Play device types for screenshots.
pub const GOOGLE_PLAY_DEVICE_TYPES: &[&str] = &["phone", "tablet_7", "tablet_10", "tv"];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_dimensions_constants() {
        // Verify Apple dimensions include both orientations
        assert!(APPLE_IPHONE_6_5_DIMS.contains(&(1242, 2688)));
        assert!(APPLE_IPHONE_6_5_DIMS.contains(&(2688, 1242)));

        // Verify Google Play feature graphic dimensions
        assert_eq!(GOOGLE_PLAY_FEATURE_GRAPHIC_DIMS, (1024, 500));

        // Verify min/max bounds
        assert_eq!(GOOGLE_PLAY_SCREENSHOT_MIN, 320);
        assert_eq!(GOOGLE_PLAY_SCREENSHOT_MAX, 3840);
    }

    #[test]
    fn test_get_apple_valid_dimensions() {
        assert!(get_apple_valid_dimensions("iphone_6_5").is_some());
        assert!(get_apple_valid_dimensions("ipad_pro_12_9").is_some());
        assert!(get_apple_valid_dimensions("unknown").is_none());
    }

    #[test]
    fn test_get_google_play_valid_dimensions() {
        assert!(get_google_play_valid_dimensions("phone").is_some());
        assert!(get_google_play_valid_dimensions("tablet_7").is_some());
        assert!(get_google_play_valid_dimensions("unknown").is_none());
    }

    #[test]
    fn test_validate_nonexistent_file() {
        let result = validate_apple_screenshot_file(Path::new("/nonexistent/file.png"), "iphone_6_5");
        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.message.contains("does not exist")));
    }

    #[test]
    fn test_validate_directory_as_file() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_apple_screenshot_file(temp_dir.path(), "iphone_6_5");
        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.message.contains("not a file")));
    }

    #[test]
    fn test_validate_google_play_nonexistent() {
        let result = validate_google_play_screenshot_file(Path::new("/nonexistent/file.png"), "phone");
        assert!(!result.is_valid());
    }

    #[test]
    fn test_validate_feature_graphic_nonexistent() {
        let result = validate_feature_graphic_file(Path::new("/nonexistent/feature.png"));
        assert!(!result.is_valid());
    }

    #[test]
    fn test_device_type_lists() {
        assert!(APPLE_DEVICE_TYPES.contains(&"iphone_6_5"));
        assert!(APPLE_DEVICE_TYPES.contains(&"ipad_pro_12_9"));
        assert!(GOOGLE_PLAY_DEVICE_TYPES.contains(&"phone"));
        assert!(GOOGLE_PLAY_DEVICE_TYPES.contains(&"tv"));
    }

    #[tokio::test]
    async fn test_validate_nonexistent_directory() {
        let result = validate_screenshot_directory(
            Path::new("/nonexistent/directory"),
            Platform::Apple,
            "iphone_6_5",
        )
        .await;
        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.message.contains("does not exist")));
    }

    #[tokio::test]
    async fn test_validate_empty_directory() {
        let temp_dir = TempDir::new().unwrap();
        let result = validate_screenshot_directory(temp_dir.path(), Platform::Apple, "iphone_6_5").await;
        // Should be valid but with a warning about no images
        assert!(result.is_valid());
        assert!(result.warnings().iter().any(|w| w.message.contains("No image files")));
    }

    #[tokio::test]
    async fn test_validate_file_as_directory() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "test").unwrap();

        let result = validate_screenshot_directory(&file_path, Platform::Apple, "iphone_6_5").await;
        assert!(!result.is_valid());
        assert!(result.errors().iter().any(|e| e.message.contains("not a directory")));
    }
}
