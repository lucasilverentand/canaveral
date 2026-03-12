//! Data structures for embedded tool definitions

use std::collections::HashMap;

use serde::Deserialize;

/// A tool definition loaded from an embedded TOML file.
///
/// Each definition describes how to download, extract, and detect a single
/// tool binary from a GitHub release.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDefinition {
    /// Unique identifier (e.g. "ripgrep")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Binary name on disk (e.g. "rg")
    pub binary: String,
    /// GitHub `owner/repo` (e.g. "BurntSushi/ripgrep")
    pub repo: String,
    /// Git tag prefix before the version number (default: "v")
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    /// Alternative names that resolve to this tool
    #[serde(default)]
    pub aliases: Vec<String>,
    /// How to detect the installed version
    #[serde(default)]
    pub version_detect: VersionDetect,
    /// Asset template for GitHub releases (uses `{version}`, `{os}`, `{arch}`)
    pub asset: Option<String>,
    /// Direct URL template (overrides GitHub releases URL construction)
    pub url: Option<String>,
    /// Archive format: "tar.gz", "zip", or "raw" (default: "tar.gz")
    #[serde(default = "default_format")]
    pub format: String,
    /// Files to extract from the archive
    #[serde(default)]
    pub files: Vec<FileMapping>,
    /// Platform key → OS/arch values used in template expansion
    #[serde(default)]
    pub platforms: HashMap<String, PlatformMapping>,
    /// Platform key → field overrides (format, asset, url, files)
    #[serde(default)]
    pub platform_overrides: HashMap<String, PlatformOverride>,
}

fn default_tag_prefix() -> String {
    "v".to_string()
}

fn default_format() -> String {
    "tar.gz".to_string()
}

/// Configuration for detecting an installed tool's version.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionDetect {
    /// Arguments to pass to the binary (default: `["--version"]`)
    #[serde(default = "default_version_args")]
    pub args: Vec<String>,
    /// Regex to extract the version string from output (default: `(\d+\.\d+\.\d+)`)
    #[serde(default = "default_version_regex")]
    pub regex: String,
}

impl Default for VersionDetect {
    fn default() -> Self {
        Self {
            args: default_version_args(),
            regex: default_version_regex(),
        }
    }
}

fn default_version_args() -> Vec<String> {
    vec!["--version".to_string()]
}

fn default_version_regex() -> String {
    r"(\d+\.\d+\.\d+)".to_string()
}

/// Maps an archive entry to a destination binary name.
#[derive(Debug, Clone, Deserialize)]
pub struct FileMapping {
    /// Destination filename in the `bin/` directory
    pub name: String,
    /// Source path inside the archive (supports `{version}`, `{os}`, `{arch}`)
    pub src: Option<String>,
}

/// OS/arch values for a platform key, used in template expansion.
#[derive(Debug, Clone, Deserialize)]
pub struct PlatformMapping {
    /// OS string to substitute into templates (e.g. "apple-darwin")
    pub os: String,
    /// Architecture string to substitute into templates (e.g. "aarch64")
    pub arch: String,
}

/// Per-platform overrides for fields that differ across OS/arch combinations.
#[derive(Debug, Clone, Deserialize)]
pub struct PlatformOverride {
    /// Override archive format
    pub format: Option<String>,
    /// Override asset template
    pub asset: Option<String>,
    /// Override URL template
    pub url: Option<String>,
    /// Override file mappings
    pub files: Option<Vec<FileMapping>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_full_toml_definition() {
        let toml_str = r#"
id = "ripgrep"
name = "ripgrep"
binary = "rg"
repo = "BurntSushi/ripgrep"
aliases = ["rg"]
tag_prefix = "v"
asset = "ripgrep-{version}-{arch}-{os}.tar.gz"
format = "tar.gz"

[version_detect]
regex = '(\d+\.\d+\.\d+)'

[[files]]
name = "rg"
src = "ripgrep-{version}-{arch}-{os}/rg"

[platforms.darwin-aarch64]
os = "apple-darwin"
arch = "aarch64"

[platforms.darwin-x86_64]
os = "apple-darwin"
arch = "x86_64"

[platforms.linux-x86_64]
os = "unknown-linux-musl"
arch = "x86_64"

[platform_overrides.windows-x86_64]
format = "zip"
asset = "ripgrep-{version}-{arch}-pc-windows-msvc.zip"
"#;
        let def: ToolDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(def.id, "ripgrep");
        assert_eq!(def.name, "ripgrep");
        assert_eq!(def.binary, "rg");
        assert_eq!(def.repo, "BurntSushi/ripgrep");
        assert_eq!(def.tag_prefix, "v");
        assert_eq!(def.aliases, vec!["rg"]);
        assert_eq!(
            def.asset.as_deref(),
            Some("ripgrep-{version}-{arch}-{os}.tar.gz")
        );
        assert_eq!(def.format, "tar.gz");
        assert_eq!(def.files.len(), 1);
        assert_eq!(def.files[0].name, "rg");
        assert_eq!(
            def.files[0].src.as_deref(),
            Some("ripgrep-{version}-{arch}-{os}/rg")
        );
        assert_eq!(def.platforms.len(), 3);
        assert_eq!(def.platforms["darwin-aarch64"].os, "apple-darwin");
        assert_eq!(def.platforms["darwin-aarch64"].arch, "aarch64");
        assert_eq!(def.platform_overrides.len(), 1);
        assert_eq!(
            def.platform_overrides["windows-x86_64"].format.as_deref(),
            Some("zip")
        );
    }

    #[test]
    fn defaults_applied_for_missing_fields() {
        let toml_str = r#"
id = "mytool"
name = "My Tool"
binary = "mytool"
repo = "owner/mytool"
"#;
        let def: ToolDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(def.tag_prefix, "v");
        assert_eq!(def.format, "tar.gz");
        assert!(def.aliases.is_empty());
        assert!(def.files.is_empty());
        assert!(def.platforms.is_empty());
        assert!(def.platform_overrides.is_empty());
        assert!(def.asset.is_none());
        assert!(def.url.is_none());
    }

    #[test]
    fn default_version_detect() {
        let toml_str = r#"
id = "mytool"
name = "My Tool"
binary = "mytool"
repo = "owner/mytool"
"#;
        let def: ToolDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(def.version_detect.args, vec!["--version"]);
        assert_eq!(def.version_detect.regex, r"(\d+\.\d+\.\d+)");
    }

    #[test]
    fn custom_version_detect() {
        let toml_str = r#"
id = "mytool"
name = "My Tool"
binary = "mytool"
repo = "owner/mytool"

[version_detect]
args = ["version"]
regex = 'v(\d+\.\d+)'
"#;
        let def: ToolDefinition = toml::from_str(toml_str).unwrap();
        assert_eq!(def.version_detect.args, vec!["version"]);
        assert_eq!(def.version_detect.regex, r"v(\d+\.\d+)");
    }

    #[test]
    fn optional_url_overrides_asset() {
        let toml_str = r#"
id = "jq"
name = "jq"
binary = "jq"
repo = "jqlang/jq"
format = "raw"
url = "https://github.com/jqlang/jq/releases/download/jq-{version}/jq-{os}-{arch}"
"#;
        let def: ToolDefinition = toml::from_str(toml_str).unwrap();
        assert!(def.url.is_some());
        assert!(def.asset.is_none());
        assert_eq!(def.format, "raw");
    }
}
