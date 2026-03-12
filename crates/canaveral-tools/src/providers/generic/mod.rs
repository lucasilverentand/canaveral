//! Generic tool provider backed by embedded TOML definitions
//!
//! Replaces the aqua registry-backed provider with a simpler, self-contained
//! approach: tool definitions are compiled into the binary as TOML, and the
//! generic provider handles download, extraction, and version detection for
//! any tool described by a `ToolDefinition`.

pub mod platform;
pub mod provider;
pub mod template;

pub use provider::GenericProvider;
