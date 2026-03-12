//! GenericProvider — ToolProvider backed by embedded TOML definitions

use std::io::Read as IoRead;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::providers::generic::platform::current_platform_key;
use crate::providers::generic::template::expand;
use crate::tool_defs::ToolDefinition;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

/// A tool provider backed by an embedded `ToolDefinition`.
///
/// Downloads pre-built binaries from GitHub releases (or a direct URL),
/// extracts them, and manages version detection.
pub struct GenericProvider {
    /// Leaked static id for the trait's `&'static str` return
    tool_id: &'static str,
    /// Leaked static name
    tool_name: &'static str,
    /// Leaked static binary name
    binary: &'static str,
    /// The full tool definition
    definition: ToolDefinition,
    /// HTTP client
    client: reqwest::Client,
}

impl GenericProvider {
    /// Create a `GenericProvider` from a `ToolDefinition`.
    pub fn new(def: ToolDefinition) -> Self {
        let tool_id: &'static str = Box::leak(def.id.clone().into_boxed_str());
        let tool_name: &'static str = Box::leak(def.name.clone().into_boxed_str());
        let binary: &'static str = Box::leak(def.binary.clone().into_boxed_str());

        Self {
            tool_id,
            tool_name,
            binary,
            definition: def,
            client: reqwest::Client::new(),
        }
    }

    /// Create a minimal `GenericProvider` from a `name` and `owner/repo` source.
    ///
    /// Used for the `get_with_source` path when the user specifies an explicit
    /// repo in config. Applies sensible defaults: the binary name equals the
    /// tool name, tag prefix is `v`, format is `tar.gz`.
    pub fn from_repo(name: &str, source: &str) -> Self {
        let def = ToolDefinition {
            id: name.to_string(),
            name: name.to_string(),
            binary: name.to_string(),
            repo: source.to_string(),
            tag_prefix: "v".to_string(),
            aliases: vec![],
            version_detect: Default::default(),
            asset: None,
            url: None,
            format: "tar.gz".to_string(),
            files: vec![],
            platforms: Default::default(),
            platform_overrides: Default::default(),
        };
        Self::new(def)
    }

    /// Build the download URL for a given version.
    ///
    /// Resolves the current platform, applies platform overrides, and expands
    /// the asset or URL template.
    fn build_download_url(&self, version: &str) -> Result<String, ToolError> {
        let platform_key = current_platform_key();
        let def = &self.definition;

        // Look up OS/arch for the current platform
        let (os, arch) = if let Some(pm) = def.platforms.get(platform_key) {
            (pm.os.as_str(), pm.arch.as_str())
        } else {
            // No platform mapping — use the raw platform key components
            let parts: Vec<&str> = platform_key.splitn(2, '-').collect();
            if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                return Err(ToolError::UnsupportedPlatform(self.tool_id.to_string()));
            }
        };

        // Check for platform overrides
        let overrides = def.platform_overrides.get(platform_key);
        let format = overrides
            .and_then(|o| o.format.as_deref())
            .unwrap_or(&def.format);
        let asset_template = overrides
            .and_then(|o| o.asset.as_deref())
            .or(def.asset.as_deref());
        let url_template = overrides
            .and_then(|o| o.url.as_deref())
            .or(def.url.as_deref());

        // If there's a direct URL template, use it
        if let Some(url_tmpl) = url_template {
            return Ok(expand(url_tmpl, version, os, arch));
        }

