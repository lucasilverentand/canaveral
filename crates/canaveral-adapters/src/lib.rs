//! Canaveral Adapters - Package adapters for release management
//!
//! This crate provides package manager adapters for npm, Cargo, Python, etc.

pub mod cargo;
pub mod credentials;
pub mod detector;
pub mod npm;
pub mod publish;
pub mod python;
pub mod registry;
mod traits;

pub use credentials::{Credential, CredentialProvider};
pub use detector::detect_packages;
pub use publish::{PublishAccess, PublishOptions, ValidationResult};
pub use registry::AdapterRegistry;
pub use traits::PackageAdapter;
