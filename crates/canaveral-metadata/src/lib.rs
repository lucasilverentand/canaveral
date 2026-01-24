//! App store metadata management for Canaveral.
//!
//! This crate provides types and utilities for managing app store metadata
//! for Apple App Store and Google Play Store.

pub mod error;
pub mod types;

pub use error::MetadataError;
pub use types::apple::{
    AppleAgeRating, AppleCategory, AppleLocalizedMetadata, AppleMetadata, AppleScreenshotSet,
};
pub use types::common::{AssetType, Locale, MediaAsset, Platform};
pub use types::google_play::{
    GooglePlayCategory, GooglePlayContentRating, GooglePlayLocalizedMetadata, GooglePlayMetadata,
    GooglePlayScreenshotSet,
};

/// Result type alias for metadata operations.
pub type Result<T> = std::result::Result<T, MetadataError>;
