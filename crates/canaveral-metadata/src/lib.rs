//! App store metadata management for Canaveral.
//!
//! This crate provides types and utilities for managing app store metadata
//! for Apple App Store and Google Play Store.
//!
//! ## Storage Backends
//!
//! The crate supports different storage backends for persisting metadata:
//!
//! - [`FastlaneStorage`]: A Fastlane-compatible directory structure with
//!   individual text files for each metadata field.
//!
//! ## Validation
//!
//! The crate provides comprehensive validation for app store metadata:
//!
//! - [`AppleValidator`]: Validates Apple App Store metadata against Apple's requirements
//!
//! ## Sync (requires `sync` feature)
//!
//! With the `sync` feature enabled, you can synchronize metadata with app stores:
//!
//! - [`sync::AppleMetadataSync`]: Sync metadata with App Store Connect
//!
//! ## Example
//!
//! ```no_run
//! use canaveral_metadata::{FastlaneStorage, MetadataStorage, Platform, Locale};
//!
//! # async fn example() -> canaveral_metadata::Result<()> {
//! // Create a storage backend
//! let storage = FastlaneStorage::new("metadata");
//!
//! // Initialize metadata for a new app
//! let locales = vec![Locale::new("en-US")?, Locale::new("de-DE")?];
//! storage.init(Platform::Apple, "com.example.app", &locales).await?;
//!
//! // Load existing metadata
//! let metadata = storage.load_apple("com.example.app").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Validation Example
//!
//! ```rust
//! use canaveral_metadata::{AppleMetadata, AppleValidator};
//!
//! let metadata = AppleMetadata::new("com.example.app");
//! let validator = AppleValidator::new(false);
//! let result = validator.validate(&metadata);
//!
//! if result.is_valid() {
//!     println!("Metadata is valid for submission!");
//! } else {
//!     for error in result.errors() {
//!         eprintln!("Error: {}", error);
//!     }
//! }
//! ```
//!
//! ## Sync Example (requires `sync` feature)
//!
//! ```no_run,ignore
//! use canaveral_metadata::sync::{AppleMetadataSync, AppleSyncConfig, MetadataSync};
//! use std::path::PathBuf;
//!
//! # async fn example() -> canaveral_metadata::Result<()> {
//! // Configure App Store Connect API credentials
//! let config = AppleSyncConfig::from_env()?;
//!
//! // Create sync client
//! let sync = AppleMetadataSync::new(config, PathBuf::from("metadata")).await?;
//!
//! // Pull metadata from App Store Connect
//! sync.pull("com.example.app", None).await?;
//!
//! // Check for differences
//! let diff = sync.diff("com.example.app").await?;
//! println!("{}", diff);
//!
//! // Push local changes (dry run first)
//! let result = sync.push("com.example.app", None, true).await?;
//! println!("Would update: {}", result);
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod storage;
#[cfg(feature = "sync")]
pub mod sync;
pub mod types;
pub mod validation;

pub use error::MetadataError;
pub use storage::{FastlaneStorage, MetadataStorage, StorageFormat};
pub use types::apple::{
    AppleAgeRating, AppleCategory, AppleLocalizedMetadata, AppleMetadata, AppleScreenshotSet,
};
pub use types::common::{AssetType, Locale, MediaAsset, Platform};
pub use types::google_play::{
    GooglePlayCategory, GooglePlayContentRating, GooglePlayLocalizedMetadata, GooglePlayMetadata,
    GooglePlayScreenshotSet,
};
pub use validation::{
    validate_localized_google_play_screenshots, validate_localized_screenshots, AppleValidator,
    GooglePlayValidator, Severity, ValidationIssue, ValidationResult,
    // Screenshot file validation
    read_image_dimensions, validate_apple_screenshot_file, validate_feature_graphic_file,
    validate_google_play_screenshot_file, validate_screenshot_directory,
    // Apple dimension constants
    APPLE_DEVICE_TYPES, APPLE_IPAD_10_5_DIMS, APPLE_IPAD_PRO_11_DIMS, APPLE_IPAD_PRO_12_9_DIMS,
    APPLE_IPHONE_5_5_DIMS, APPLE_IPHONE_6_1_DIMS, APPLE_IPHONE_6_5_DIMS, APPLE_IPHONE_6_7_DIMS,
    APPLE_TV_DIMS, APPLE_WATCH_SERIES_9_DIMS,
    // Google Play dimension constants
    GOOGLE_PLAY_DEVICE_TYPES, GOOGLE_PLAY_FEATURE_GRAPHIC_DIMS, GOOGLE_PLAY_PHONE_DIMS,
    GOOGLE_PLAY_SCREENSHOT_MAX, GOOGLE_PLAY_SCREENSHOT_MIN, GOOGLE_PLAY_TABLET_10_DIMS,
    GOOGLE_PLAY_TABLET_7_DIMS, GOOGLE_PLAY_TV_DIMS,
    // Helper functions
    get_apple_valid_dimensions, get_google_play_valid_dimensions,
};

/// Result type alias for metadata operations.
pub type Result<T> = std::result::Result<T, MetadataError>;
