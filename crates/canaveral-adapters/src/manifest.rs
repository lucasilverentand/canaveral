//! Manifest file trait for package adapters
//!
//! Provides a common interface for reading and writing version information
//! from different package manifest formats (package.json, Cargo.toml, etc.).

use std::path::Path;

use anyhow::Result;

/// Trait for reading/writing version info from package manifest files.
///
/// Each package ecosystem stores metadata in a different manifest format.
/// This trait captures the common operations: load, read version/name, update
/// version, and save back to disk.
pub trait ManifestFile: Sized {
    /// The filename this manifest type uses (e.g., "package.json", "Cargo.toml")
    fn filename() -> &'static str;

    /// Load and parse the manifest from the given directory
    fn load(dir: &Path) -> Result<Self>;

    /// Save the manifest back to disk in the given directory
    fn save(&self, dir: &Path) -> Result<()>;

    /// Get the current version string, if present in the manifest
    fn version(&self) -> Option<&str>;

    /// Set a new version string
    fn set_version(&mut self, version: &str) -> Result<()>;

    /// Get the package name, if present in the manifest
    fn name(&self) -> Option<&str>;
}
