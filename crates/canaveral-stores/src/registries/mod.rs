//! Package registry adapters
//!
//! Provides integration with various package registries for publishing
//! and managing releases.

pub mod crates_io;
pub mod npm;

pub use crates_io::{CratesIoConfig, CratesIoRegistry};
pub use npm::{NpmConfig, NpmRegistry, TagSupport};