        // Otherwise, construct from GitHub releases + asset template
        let asset = asset_template.ok_or_else(|| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            reason: "no asset or url template in tool definition".to_string(),
        })?;

        let _ = format; // format is used for extraction, not URL construction
        let expanded_asset = expand(asset, version, os, arch);
        let tag = format!("{}{}", def.tag_prefix, version);

        Ok(format!(
            "https://github.com/{}/releases/download/{}/{}",
            def.repo, tag, expanded_asset
        ))
    }

    /// Resolve the effective format and file mappings for the current platform.
    fn resolve_platform(&self) -> (&str, &[crate::tool_defs::schema::FileMapping]) {
        let platform_key = current_platform_key();
        let def = &self.definition;
        let overrides = def.platform_overrides.get(platform_key);

        let format = overrides
            .and_then(|o| o.format.as_deref())
            .unwrap_or(&def.format);

        let files = overrides
            .and_then(|o| o.files.as_deref())
            .unwrap_or(&def.files);

        (format, files)
    }

    /// Download and extract a tool binary into `dest_dir`.
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = self.build_download_url(version)?;
        debug!(tool = self.tool_id, url = %url, "downloading");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: format!("failed to read download: {e}"),
            })?;

        let bin_dir = dest_dir.join("bin");
        std::fs::create_dir_all(&bin_dir)?;

        let (format, files) = self.resolve_platform();

        // Resolve OS/arch for file mapping expansion
        let platform_key = current_platform_key();
        let (os, arch) = if let Some(pm) = self.definition.platforms.get(platform_key) {
            (pm.os.as_str(), pm.arch.as_str())
        } else {
            let parts: Vec<&str> = platform_key.splitn(2, '-').collect();
            if parts.len() == 2 {
                (parts[0], parts[1])
            } else {
                ("unknown", "unknown")
            }
        };

        match format {
            "tar.gz" | "tgz" => {
                let decoder = flate2::read::GzDecoder::new(&bytes[..]);
                let mut archive = tar::Archive::new(decoder);
                self.extract_tar(&mut archive, files, version, os, arch, &bin_dir)?;
            }
            "zip" => {
                self.extract_zip(&bytes, files, version, os, arch, &bin_dir)?;
            }
            "raw" | "" => {
                let binary_name = if !files.is_empty() {
                    files[0].name.clone()
                } else {
                    self.binary.to_string()
                };
                let dest = bin_dir.join(&binary_name);
                std::fs::write(&dest, &bytes)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
                }
            }
            other => {
                return Err(ToolError::InstallFailed {
                    tool: self.tool_id.to_string(),
                    version: version.to_string(),
                    reason: format!("unsupported archive format: {other}"),
                });
            }
        }

        Ok(bin_dir)
    }

    fn extract_tar<R: IoRead>(
        &self,
        archive: &mut tar::Archive<R>,
        files: &[crate::tool_defs::schema::FileMapping],
        version: &str,
        os: &str,
        arch: &str,
        bin_dir: &Path,
    ) -> Result<(), ToolError> {
        let entries = archive.entries().map_err(|e| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            reason: format!("failed to read archive: {e}"),
        })?;

        // Build a map of source paths to destination names
        let file_mappings: Vec<(String, String)> = if files.is_empty() {
            vec![]
        } else {
            files
                .iter()
                .map(|f| {
                    let src = f
                        .src
                        .as_ref()
                        .map(|s| expand(s, version, os, arch))
                        .unwrap_or_else(|| f.name.clone());
                    (src, f.name.clone())
                })
                .collect()
        };

        for entry_result in entries {
            let mut entry = entry_result.map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: format!("failed to read archive entry: {e}"),
            })?;

            let path = entry
                .path()
                .map_err(|e| ToolError::InstallFailed {
                    tool: self.tool_id.to_string(),
                    version: version.to_string(),
                    reason: format!("invalid path in archive: {e}"),
                })?
                .to_path_buf();

            let path_str = path.to_string_lossy();

            if file_mappings.is_empty() {
                // No mappings: extract files that look like executables
                if entry.header().entry_type().is_file() {
                    if let Some(name) = path.file_name() {
                        let dest = bin_dir.join(name);
                        let mut contents = Vec::new();
                        entry
                            .read_to_end(&mut contents)
                            .map_err(|e| ToolError::InstallFailed {
                                tool: self.tool_id.to_string(),
                                version: version.to_string(),
                                reason: format!("failed to extract: {e}"),
                            })?;
                        std::fs::write(&dest, &contents)?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            std::fs::set_permissions(
                                &dest,
                                std::fs::Permissions::from_mode(0o755),
                            )?;
                        }
                    }
                }
            } else {
                // Match against file mappings
                for (src, dest_name) in &file_mappings {
                    if path_str.as_ref() == src
                        || path_str.ends_with(&format!("/{src}"))
                        || path
                            .file_name()
                            .map(|n| n.to_string_lossy().as_ref() == src.as_str())
                            == Some(true)
                    {
                        let dest = bin_dir.join(dest_name);
                        let mut contents = Vec::new();
                        entry
                            .read_to_end(&mut contents)
                            .map_err(|e| ToolError::InstallFailed {
                                tool: self.tool_id.to_string(),
                                version: version.to_string(),
                                reason: format!("failed to extract {dest_name}: {e}"),
                            })?;
                        std::fs::write(&dest, &contents)?;
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::PermissionsExt;
                            std::fs::set_permissions(
                                &dest,
                                std::fs::Permissions::from_mode(0o755),
                            )?;
                        }
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn extract_zip(
        &self,
        data: &[u8],
        files: &[crate::tool_defs::schema::FileMapping],
        version: &str,
        os: &str,
        arch: &str,
        bin_dir: &Path,
    ) -> Result<(), ToolError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            reason: format!("failed to open zip: {e}"),
        })?;

        let file_mappings: Vec<(String, String)> = if files.is_empty() {
            vec![]
        } else {
            files
                .iter()
                .map(|f| {
                    let src = f
                        .src
                        .as_ref()
                        .map(|s| expand(s, version, os, arch))
                        .unwrap_or_else(|| f.name.clone());
                    (src, f.name.clone())
                })
                .collect()
        };

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: format!("failed to read zip entry: {e}"),
            })?;

            if file.is_dir() {
                continue;
            }

            let path = file.enclosed_name().map(|p| p.to_path_buf());
            let path = match path {
                Some(p) => p,
                None => continue,
            };
            let path_str = path.to_string_lossy();

            let extract = if file_mappings.is_empty() {
                path.file_name().map(|n| {
                    (
                        n.to_string_lossy().to_string(),
                        n.to_string_lossy().to_string(),
                    )
                })
            } else {
                file_mappings.iter().find_map(|(src, dest_name)| {
                    if path_str.as_ref() == src
                        || path_str.ends_with(&format!("/{src}"))
                        || path
                            .file_name()
                            .map(|n| n.to_string_lossy().as_ref() == src.as_str())
                            == Some(true)
                    {
                        Some((src.clone(), dest_name.clone()))
                    } else {
                        None
                    }
                })
            };

            if let Some((_, dest_name)) = extract {
                let dest = bin_dir.join(&dest_name);
                let mut contents = Vec::new();
                file.read_to_end(&mut contents)
                    .map_err(|e| ToolError::InstallFailed {
                        tool: self.tool_id.to_string(),
                        version: version.to_string(),
                        reason: format!("failed to extract {dest_name}: {e}"),
                    })?;
                std::fs::write(&dest, &contents)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&dest, std::fs::Permissions::from_mode(0o755))?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl ToolProvider for GenericProvider {
    fn id(&self) -> &'static str {
        self.tool_id
    }

    fn name(&self) -> &'static str {
        self.tool_name
    }

    fn binary_name(&self) -> &'static str {
        self.binary
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        if which::which(self.binary).is_err() {
            return Ok(None);
        }

        let args = &self.definition.version_detect.args;
        let output = tokio::process::Command::new(self.binary)
            .args(args)
            .output()
            .await
            .map_err(|e| {
                ToolError::DetectionFailed(format!(
                    "failed to run {} {}: {e}",
                    self.binary,
                    args.join(" ")
                ))
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let re = Regex::new(&self.definition.version_detect.regex)
            .map_err(|e| ToolError::DetectionFailed(format!("invalid version regex: {e}")))?;

        // Try stdout first
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(caps) = re.captures(&text) {
            let version = caps[1].to_string();
            debug!(tool = self.tool_id, version = %version, "detected version");
            return Ok(Some(version));
        }

        // Fall back to stderr
        let text = String::from_utf8_lossy(&output.stderr);
        if let Some(caps) = re.captures(&text) {
            let version = caps[1].to_string();
            debug!(tool = self.tool_id, version = %version, "detected version from stderr");
            return Ok(Some(version));
        }

        Ok(None)
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        let install_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(format!(".canaveral/tools/{}/{version}", self.tool_id));
        self.install_to_cache(version, &install_dir).await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let bin_dir = self.download_and_extract(version, cache_dir).await?;

        Ok(InstallResult {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            install_path: bin_dir,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let parts: Vec<&str> = self.definition.repo.split('/').collect();
        if parts.len() != 2 {
            return Ok(vec![]);
        }
        let (owner, repo) = (parts[0], parts[1]);

        let url = format!("https://api.github.com/repos/{owner}/{repo}/releases?per_page=100");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .header("Accept", "application/vnd.github.v3+json")
            .send()
            .await
            .map_err(|e| ToolError::Other(anyhow::anyhow!("failed to fetch releases: {e}")))?;

        if !response.status().is_success() {
            warn!(
                tool = self.tool_id,
                status = response.status().as_u16(),
                "failed to list releases"
            );
            return Ok(vec![]);
        }

        #[derive(serde::Deserialize)]
        struct Release {
            tag_name: String,
        }

        let releases: Vec<Release> = response
            .json()
            .await
            .map_err(|e| ToolError::Other(anyhow::anyhow!("failed to parse releases: {e}")))?;

        let prefix = &self.definition.tag_prefix;
        Ok(releases
            .into_iter()
            .map(|r| r.tag_name.trim_start_matches(prefix.as_str()).to_string())
            .collect())
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            install_path.display().to_string()
        } else {
            format!("{}:{path}", install_path.display())
        };
        vec![("PATH".into(), new_path)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_defs::schema::{FileMapping, PlatformMapping, VersionDetect};
    use std::collections::HashMap;

    fn test_definition() -> ToolDefinition {
        let mut platforms = HashMap::new();
        platforms.insert(
            "darwin-aarch64".to_string(),
            PlatformMapping {
                os: "apple-darwin".to_string(),
                arch: "aarch64".to_string(),
            },
        );
        platforms.insert(
            "darwin-x86_64".to_string(),
            PlatformMapping {
                os: "apple-darwin".to_string(),
                arch: "x86_64".to_string(),
            },
        );
        platforms.insert(
            "linux-x86_64".to_string(),
            PlatformMapping {
                os: "unknown-linux-musl".to_string(),
                arch: "x86_64".to_string(),
            },
        );
        platforms.insert(
            "linux-aarch64".to_string(),
            PlatformMapping {
                os: "unknown-linux-gnu".to_string(),
                arch: "aarch64".to_string(),
            },
        );

        ToolDefinition {
            id: "ripgrep".to_string(),
            name: "ripgrep".to_string(),
            binary: "rg".to_string(),
            repo: "BurntSushi/ripgrep".to_string(),
            tag_prefix: "v".to_string(),
            aliases: vec!["rg".to_string()],
            version_detect: VersionDetect::default(),
            asset: Some("ripgrep-{version}-{arch}-{os}.tar.gz".to_string()),
            url: None,
            format: "tar.gz".to_string(),
            files: vec![FileMapping {
                name: "rg".to_string(),
                src: Some("ripgrep-{version}-{arch}-{os}/rg".to_string()),
            }],
            platforms,
            platform_overrides: HashMap::new(),
        }
    }

    #[test]
    fn test_generic_provider_id_and_name() {
        let def = test_definition();
        let provider = GenericProvider::new(def);
        assert_eq!(provider.id(), "ripgrep");
        assert_eq!(provider.name(), "ripgrep");
        assert_eq!(provider.binary_name(), "rg");
    }

    #[test]
    fn test_build_download_url() {
        let def = test_definition();
        let provider = GenericProvider::new(def);
        let url = provider.build_download_url("14.1.1").unwrap();

        let platform_key = current_platform_key();
        let pm = &provider.definition.platforms[platform_key];
        let expected = format!(
            "https://github.com/BurntSushi/ripgrep/releases/download/v14.1.1/ripgrep-14.1.1-{}-{}.tar.gz",
            pm.arch, pm.os
        );
        assert_eq!(url, expected);
    }

    #[test]
    fn test_from_repo_defaults() {
        let provider = GenericProvider::from_repo("mytool", "owner/mytool");
        assert_eq!(provider.id(), "mytool");
        assert_eq!(provider.name(), "mytool");
        assert_eq!(provider.binary_name(), "mytool");
        assert_eq!(provider.definition.repo, "owner/mytool");
        assert_eq!(provider.definition.tag_prefix, "v");
        assert_eq!(provider.definition.format, "tar.gz");
        assert!(provider.definition.aliases.is_empty());
        assert!(provider.definition.platforms.is_empty());
    }

    #[test]
    fn test_env_vars_prepends_path() {
        let def = test_definition();
        let provider = GenericProvider::new(def);
        let bin_dir = Path::new("/home/user/.canaveral/tools/ripgrep/14.1.1/bin");
        let vars = provider.env_vars(bin_dir);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].0, "PATH");
        assert!(vars[0]
            .1
            .starts_with("/home/user/.canaveral/tools/ripgrep/14.1.1/bin"));
    }

    #[test]
    fn test_build_download_url_with_direct_url() {
        let mut def = test_definition();
        def.url = Some("https://example.com/{version}/{os}/{arch}/tool.tar.gz".to_string());
        let provider = GenericProvider::new(def);
        let url = provider.build_download_url("2.0.0").unwrap();

        let platform_key = current_platform_key();
        let pm = &provider.definition.platforms[platform_key];
        let expected = format!(
            "https://example.com/2.0.0/{}/{}/tool.tar.gz",
            pm.os, pm.arch
        );
        assert_eq!(url, expected);
    }
}
