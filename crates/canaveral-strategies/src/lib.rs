//! Canaveral Strategies - Version strategies for release management
//!
//! This crate provides version calculation strategies like SemVer and CalVer.

mod semver;
mod traits;
pub mod types;

pub use semver::SemVerStrategy;
pub use traits::VersionStrategy;
pub use types::{BumpType, VersionComponents};
