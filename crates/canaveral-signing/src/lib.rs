//! Canaveral Signing - Code signing support for release management
//!
//! This crate provides code signing functionality for various platforms:
//! - macOS: codesign, productsign, notarization
//! - Windows: signtool (Authenticode)
//! - Android: apksigner, jarsigner
//! - GPG: General-purpose signing
//!
//! It also includes a team vault for securely sharing signing credentials.

pub mod error;
pub mod identity;
pub mod provider;
pub mod providers;
pub mod team;

pub use error::{Result, SigningError};
pub use identity::{SigningIdentity, SigningIdentityType};
pub use provider::{
    SignOptions, SignatureInfo, SignatureStatus, SigningProvider, VerifyOptions,
};

// Re-export providers
pub use providers::gpg::GpgProvider;

#[cfg(target_os = "macos")]
pub use providers::macos::MacOSProvider;

#[cfg(target_os = "windows")]
pub use providers::windows::WindowsProvider;

pub use providers::android::AndroidProvider;
