//! Optional metadata validation and sync integration.
//!
//! This module provides integration with `canaveral-metadata` for validating
//! and syncing app store metadata as part of the upload workflow.
//!
//! ## Feature Flag
//!
//! This module requires the `metadata` feature to be enabled:
//!
//! ```toml
//! [dependencies]
//! canaveral-stores = { version = "0.1", features = ["metadata"] }
//! ```
//!
//! ## Usage
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
//! // Check if validation should run
//! if should_validate_metadata(&options) {
//!     let result = validate_metadata_for_upload(
//!         MetadataPlatform::Apple,
//!         "com.example.app",
//!         options.metadata_path.as_ref().unwrap(),
//!         true, // strict mode
//!     ).await?;
//!
//!     print_validation_summary(&result);
//!
//!     if !result.is_valid() && options.require_valid_metadata {
//!         return Err(StoreError::ValidationFailed("Metadata validation failed".into()));
//!     }
//! }
//! ```

#[cfg(feature = "metadata")]
pub use integration::*;

#[cfg(feature = "metadata")]
mod integration {
    use std::path::Path;
    use crate::{error::Result, StoreError, UploadOptions};
    use canaveral_metadata::{
        AppleValidator, FastlaneStorage, GooglePlayValidator, MetadataStorage,
        ValidationResult as MetadataValidationResult,
    };
    use tracing::{debug, info, instrument, warn};

