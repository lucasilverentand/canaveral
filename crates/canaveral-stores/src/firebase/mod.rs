//! Firebase App Distribution integration
//!
//! Provides upload and distribution capabilities for Firebase App Distribution.
//! Supports both Android (APK/AAB) and iOS (IPA) apps.
//!
//! ## Authentication
//!
//! Firebase App Distribution supports multiple authentication methods:
//! - Service account JSON file
//! - Firebase CLI token
//! - Application Default Credentials
//!
//! ## Usage
//!
//! ```ignore
//! use canaveral_stores::firebase::Firebase;
//!
//! let firebase = Firebase::from_env()?;
//! firebase.upload(&artifact_path, &options).await?;
//! ```

mod distribution;

pub use distribution::{
    Firebase, FirebaseConfig, FirebaseRelease, FirebaseUploadOptions,
    TesterGroup, DistributionStatus, ReleaseInfo,
};
