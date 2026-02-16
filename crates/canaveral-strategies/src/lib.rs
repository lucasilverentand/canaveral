//! Canaveral Strategies - Version strategies for release management
//!
//! This crate provides version calculation strategies like SemVer, CalVer, and Build Numbers.

mod buildnum;
mod calver;
mod registry;
mod semver;
mod traits;
pub mod types;

pub use buildnum::{BuildNumberFormat, BuildNumberStrategy};
pub use calver::{CalVerFormat, CalVerStrategy};
pub use registry::StrategyRegistry;
pub use semver::SemVerStrategy;
pub use traits::VersionStrategy;
pub use types::{BumpType, VersionComponents};
