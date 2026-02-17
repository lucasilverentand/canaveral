//! Cargo.toml handling

use std::path::Path;

use canaveral_core::error::{AdapterError, Result};
use serde::Deserialize;
use toml_edit::{value, DocumentMut};

/// Cargo.toml structure (for reading)
#[derive(Debug, Clone, Deserialize)]
pub struct CargoToml {
    /// Package section
    pub package: Option<Package>,
    /// Workspace section
    pub workspace: Option<Workspace>,
}

/// Package section
#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    /// Package name
    pub name: String,
    /// Package version
    pub version: String,
    /// Package description
    pub description: Option<String>,
    /// Authors
    pub authors: Option<Vec<String>>,
    /// License
    pub license: Option<String>,
    /// License file
    #[serde(rename = "license-file")]
    pub license_file: Option<String>,
    /// Repository
    pub repository: Option<String>,
    /// Whether to publish
    pub publish: Option<bool>,
    /// Edition
    pub edition: Option<String>,
    /// Rust version requirement
    #[serde(rename = "rust-version")]
    pub rust_version: Option<String>,
    /// Default run target
    #[serde(rename = "default-run")]
    pub default_run: Option<String>,
}

impl Package {
    /// Check if this is a binary crate (has default-run or is likely a binary)
    pub fn is_binary(&self) -> bool {
        self.default_run.is_some()
    }
}

/// Workspace section
#[derive(Debug, Clone, Deserialize)]
pub struct Workspace {
    /// Workspace members
    pub members: Option<Vec<String>>,
}

impl CargoToml {
    /// Load Cargo.toml from path
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        toml::from_str(&content).map_err(|e| AdapterError::ManifestParseError(e.to_string()).into())
    }

    /// Update version in Cargo.toml (preserves formatting using toml_edit)
    pub fn update_version(path: &Path, version: &str) -> Result<()> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        let mut doc: DocumentMut = content
            .parse()
            .map_err(|e: toml_edit::TomlError| AdapterError::ManifestParseError(e.to_string()))?;

        // Update the version field
        if let Some(package) = doc.get_mut("package") {
            if let Some(table) = package.as_table_mut() {
                table["version"] = value(version);
            }
        } else {
            return Err(
                AdapterError::ManifestParseError("No [package] section found".to_string()).into(),
            );
        }

        std::fs::write(path, doc.to_string())
            .map_err(|e| AdapterError::ManifestUpdateError(e.to_string()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("Cargo.toml");

        std::fs::write(
            &path,
            r#"
[package]
name = "test-crate"
version = "1.0.0"
description = "A test crate"
"#,
        )
        .unwrap();

        let cargo = CargoToml::load(&path).unwrap();
        let package = cargo.package.unwrap();

        assert_eq!(package.name, "test-crate");
        assert_eq!(package.version, "1.0.0");
        assert_eq!(package.description, Some("A test crate".to_string()));
    }

    #[test]
    fn test_update_version_preserves_formatting() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("Cargo.toml");

        let original = r#"[package]
name = "test-crate"
version = "1.0.0"
edition = "2021"

# This is a comment

[dependencies]
serde = "1.0"
"#;

        std::fs::write(&path, original).unwrap();

        CargoToml::update_version(&path, "2.0.0").unwrap();

        let updated = std::fs::read_to_string(&path).unwrap();

        // Check version was updated
        assert!(updated.contains("version = \"2.0.0\""));

        // Check formatting is preserved
        assert!(updated.contains("# This is a comment"));
        assert!(updated.contains("[dependencies]"));
    }

    #[test]
    fn test_workspace_detection() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("Cargo.toml");

        std::fs::write(
            &path,
            r#"
[workspace]
members = ["crates/*"]
"#,
        )
        .unwrap();

        let cargo = CargoToml::load(&path).unwrap();
        assert!(cargo.package.is_none());
        assert!(cargo.workspace.is_some());
    }
}
