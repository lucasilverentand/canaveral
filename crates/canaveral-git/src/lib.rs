//! Canaveral Git - Git operations for release management
//!
//! This crate provides git repository operations including commit history,
//! tag management, and remote operations.

mod commits;
pub mod hooks;
mod remote;
mod repository;
mod status;
mod tags;
pub mod types;

pub use remote::{git_push, git_push_tag, git_push_with_tags};
pub use repository::{GitRepo, Result};
pub use types::{CommitInfo, TagInfo};
