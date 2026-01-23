//! Commit parsing

mod conventional;
mod types;

pub use conventional::ConventionalParser;
pub use types::*;

use crate::types::ParsedCommit;
use canaveral_git::CommitInfo;

/// Trait for commit parsers
pub trait CommitParser: Send + Sync {
    /// Parse a commit into a structured format
    fn parse(&self, commit: &CommitInfo) -> Option<ParsedCommit>;

    /// Check if a commit should be included in the changelog
    fn should_include(&self, commit: &ParsedCommit) -> bool;
}
