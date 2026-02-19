//! Serde structs matching the aqua registry YAML format

use std::collections::HashMap;

use serde::Deserialize;

/// Top-level registry YAML file
#[derive(Debug, Clone, Deserialize)]
pub struct AquaRegistryFile {
    pub packages: Vec<AquaPackage>,
}

/// A package definition in the aqua registry
#[derive(Debug, Clone, Deserialize)]
pub struct AquaPackage {
    /// Package type: "github_release" or "http"
    #[serde(rename = "type")]
    pub pkg_type: String,

    /// GitHub repo owner (for github_release)
    pub repo_owner: Option<String>,

    /// GitHub repo name (for github_release)
    pub repo_name: Option<String>,

    /// Asset filename template (Go template syntax)
    pub asset: Option<String>,

    /// Archive format: "tar.gz", "zip", "raw", etc.
    pub format: Option<String>,

    /// Files to extract from the archive
    #[serde(default)]
    pub files: Vec<AquaFile>,

    /// OS/Arch string replacements (e.g. "amd64" -> "x86_64")
    #[serde(default)]
    pub replacements: HashMap<String, HashMap<String, String>>,

    /// Supported environments (e.g. ["linux/amd64", "darwin/arm64"])
    #[serde(default)]
    pub supported_envs: Vec<String>,

    /// Platform-specific overrides
    #[serde(default)]
    pub overrides: Vec<AquaOverride>,

    /// Version-specific overrides
    #[serde(default)]
    pub version_overrides: Vec<AquaVersionOverride>,

    /// Version tag prefix (default: "v")
    pub version_prefix: Option<String>,

    /// Checksum configuration
    pub checksum: Option<AquaChecksum>,

    /// URL template (for http type)
    pub url: Option<String>,

    /// Description
    pub description: Option<String>,

    /// Link (documentation URL)
    pub link: Option<String>,
}

/// A file to extract from an archive
#[derive(Debug, Clone, Deserialize)]
pub struct AquaFile {
    /// Destination filename
    pub name: String,
    /// Source path within the archive (Go template)
    #[serde(default)]
    pub src: Option<String>,
}

/// Platform-specific override
#[derive(Debug, Clone, Deserialize)]
pub struct AquaOverride {
    /// Target OS (Go-style, e.g. "darwin", "linux", "windows")
    pub goos: Option<String>,
    /// Target arch (Go-style, e.g. "amd64", "arm64")
    pub goarch: Option<String>,
    /// Override asset template
    pub asset: Option<String>,
    /// Override archive format
    pub format: Option<String>,
    /// Override replacements
    #[serde(default)]
    pub replacements: Option<HashMap<String, HashMap<String, String>>>,
    /// Override files
    pub files: Option<Vec<AquaFile>>,
    /// Override URL
    pub url: Option<String>,
}

/// Version-specific override
#[derive(Debug, Clone, Deserialize)]
pub struct AquaVersionOverride {
    /// Version constraint (semver expression or "true" for catch-all)
    pub version_constraint: String,
    /// Override package type
    #[serde(rename = "type")]
    pub pkg_type: Option<String>,
    /// Override asset template
    pub asset: Option<String>,
    /// Override format
    pub format: Option<String>,
    /// Override files
    pub files: Option<Vec<AquaFile>>,
    /// Override replacements
    pub replacements: Option<HashMap<String, HashMap<String, String>>>,
    /// Override supported envs
    pub supported_envs: Option<Vec<String>>,
    /// Override overrides
    pub overrides: Option<Vec<AquaOverride>>,
    /// Override version prefix
    pub version_prefix: Option<String>,
    /// Override checksum
    pub checksum: Option<AquaChecksum>,
    /// Override URL
    pub url: Option<String>,
}

/// Checksum verification configuration
#[derive(Debug, Clone, Deserialize)]
pub struct AquaChecksum {
    /// Type of checksum: "github_release" or "http"
    #[serde(rename = "type")]
    pub checksum_type: Option<String>,
    /// Checksum asset filename template
    pub asset: Option<String>,
    /// Hash algorithm (default: "sha256")
    pub algorithm: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_github_release_package() {
        let yaml = r#"
packages:
  - type: github_release
    repo_owner: BurntSushi
    repo_name: ripgrep
    asset: ripgrep-{{.Version}}-{{.Arch}}-{{.OS}}.tar.gz
    format: tar.gz
    files:
      - name: rg
        src: ripgrep-{{.Version}}-{{.Arch}}-{{.OS}}/rg
    replacements:
      linux:
        amd64: x86_64
      darwin:
        amd64: x86_64
    supported_envs:
      - linux/amd64
      - darwin/arm64
      - darwin/amd64
"#;
        let reg: AquaRegistryFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(reg.packages.len(), 1);
        let pkg = &reg.packages[0];
        assert_eq!(pkg.pkg_type, "github_release");
        assert_eq!(pkg.repo_owner.as_deref(), Some("BurntSushi"));
        assert_eq!(pkg.repo_name.as_deref(), Some("ripgrep"));
        assert_eq!(pkg.files.len(), 1);
        assert_eq!(pkg.files[0].name, "rg");
        assert_eq!(pkg.supported_envs.len(), 3);
    }

    #[test]
    fn parse_http_package() {
        let yaml = r#"
packages:
  - type: http
    repo_owner: jqlang
    repo_name: jq
    url: https://github.com/jqlang/jq/releases/download/jq-{{.Version}}/jq-{{.OS}}-{{.Arch}}
    format: raw
    files:
      - name: jq
"#;
        let reg: AquaRegistryFile = serde_yaml::from_str(yaml).unwrap();
        let pkg = &reg.packages[0];
        assert_eq!(pkg.pkg_type, "http");
        assert!(pkg.url.is_some());
    }

    #[test]
    fn parse_version_overrides() {
        let yaml = r#"
packages:
  - type: github_release
    repo_owner: sharkdp
    repo_name: fd
    asset: fd-v{{.Version}}-{{.Arch}}-{{.OS}}.tar.gz
    format: tar.gz
    files:
      - name: fd
    version_overrides:
      - version_constraint: "semver(< 8.0.0)"
        asset: fd-v{{.Version}}-{{.Arch}}-{{.OS}}.tar.gz
        format: tar.gz
"#;
        let reg: AquaRegistryFile = serde_yaml::from_str(yaml).unwrap();
        let pkg = &reg.packages[0];
        assert_eq!(pkg.version_overrides.len(), 1);
        assert_eq!(
            pkg.version_overrides[0].version_constraint,
            "semver(< 8.0.0)"
        );
    }

    #[test]
    fn parse_overrides() {
        let yaml = r#"
packages:
  - type: github_release
    repo_owner: example
    repo_name: tool
    asset: tool-{{.OS}}-{{.Arch}}
    format: raw
    overrides:
      - goos: windows
        asset: tool-{{.OS}}-{{.Arch}}.exe
        format: raw
"#;
        let reg: AquaRegistryFile = serde_yaml::from_str(yaml).unwrap();
        let pkg = &reg.packages[0];
        assert_eq!(pkg.overrides.len(), 1);
        assert_eq!(pkg.overrides[0].goos.as_deref(), Some("windows"));
    }
}
