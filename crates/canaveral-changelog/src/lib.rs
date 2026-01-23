//! Canaveral Changelog - Changelog generation for release management
//!
//! This crate provides commit parsing and changelog generation capabilities.

pub mod formatter;
pub mod generator;
pub mod parser;
pub mod types;

pub use generator::ChangelogGenerator;
pub use parser::{CommitParser, ConventionalParser};
pub use types::ParsedCommit;
pub use types::{ChangelogEntry, Section};