    /// Platform identifier for metadata validation.
    ///
    /// This is separate from `StoreType` to specifically represent
    /// platforms that support metadata management.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum MetadataPlatform {
        /// Apple App Store / Mac App Store
        Apple,
        /// Google Play Store
        GooglePlay,
    }

    impl std::fmt::Display for MetadataPlatform {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                MetadataPlatform::Apple => write!(f, "Apple"),
                MetadataPlatform::GooglePlay => write!(f, "Google Play"),
            }
        }
    }

    /// Validate metadata before upload.
    ///
    /// Loads metadata from the specified path and validates it against
    /// the platform's requirements.
    ///
    /// # Arguments
    ///
    /// * `platform` - Target platform (Apple or Google Play)
    /// * `app_id` - App identifier (bundle ID or package name)
    /// * `metadata_path` - Path to the metadata directory
    /// * `strict` - If true, treats warnings as errors
    ///
    /// # Returns
    ///
    /// Returns the validation result containing any issues found.
    ///
    /// # Errors
    ///
    /// Returns an error if metadata cannot be loaded.
    #[instrument(skip_all, fields(platform = %platform, app_id = app_id, metadata_path = %metadata_path.display()))]
    pub async fn validate_metadata_for_upload(
        platform: MetadataPlatform,
        app_id: &str,
        metadata_path: &Path,
        strict: bool,
    ) -> Result<MetadataValidationResult> {
        debug!(
            "Validating {} metadata for {} at {:?}",
            platform, app_id, metadata_path
        );

        let storage = FastlaneStorage::new(metadata_path.to_path_buf());

        let result = match platform {
            MetadataPlatform::Apple => {
                let metadata = storage.load_apple(app_id).await.map_err(|e| {
                    StoreError::ConfigurationError(format!("Failed to load Apple metadata: {}", e))
                })?;
                let validator = AppleValidator::new(strict);
                validator.validate(&metadata)
            }
            MetadataPlatform::GooglePlay => {
                let metadata = storage.load_google_play(app_id).await.map_err(|e| {
                    StoreError::ConfigurationError(format!(
                        "Failed to load Google Play metadata: {}",
                        e
                    ))
                })?;
                let validator = GooglePlayValidator::new(strict);
                validator.validate(&metadata)
            }
        };

        info!(
            "Metadata validation complete: {} errors, {} warnings",
            result.error_count(),
            result.warning_count()
        );

        Ok(result)
    }

    /// Check if metadata should be validated based on upload options.
    ///
    /// Returns `true` if:
    /// - `validate_metadata` is enabled, AND
    /// - `metadata_path` is specified
    pub fn should_validate_metadata(options: &UploadOptions) -> bool {
        options.validate_metadata && options.metadata_path.is_some()
    }

    /// Check if metadata should be synced after upload based on options.
    ///
    /// Returns `true` if:
    /// - `sync_metadata` is enabled, AND
    /// - `metadata_path` is specified
    pub fn should_sync_metadata(options: &UploadOptions) -> bool {
        options.sync_metadata && options.metadata_path.is_some()
    }

    /// Print validation results to console.
    ///
    /// Outputs a formatted summary of validation results including
    /// errors, warnings, and informational messages.
    pub fn print_validation_summary(result: &MetadataValidationResult) {
        if result.is_clean() {
            info!("Metadata validation passed with no issues");
            return;
        }

        // Print errors
        for issue in result.errors() {
            eprintln!(
                "  [ERROR] {}: {}",
                issue.field, issue.message
            );
            if let Some(ref suggestion) = issue.suggestion {
                eprintln!("          Suggestion: {}", suggestion);
            }
        }

        // Print warnings
        for issue in result.warnings() {
            warn!(
                "[WARNING] {}: {}",
                issue.field, issue.message
            );
            if let Some(ref suggestion) = issue.suggestion {
                eprintln!("          Suggestion: {}", suggestion);
            }
        }

        // Print info messages (only in verbose mode or if there are no other issues)
        if result.error_count() == 0 && result.warning_count() == 0 {
            for issue in result.infos() {
                info!("[INFO] {}: {}", issue.field, issue.message);
            }
        }

        // Summary
        eprintln!();
        eprintln!(
            "  Validation summary: {} error(s), {} warning(s)",
            result.error_count(),
            result.warning_count()
        );
    }

    /// Metadata validation summary for inclusion in upload results.
    #[derive(Debug, Clone)]
    pub struct MetadataValidationSummary {
        /// Whether validation passed (no errors)
        pub valid: bool,
        /// Number of errors found
        pub error_count: usize,
        /// Number of warnings found
        pub warning_count: usize,
        /// First few error messages (for display)
        pub error_messages: Vec<String>,
        /// First few warning messages (for display)
        pub warning_messages: Vec<String>,
    }

    impl MetadataValidationSummary {
        /// Create a summary from a validation result.
        pub fn from_result(result: &MetadataValidationResult) -> Self {
            Self {
                valid: result.is_valid(),
                error_count: result.error_count(),
                warning_count: result.warning_count(),
                error_messages: result
                    .errors()
                    .iter()
                    .take(5)
                    .map(|i| format!("{}: {}", i.field, i.message))
                    .collect(),
                warning_messages: result
                    .warnings()
                    .iter()
                    .take(5)
                    .map(|i| format!("{}: {}", i.field, i.message))
                    .collect(),
            }
        }

        /// Create a skipped summary (validation was not run).
        pub fn skipped() -> Self {
            Self {
                valid: true,
                error_count: 0,
                warning_count: 0,
                error_messages: Vec::new(),
                warning_messages: Vec::new(),
            }
        }
    }

    /// Run metadata validation as part of the upload workflow.
    ///
    /// This is a convenience function that:
    /// 1. Checks if validation should run
    /// 2. Validates metadata if enabled
    /// 3. Prints results
    /// 4. Returns an error if validation fails and `require_valid_metadata` is set
    ///
    /// # Arguments
    ///
    /// * `platform` - Target platform
    /// * `app_id` - App identifier
    /// * `options` - Upload options containing validation settings
    /// * `strict` - Use strict validation mode
    ///
    /// # Returns
    ///
    /// Returns a validation summary, or an error if validation fails and is required.
    #[instrument(skip(options), fields(platform = %platform, app_id = app_id))]
    pub async fn run_pre_upload_validation(
        platform: MetadataPlatform,
        app_id: &str,
        options: &UploadOptions,
        strict: bool,
    ) -> Result<MetadataValidationSummary> {
        if !should_validate_metadata(options) {
            debug!("Metadata validation skipped (not enabled or no path specified)");
            return Ok(MetadataValidationSummary::skipped());
        }

        let metadata_path = options.metadata_path.as_ref().unwrap();

        info!(
            "Validating {} metadata before upload...",
            platform
        );

        let result = validate_metadata_for_upload(platform, app_id, metadata_path, strict).await?;

        if options.verbose {
            print_validation_summary(&result);
        }

        let summary = MetadataValidationSummary::from_result(&result);

        if !result.is_valid() && options.require_valid_metadata {
            let error_msg = format!(
                "Metadata validation failed with {} error(s). First error: {}",
                result.error_count(),
                summary
                    .error_messages
                    .first()
                    .map(|s| s.as_str())
                    .unwrap_or("unknown")
            );
            return Err(StoreError::ValidationFailed(error_msg));
        }

        if !result.is_valid() {
            warn!(
                "Metadata validation found {} error(s), but continuing upload (require_valid_metadata=false)",
                result.error_count()
            );
        }

        Ok(summary)
    }
}

// Stub implementations when feature is disabled
#[cfg(not(feature = "metadata"))]
pub fn should_validate_metadata(_options: &crate::UploadOptions) -> bool {
    false
}

#[cfg(not(feature = "metadata"))]
pub fn should_sync_metadata(_options: &crate::UploadOptions) -> bool {
    false
}
