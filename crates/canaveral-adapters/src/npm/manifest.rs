//! npm package.json handling

use std::collections::HashMap;
use std::path::Path;

use canaveral_core::error::{AdapterError, Result};
use serde::{Deserialize, Serialize};

/// package.json structure
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageJson {
    /// Package name
    pub name: String,

    /// Package version
    pub version: String,

    /// Package description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Main entry point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,

    /// Module entry point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,

    /// Types entry point
    #[serde(skip_serializing_if = "Option::is_none")]
    pub types: Option<String>,

    /// Scripts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<HashMap<String, String>>,

    /// Dependencies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependencies: Option<HashMap<String, String>>,

    /// Dev dependencies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dev_dependencies: Option<HashMap<String, String>>,

    /// Peer dependencies
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peer_dependencies: Option<HashMap<String, String>>,

    /// Whether package is private
    #[serde(skip_serializing_if = "Option::is_none")]
    pub private: Option<bool>,

    /// Repository info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<serde_json::Value>,

    /// Author
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<serde_json::Value>,

    /// License
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    /// Keywords
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keywords: Option<Vec<String>>,

    /// Files to include in package
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,

    /// Exports map
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exports: Option<serde_json::Value>,

    /// Engines
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engines: Option<HashMap<String, String>>,

    /// Preserve other fields
    #[serde(flatten)]
    pub other: HashMap<String, serde_json::Value>,
}

impl PackageJson {
    /// Load package.json from path
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        serde_json::from_str(&content)
            .map_err(|e| AdapterError::ManifestParseError(e.to_string()).into())
    }

    /// Save package.json to path
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| AdapterError::ManifestUpdateError(e.to_string()))?;

        // Ensure trailing newline
        let content = if content.ends_with('\n') {
            content
        } else {
            format!("{}\n", content)
        };

        std::fs::write(path, content)
            .map_err(|e| AdapterError::ManifestUpdateError(e.to_string()).into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_minimal() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.json");

        std::fs::write(&path, r#"{"name": "test", "version": "1.0.0"}"#).unwrap();

        let pkg = PackageJson::load(&path).unwrap();
        assert_eq!(pkg.name, "test");
        assert_eq!(pkg.version, "1.0.0");
    }

    #[test]
    fn test_load_full() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.json");

        std::fs::write(
            &path,
            r#"{
                "name": "test-package",
                "version": "1.2.3",
                "description": "A test package",
                "main": "dist/index.js",
                "scripts": {
                    "build": "tsc",
                    "test": "jest"
                },
                "dependencies": {
                    "lodash": "^4.17.21"
                },
                "devDependencies": {
                    "typescript": "^5.0.0"
                },
                "private": false,
                "license": "MIT"
            }"#,
        )
        .unwrap();

        let pkg = PackageJson::load(&path).unwrap();
        assert_eq!(pkg.name, "test-package");
        assert_eq!(pkg.version, "1.2.3");
        assert_eq!(pkg.description, Some("A test package".to_string()));
        assert!(pkg.scripts.is_some());
        assert!(!pkg.private.unwrap_or(true));
    }

    #[test]
    fn test_save() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.json");

        std::fs::write(&path, r#"{"name": "test", "version": "1.0.0"}"#).unwrap();

        let mut pkg = PackageJson::load(&path).unwrap();
        pkg.version = "2.0.0".to_string();
        pkg.save(&path).unwrap();

        let loaded = PackageJson::load(&path).unwrap();
        assert_eq!(loaded.version, "2.0.0");
    }

    #[test]
    fn test_preserves_extra_fields() {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join("package.json");

        std::fs::write(
            &path,
            r#"{"name": "test", "version": "1.0.0", "customField": "value"}"#,
        )
        .unwrap();

        let mut pkg = PackageJson::load(&path).unwrap();
        assert!(pkg.other.contains_key("customField"));

        pkg.version = "2.0.0".to_string();
        pkg.save(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("customField"));
    }
}
