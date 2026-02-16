//! Canaveral Changelog - Changelog generation for release management
//!
//! This crate provides commit parsing and changelog generation capabilities.

pub mod formatter;
pub mod generator;
pub mod parser;
pub mod release_notes;
pub mod types;

pub use generator::ChangelogGenerator;
pub use formatter::{ChangelogFormatter, MarkdownFormatter, FormatterRegistry};
pub use parser::{CommitParser, ConventionalParser, ParserRegistry};
pub use release_notes::{ReleaseNotesGenerator, ReleaseNotes};
pub use types::ParsedCommit;
pub use types::{ChangelogEntry, Section};
