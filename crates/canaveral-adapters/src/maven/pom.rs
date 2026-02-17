//! pom.xml file parsing and manipulation

use std::path::Path;

use canaveral_core::error::{AdapterError, Result};

/// Parsed pom.xml file
#[derive(Debug, Clone, Default)]
pub struct PomXml {
    /// Group ID
    pub group_id: Option<String>,
    /// Artifact ID
    pub artifact_id: Option<String>,
    /// Version
    pub version: Option<String>,
    /// Packaging type (jar, war, pom, etc.)
    pub packaging: Option<String>,
    /// Project name
    pub name: Option<String>,
    /// Project description
    pub description: Option<String>,
    /// Project URL
    pub url: Option<String>,
    /// Licenses
    pub licenses: Vec<License>,
    /// Developers
    pub developers: Vec<Developer>,
    /// SCM information
    pub scm: Option<Scm>,
    /// Parent POM
    pub parent: Option<Parent>,
}

/// License information
#[derive(Debug, Clone)]
pub struct License {
    /// License name
    pub name: String,
    /// License URL
    pub url: Option<String>,
}

/// Developer information
#[derive(Debug, Clone)]
pub struct Developer {
    /// Developer ID
    pub id: Option<String>,
    /// Developer name
    pub name: Option<String>,
    /// Developer email
    pub email: Option<String>,
}

/// SCM information
#[derive(Debug, Clone)]
pub struct Scm {
    /// Connection URL
    pub connection: Option<String>,
    /// Developer connection URL
    pub developer_connection: Option<String>,
    /// Web URL
    pub url: Option<String>,
    /// Tag
    pub tag: Option<String>,
}

/// Parent POM
#[derive(Debug, Clone)]
pub struct Parent {
    /// Group ID
    pub group_id: String,
    /// Artifact ID
    pub artifact_id: String,
    /// Version
    pub version: String,
}

