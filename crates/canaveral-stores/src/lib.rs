//! App store and package registry upload adapters for Canaveral
//!
//! This crate provides integration with various app stores and package registries
//! for uploading and managing releases.
//!
//! ## Supported Stores
//!
//! ### App Stores
//! - **Apple**: App Store Connect, macOS notarization
//! - **Google Play**: Android app distribution
//! - **Microsoft**: Microsoft Store / Partner Center
//!
//! ### Package Registries
//! - **NPM**: JavaScript/TypeScript package registry
//! - **Crates.io**: Rust package registry
//!
//! ## Features
//!
//! - **metadata**: Optional integration with `canaveral-metadata` for validating
//!   and syncing app store metadata as part of the upload workflow.
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::{StoreAdapter, apple::AppStoreConnect};
//!
//! let store = AppStoreConnect::new(config)?;
//! store.upload(&artifact_path, &options).await?;
//! ```
//!
//! ## Metadata Integration
//!
//! With the `metadata` feature enabled, you can validate metadata before upload:
//!
//! ```ignore
//! use canaveral_stores::{UploadOptions, metadata_integration::*};
//! use std::path::PathBuf;
//!
//! let mut options = UploadOptions::default();
//! options.validate_metadata = true;
//! options.metadata_path = Some(PathBuf::from("./fastlane/metadata"));
//! options.require_valid_metadata = true;
//!
//! // Validation will be performed before upload
//! store.upload(&artifact_path, &options).await?;
//! ```

pub mod error;
pub mod metadata_integration;
pub mod registry;
pub mod traits;
pub mod types;

pub mod apple;
pub mod firebase;
pub mod google_play;
pub mod microsoft;
pub mod registries;

pub use error::StoreError;
pub use registry::StoreRegistry;
pub use traits::StoreAdapter;
pub use types::*;

// Re-export registry types
pub use registries::{CratesIoConfig, CratesIoRegistry, NpmConfig, NpmRegistry, TagSupport};

// Re-export metadata integration types when feature is enabled
#[cfg(feature = "metadata")]
pub use metadata_integration::{
    run_pre_upload_validation, should_sync_metadata, should_validate_metadata,
    validate_metadata_for_upload, MetadataPlatform, MetadataValidationSummary,
};
