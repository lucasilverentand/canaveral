//! Canaveral Core - Core library for release management
//!
//! This crate provides the foundational types, error handling, configuration,
//! and workflow orchestration for the Canaveral release management tool.

pub mod config;
pub mod error;
pub mod types;
pub mod workflow;

pub use error::{CanaveralError, Result};
pub use types::{ReleaseResult, ReleaseType};
