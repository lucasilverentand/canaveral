//! Exit codes for the CLI

#![allow(dead_code)]

/// Success
pub const SUCCESS: i32 = 0;

/// General error
pub const ERROR: i32 = 1;

/// Configuration error
pub const CONFIG_ERROR: i32 = 2;

/// Git error
pub const GIT_ERROR: i32 = 3;

/// Version error
pub const VERSION_ERROR: i32 = 4;

/// Validation error
pub const VALIDATION_ERROR: i32 = 5;

/// User cancelled
pub const CANCELLED: i32 = 130;