impl PomXml {
    /// Load a pom.xml file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AdapterError::ManifestParseError(format!("Failed to read pom.xml: {}", e))
        })?;

        Self::parse(&content)
    }

    /// Parse pom.xml content
    pub fn parse(content: &str) -> Result<Self> {
        // Simple XML parsing without a full XML library
        // This handles the most common cases
        let mut group_id = Self::extract_element(content, "groupId");
        let artifact_id = Self::extract_element(content, "artifactId");
        let mut version = Self::extract_element(content, "version");
        let packaging = Self::extract_element(content, "packaging");
        let name = Self::extract_element(content, "name");
        let description = Self::extract_element(content, "description");
        let url = Self::extract_element(content, "url");

        // Parse parent if present
        let parent = if let Some(parent_block) = Self::extract_block(content, "parent") {
            let parent = Parent {
                group_id: Self::extract_element(&parent_block, "groupId").unwrap_or_default(),
                artifact_id: Self::extract_element(&parent_block, "artifactId").unwrap_or_default(),
                version: Self::extract_element(&parent_block, "version").unwrap_or_default(),
            };

            // Inherit groupId and version from parent if not specified
            if group_id.is_none() {
                group_id = Some(parent.group_id.clone());
            }
            if version.is_none() {
                version = Some(parent.version.clone());
            }
            Some(parent)
        } else {
            None
        };

        // Parse licenses
        let licenses = Self::extract_block(content, "licenses")
            .map(|b| Self::parse_licenses(&b))
            .unwrap_or_default();

        // Parse developers
        let developers = Self::extract_block(content, "developers")
            .map(|b| Self::parse_developers(&b))
            .unwrap_or_default();

        // Parse SCM
        let scm = Self::extract_block(content, "scm").map(|scm_block| Scm {
            connection: Self::extract_element(&scm_block, "connection"),
            developer_connection: Self::extract_element(&scm_block, "developerConnection"),
            url: Self::extract_element(&scm_block, "url"),
            tag: Self::extract_element(&scm_block, "tag"),
        });

        Ok(PomXml {
            group_id,
            artifact_id,
            version,
            packaging,
            name,
            description,
            url,
            licenses,
            developers,
            scm,
            parent,
        })
    }

    /// Extract a simple element value
    fn extract_element(content: &str, element: &str) -> Option<String> {
        let start_tag = format!("<{}>", element);
        let end_tag = format!("</{}>", element);

        if let Some(start) = content.find(&start_tag) {
            if let Some(end) = content[start..].find(&end_tag) {
                let value_start = start + start_tag.len();
                let value_end = start + end;
                let value = content[value_start..value_end].trim();
                if !value.is_empty() {
                    return Some(value.to_string());
                }
            }
        }

        None
    }

    /// Extract a block of XML
    fn extract_block(content: &str, element: &str) -> Option<String> {
        let start_tag = format!("<{}", element);
        let end_tag = format!("</{}>", element);

        if let Some(start) = content.find(&start_tag) {
            // Find the end of the start tag
            let tag_end = content[start..].find('>')? + start + 1;
            if let Some(end) = content[tag_end..].find(&end_tag) {
                return Some(content[tag_end..tag_end + end].to_string());
            }
        }

        None
    }

    /// Parse licenses from a licenses block
    fn parse_licenses(block: &str) -> Vec<License> {
        let mut licenses = Vec::new();

        // Find each <license> block
        let mut pos = 0;
        while let Some(start) = block[pos..].find("<license>") {
            let start = pos + start;
            if let Some(end) = block[start..].find("</license>") {
                let license_block = &block[start..start + end];
                licenses.push(License {
                    name: Self::extract_element(license_block, "name").unwrap_or_default(),
                    url: Self::extract_element(license_block, "url"),
                });
                pos = start + end;
            } else {
                break;
            }
        }

        licenses
    }

    /// Parse developers from a developers block
    fn parse_developers(block: &str) -> Vec<Developer> {
        let mut developers = Vec::new();

        let mut pos = 0;
        while let Some(start) = block[pos..].find("<developer>") {
            let start = pos + start;
            if let Some(end) = block[start..].find("</developer>") {
                let dev_block = &block[start..start + end];
                developers.push(Developer {
                    id: Self::extract_element(dev_block, "id"),
                    name: Self::extract_element(dev_block, "name"),
                    email: Self::extract_element(dev_block, "email"),
                });
                pos = start + end;
            } else {
                break;
            }
        }

        developers
    }

    /// Update the version in a pom.xml file
    pub fn update_version(path: &Path, new_version: &str) -> Result<()> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AdapterError::ManifestParseError(format!("Failed to read pom.xml: {}", e))
        })?;

        // Find and replace the version element (top-level, not in dependencies)
        // We need to be careful to only replace the project version, not dependency versions

        let new_content = Self::replace_project_version(&content, new_version)?;

        std::fs::write(path, new_content).map_err(|e| {
            AdapterError::ManifestUpdateError(format!("Failed to write pom.xml: {}", e))
        })?;

        Ok(())
    }

    /// Replace the project version in pom.xml content
    fn replace_project_version(content: &str, new_version: &str) -> Result<String> {
        // Find the project's version element (not inside parent, dependencies, etc.)
        // This is a simplified approach - for production, use a proper XML parser

        let lines: Vec<&str> = content.lines().collect();
        let mut result = Vec::new();
        let mut in_parent = false;
        let mut in_dependencies = false;
        let mut in_dependency = false;
        let mut version_replaced = false;

        for line in lines {
            let trimmed = line.trim();

            // Track nesting
            if trimmed.starts_with("<parent") {
                in_parent = true;
            } else if trimmed.starts_with("</parent>") {
                in_parent = false;
            } else if trimmed.starts_with("<dependencies") {
                in_dependencies = true;
            } else if trimmed.starts_with("</dependencies>") {
                in_dependencies = false;
            } else if trimmed.starts_with("<dependency") {
                in_dependency = true;
            } else if trimmed.starts_with("</dependency>") {
                in_dependency = false;
            }

            // Replace version only if not in parent/dependencies
            if !version_replaced
                && !in_parent
                && !in_dependencies
                && !in_dependency
                && trimmed.starts_with("<version>")
                && trimmed.ends_with("</version>")
            {
                let indent = line.len() - line.trim_start().len();
                let spaces = " ".repeat(indent);
                result.push(format!("{}<version>{}</version>", spaces, new_version));
                version_replaced = true;
            } else {
                result.push(line.to_string());
            }
        }

        if !version_replaced {
            return Err(AdapterError::ManifestUpdateError(
                "Could not find project version in pom.xml".to_string(),
            )
            .into());
        }

        Ok(result.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_simple() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>my-project</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>
</project>"#;

        let pom = PomXml::parse(content).unwrap();
        assert_eq!(pom.group_id, Some("com.example".to_string()));
        assert_eq!(pom.artifact_id, Some("my-project".to_string()));
        assert_eq!(pom.version, Some("1.0.0".to_string()));
        assert_eq!(pom.packaging, Some("jar".to_string()));
    }

    #[test]
    fn test_parse_with_parent() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>3.0.0</version>
    </parent>
    <artifactId>my-app</artifactId>
</project>"#;

        let pom = PomXml::parse(content).unwrap();
        assert!(pom.parent.is_some());
        let parent = pom.parent.unwrap();
        assert_eq!(parent.group_id, "org.springframework.boot");
        assert_eq!(parent.version, "3.0.0");

        // groupId should be inherited from parent
        assert_eq!(pom.group_id, Some("org.springframework.boot".to_string()));
    }

    #[test]
    fn test_parse_with_licenses() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0.0</version>
    <licenses>
        <license>
            <name>MIT License</name>
            <url>https://opensource.org/licenses/MIT</url>
        </license>
    </licenses>
