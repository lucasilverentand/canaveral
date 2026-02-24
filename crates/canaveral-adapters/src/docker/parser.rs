//! Dockerfile parsing and image info extraction

use std::path::Path;

use canaveral_core::error::Result;

/// Extract image name and version from Dockerfile labels, package.json, or directory name.
pub fn parse_image_info(path: &Path) -> Result<(String, String)> {
    // Try to read image name from Dockerfile labels
    let dockerfile = path.join("Dockerfile");
    if dockerfile.exists() {
        if let Ok(content) = std::fs::read_to_string(&dockerfile) {
            for line in content.lines() {
                let line = line.trim();
                if line.starts_with("LABEL") {
                    if let Some(version) = extract_label(line, "version") {
                        let name = extract_label(line, "name")
                            .or_else(|| extract_label(line, "org.opencontainers.image.title"))
                            .unwrap_or_else(|| dir_name(path));
                        return Ok((name, version));
                    }
                }

                // OCI-style labels
                if line.starts_with("LABEL org.opencontainers.image.version=") {
                    let version = line
                        .strip_prefix("LABEL org.opencontainers.image.version=")
                        .unwrap_or("0.0.0")
                        .trim_matches('"')
                        .to_string();

                    return Ok((dir_name(path), version));
                }
            }
        }
    }

    // Try to get from package.json if it exists
    let package_json = path.join("package.json");
    if package_json.exists() {
        if let Ok(content) = std::fs::read_to_string(&package_json) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                let name = json["name"]
                    .as_str()
                    .map(|s| s.replace('@', "").replace('/', "-"))
                    .unwrap_or_else(|| "app".to_string());
                let version = json["version"].as_str().unwrap_or("0.0.0").to_string();
                return Ok((name, version));
            }
        }
    }

    // Fallback to directory name and 0.0.0
    Ok((dir_name(path), "0.0.0".to_string()))
}

/// Extract a label value from a LABEL line.
pub fn extract_label(line: &str, key: &str) -> Option<String> {
    let pattern = format!("{}=", key);
    if let Some(pos) = line.find(&pattern) {
        let rest = &line[pos + pattern.len()..];
        let value = if rest.starts_with('"') {
            rest.trim_start_matches('"').split('"').next().unwrap_or("")
        } else {
            rest.split_whitespace().next().unwrap_or("")
        };
        if !value.is_empty() {
            return Some(value.to_string());
        }
    }
    None
}

/// Get a directory name as a string, falling back to "app".
fn dir_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "app".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_get_info_from_dockerfile() {
        let temp = TempDir::new().unwrap();

        std::fs::write(
            temp.path().join("Dockerfile"),
            r#"FROM alpine:latest
LABEL org.opencontainers.image.version="1.2.3"
LABEL org.opencontainers.image.title="myapp"
"#,
        )
        .unwrap();

        let (_, version) = parse_image_info(temp.path()).unwrap();
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn test_get_info_from_package_json() {
        let temp = TempDir::new().unwrap();

        std::fs::write(temp.path().join("Dockerfile"), "FROM node:18\n").unwrap();
        std::fs::write(
            temp.path().join("package.json"),
            r#"{"name": "@scope/myapp", "version": "2.0.0"}"#,
        )
        .unwrap();

        let (name, version) = parse_image_info(temp.path()).unwrap();
        assert_eq!(version, "2.0.0");
        assert_eq!(name, "scope-myapp");
    }

    #[test]
    fn test_extract_label_quoted() {
        let line = r#"LABEL version="1.0.0" name="myapp""#;
        assert_eq!(extract_label(line, "version"), Some("1.0.0".to_string()));
        assert_eq!(extract_label(line, "name"), Some("myapp".to_string()));
    }

    #[test]
    fn test_extract_label_unquoted() {
        let line = "LABEL version=1.0.0";
        assert_eq!(extract_label(line, "version"), Some("1.0.0".to_string()));
    }

    #[test]
    fn test_extract_label_missing() {
        let line = "LABEL maintainer=\"test\"";
        assert_eq!(extract_label(line, "version"), None);
    }
}
