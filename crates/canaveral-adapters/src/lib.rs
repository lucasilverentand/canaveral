//! Canaveral Adapters - Package adapters for release management
//!
//! This crate provides package manager adapters for npm, Cargo, Python, Go, Maven, Docker, etc.

pub mod cargo;
pub mod credentials;
pub mod detector;
pub mod docker;
pub mod go;
pub mod maven;
pub mod npm;
pub mod publish;
pub mod python;
pub mod registry;
mod traits;

pub use credentials::{Credential, CredentialProvider};
pub use detector::detect_packages;
pub use docker::DockerAdapter;
pub use go::GoAdapter;
pub use maven::MavenAdapter;
pub use publish::{PublishAccess, PublishOptions, ValidationResult};
pub use registry::AdapterRegistry;
pub use traits::PackageAdapter;
