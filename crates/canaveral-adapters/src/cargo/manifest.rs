//! Cargo.toml handling

use std::path::Path;

use canaveral_core::error::{AdapterError, Result};
use serde::Deserialize;
use toml_edit::{value, DocumentMut};

use crate::manifest::ManifestFile;

/// Cargo.toml structure (for reading)
#[derive(Debug, Clone, Deserialize)]
pub struct CargoToml {
    /// Package section
    pub package: Option<Package>,
    /// Workspace section
    pub workspace: Option<Workspace>,
}

/// A field that can be either a direct value or inherited from the workspace
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum MaybeWorkspace<T> {
    Value(T),
    #[allow(dead_code)]
    Workspace {
        workspace: bool,
    },
}

impl<T: Default> MaybeWorkspace<T> {
    /// Get the direct value, or default if workspace-inherited
    fn into_value(self) -> T {
        match self {
            Self::Value(v) => v,
            Self::Workspace { .. } => T::default(),
        }
    }
}

/// Package section
#[derive(Debug, Clone, Deserialize)]
pub struct Package {
    /// Package name
    pub name: String,
    /// Package version (may be inherited from workspace)
    #[serde(deserialize_with = "deserialize_version")]
    pub version: String,
    /// Whether the version is inherited from the workspace
    #[serde(skip)]
    pub version_is_workspace: bool,
    /// Package description
    pub description: Option<String>,
    /// Authors
    pub authors: Option<Vec<String>>,
    /// License
    #[serde(default, deserialize_with = "deserialize_optional_maybe_workspace")]
    pub license: Option<String>,
    /// License file
    #[serde(rename = "license-file")]
    pub license_file: Option<String>,
    /// Repository
    #[serde(default, deserialize_with = "deserialize_optional_maybe_workspace")]
    pub repository: Option<String>,
    /// Whether to publish
    pub publish: Option<bool>,
    /// Edition
    #[serde(default, deserialize_with = "deserialize_optional_maybe_workspace")]
    pub edition: Option<String>,
    /// Rust version requirement
    #[serde(
        rename = "rust-version",
        default,
        deserialize_with = "deserialize_optional_maybe_workspace"
    )]
    pub rust_version: Option<String>,
    /// Default run target
    #[serde(rename = "default-run")]
    pub default_run: Option<String>,
}

/// Deserialize a version field that may be a string or `{ workspace = true }`
fn deserialize_version<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = MaybeWorkspace::<String>::deserialize(deserializer)?;
    Ok(value.into_value())
}

/// Deserialize an optional field that may be a string or `{ workspace = true }`
fn deserialize_optional_maybe_workspace<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<MaybeWorkspace<String>>::deserialize(deserializer)?;
    Ok(value.map(|v| v.into_value()))
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
    /// Workspace-level package metadata (inherited by members)
    pub package: Option<WorkspacePackage>,
}

/// Workspace-level package metadata that members can inherit
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspacePackage {
    /// Version shared by all workspace members
    pub version: Option<String>,
    /// Edition shared by all workspace members
    pub edition: Option<String>,
}

impl CargoToml {
    /// Load Cargo.toml from a file path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        let mut manifest: Self = toml::from_str(&content)
            .map_err(|e| AdapterError::ManifestParseError(e.to_string()))?;

        // If version is empty (workspace-inherited), try to resolve from workspace root
        if let Some(ref mut pkg) = manifest.package {
            if pkg.version.is_empty() {
                if let Some(workspace_version) = Self::find_workspace_version(path) {
                    pkg.version = workspace_version;
                    pkg.version_is_workspace = true;
                }
            }
        }

        Ok(manifest)
    }

    /// Walk up directories to find the workspace root and read its version
    fn find_workspace_version(manifest_path: &Path) -> Option<String> {
        let mut dir = manifest_path.parent()?.parent()?; // Go up from the crate dir
        for _ in 0..5 {
            let candidate = dir.join("Cargo.toml");
            if candidate.exists() && candidate != *manifest_path {
                let content = std::fs::read_to_string(&candidate).ok()?;
                // Parse just enough to check for [workspace.package.version]
                let doc: toml::Value = toml::from_str(&content).ok()?;
                let version = doc
                    .get("workspace")?
                    .get("package")?
                    .get("version")?
                    .as_str()?;
                return Some(version.to_string());
            }
            dir = dir.parent()?;
        }
        None
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

impl ManifestFile for CargoToml {
    fn filename() -> &'static str {
        "Cargo.toml"
    }

    fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(Self::filename());
        CargoToml::load_from_path(&path).map_err(Into::into)
    }

    fn save(&self, dir: &Path) -> anyhow::Result<()> {
        // CargoToml uses toml_edit for version updates to preserve formatting.
        // A full save from the deserialized struct would lose comments/formatting,
        // so this is only useful after set_version which tracks the pending version.
        // For Cargo, prefer using CargoToml::update_version() directly for version changes.
        let path = dir.join(Self::filename());
        if let Some(ref pkg) = self.package {
            CargoToml::update_version(&path, &pkg.version)?;
        }
        Ok(())
    }

    fn version(&self) -> Option<&str> {
        self.package.as_ref().map(|p| p.version.as_str())
    }

    fn set_version(&mut self, version: &str) -> anyhow::Result<()> {
        if let Some(ref mut pkg) = self.package {
            pkg.version = version.to_string();
            Ok(())
        } else {
            anyhow::bail!("No [package] section found in Cargo.toml")
        }
    }

    fn name(&self) -> Option<&str> {
        self.package.as_ref().map(|p| p.name.as_str())
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

        let cargo = CargoToml::load_from_path(&path).unwrap();
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

        let cargo = CargoToml::load_from_path(&path).unwrap();
        assert!(cargo.package.is_none());
        assert!(cargo.workspace.is_some());
    }
}