</project>"#;

        let pom = PomXml::parse(content).unwrap();
        assert_eq!(pom.licenses.len(), 1);
        assert_eq!(pom.licenses[0].name, "MIT License");
    }

    #[test]
    fn test_parse_with_developers() {
        let content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0.0</version>
    <developers>
        <developer>
            <id>johndoe</id>
            <name>John Doe</name>
            <email>john@example.com</email>
        </developer>
    </developers>
</project>"#;

        let pom = PomXml::parse(content).unwrap();
        assert_eq!(pom.developers.len(), 1);
        assert_eq!(pom.developers[0].id, Some("johndoe".to_string()));
        assert_eq!(pom.developers[0].name, Some("John Doe".to_string()));
    }

    #[test]
    fn test_update_version() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        std::fs::write(
            &pom_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0.0</version>
</project>"#,
        )
        .unwrap();

        PomXml::update_version(&pom_path, "2.0.0").unwrap();

        let updated = PomXml::load(&pom_path).unwrap();
        assert_eq!(updated.version, Some("2.0.0".to_string()));
    }

    #[test]
    fn test_update_version_preserves_dependency_versions() {
        let temp = TempDir::new().unwrap();
        let pom_path = temp.path().join("pom.xml");

        std::fs::write(
            &pom_path,
            r#"<?xml version="1.0" encoding="UTF-8"?>
<project>
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test</artifactId>
    <version>1.0.0</version>
    <dependencies>
        <dependency>
            <groupId>org.junit</groupId>
            <artifactId>junit</artifactId>
            <version>5.9.0</version>
        </dependency>
    </dependencies>
</project>"#,
        )
        .unwrap();

        PomXml::update_version(&pom_path, "2.0.0").unwrap();

        let content = std::fs::read_to_string(&pom_path).unwrap();
        // Project version should be updated
        assert!(content.contains("<version>2.0.0</version>"));
        // Dependency version should remain unchanged
        assert!(content.contains("<version>5.9.0</version>"));
    }
}
