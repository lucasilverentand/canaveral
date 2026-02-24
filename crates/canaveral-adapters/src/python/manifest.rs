//! pyproject.toml handling

use std::path::Path;

use canaveral_core::error::{AdapterError, Result};
use toml_edit::{value, DocumentMut};

use crate::manifest::ManifestFile;

/// Parsed pyproject.toml with format-preserving document
#[derive(Debug, Clone)]
pub struct PyProjectToml {
    /// The raw toml_edit document (preserves formatting)
    doc: DocumentMut,
}

impl PyProjectToml {
    /// Load pyproject.toml from a file path
    pub fn load_from_path(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .map_err(|_| AdapterError::ManifestNotFound(path.to_path_buf()))?;

        let doc: DocumentMut = content
            .parse()
            .map_err(|e: toml_edit::TomlError| AdapterError::ManifestParseError(e.to_string()))?;

        Ok(Self { doc })
    }

    /// Save pyproject.toml to a file path
    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        std::fs::write(path, self.doc.to_string())
            .map_err(|e| AdapterError::ManifestUpdateError(e.to_string()).into())
    }

    /// Get the project description
    pub fn description(&self) -> Option<&str> {
        self.doc
            .get("project")
            .and_then(|p| p.get("description"))
            .and_then(|d| d.as_str())
    }

    /// Check if project has a readme field
    pub fn has_readme(&self) -> bool {
        self.doc
            .get("project")
            .and_then(|p| p.get("readme"))
            .is_some()
    }

    /// Check if project has a license field
    pub fn has_license(&self) -> bool {
        self.doc
            .get("project")
            .and_then(|p| p.get("license"))
            .is_some()
    }

    /// Check if there is a [project] section
    pub fn has_project_section(&self) -> bool {
        self.doc.get("project").is_some()
    }

    /// Check if there is a [build-system] section
    pub fn has_build_system(&self) -> bool {
        self.doc.get("build-system").is_some()
    }

    /// Access the underlying document
    pub fn doc(&self) -> &DocumentMut {
        &self.doc
    }
}

impl ManifestFile for PyProjectToml {
    fn filename() -> &'static str {
        "pyproject.toml"
    }

    fn load(dir: &Path) -> anyhow::Result<Self> {
        let path = dir.join(Self::filename());
        PyProjectToml::load_from_path(&path).map_err(Into::into)
    }

    fn save(&self, dir: &Path) -> anyhow::Result<()> {
        let path = dir.join(Self::filename());
        self.save_to_path(&path).map_err(Into::into)
    }

    fn version(&self) -> Option<&str> {
        self.doc
            .get("project")
            .and_then(|p| p.get("version"))
            .and_then(|v| v.as_str())
    }

    fn set_version(&mut self, version: &str) -> anyhow::Result<()> {
        if let Some(project) = self.doc.get_mut("project") {
            if let Some(table) = project.as_table_mut() {
                table["version"] = value(version);
                return Ok(());
            }
        }
        anyhow::bail!("No [project] section found in pyproject.toml")
    }

    fn name(&self) -> Option<&str> {
        self.doc
            .get("project")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::ManifestFile;
    use tempfile::TempDir;

    #[test]
    fn test_load_and_read() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
name = "my-package"
version = "1.0.0"
description = "A test package"
"#,
        )
        .unwrap();

        let manifest = PyProjectToml::load(temp.path()).unwrap();
        assert_eq!(manifest.name(), Some("my-package"));
        assert_eq!(manifest.version(), Some("1.0.0"));
        assert_eq!(manifest.description(), Some("A test package"));
    }

    #[test]
    fn test_set_version_and_save() {
        let temp = TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("pyproject.toml"),
            r#"
[project]
name = "my-package"
version = "1.0.0"
description = "A test"
"#,
        )
        .unwrap();

        let mut manifest = PyProjectToml::load(temp.path()).unwrap();
        manifest.set_version("2.0.0").unwrap();
        manifest.save(temp.path()).unwrap();

        let reloaded = PyProjectToml::load(temp.path()).unwrap();
        assert_eq!(reloaded.version(), Some("2.0.0"));

        // Check formatting preserved
        let content = std::fs::read_to_string(temp.path().join("pyproject.toml")).unwrap();
        assert!(content.contains("description"));
    }
}
