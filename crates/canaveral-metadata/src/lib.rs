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

pub mod error;
pub mod storage;
pub mod types;

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

/// Result type alias for metadata operations.
pub type Result<T> = std::result::Result<T, MetadataError>;
