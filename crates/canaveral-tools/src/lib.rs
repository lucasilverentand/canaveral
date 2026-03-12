//! canaveral-tools — tool version detection, installation, and activation
//!
//! This crate manages development tool versions (bun, node, python, etc.)
//! similar to mise/asdf, but integrated with the canaveral workflow.

pub mod cache;
pub mod error;
pub mod providers;
pub mod registry;
pub mod tool_defs;
pub mod traits;
pub mod version_match;

pub use cache::parse_size;
pub use cache::{CacheStatus, CachedVersion, PruneResult, ToolCache};
pub use error::ToolError;
pub use registry::ToolRegistry;
pub use traits::{InstallResult, ToolInfo, ToolProvider};
