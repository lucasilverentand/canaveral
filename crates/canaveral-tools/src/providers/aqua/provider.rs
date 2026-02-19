//! AquaProvider — ToolProvider backed by aqua-registry package definitions

use std::io::Read as IoRead;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::providers::aqua::platform::{apply_overrides, current_platform, is_supported};
use crate::providers::aqua::schema::{AquaPackage, AquaRegistryFile};
use crate::providers::aqua::template::{apply_replacements, expand, TemplateVars};
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

/// A tool provider backed by the aqua-registry.
///
/// Fetches package definitions from the aqua-registry on GitHub, caches them
/// locally, and uses them to download/extract pre-built binaries.
pub struct AquaProvider {
    /// Tool identifier (e.g. "ripgrep", "jq")
    tool_id: &'static str,
    /// GitHub owner/repo (e.g. "BurntSushi/ripgrep")
    owner_repo: String,
    /// Primary binary name (e.g. "rg")
    binary: &'static str,
    /// Local cache directory for registry YAML files
    registry_cache_dir: PathBuf,
    /// HTTP client
    client: reqwest::Client,
}

impl AquaProvider {
    /// Create an AquaProvider from a shortname lookup.
    ///
    /// Returns `None` if the tool name is not in the shortnames index.
    pub fn from_shortname(name: &str) -> Option<Self> {
        let shortnames = super::shortnames();
        let owner_repo = shortnames.get(name)?;

        let registry_cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/.registry");

        // The binary name defaults to the tool name.
        // For tools where the binary differs (e.g. ripgrep -> rg),
        // the aqua registry files list tells us the actual binary name,
        // but we need a static name for the trait. We'll use the tool name
        // as default and override from the registry at runtime.
        let tool_id: &'static str = Box::leak(name.to_string().into_boxed_str());
        let binary: &'static str = Box::leak(name.to_string().into_boxed_str());

        Some(Self {
            tool_id,
            owner_repo: owner_repo.clone(),
            binary,
            registry_cache_dir,
            client: reqwest::Client::new(),
        })
    }

    /// Create an AquaProvider from an explicit `owner/repo` string.
    pub fn from_source(name: &str, owner_repo: &str) -> Self {
        let registry_cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/.registry");

        let tool_id: &'static str = Box::leak(name.to_string().into_boxed_str());
        let binary: &'static str = Box::leak(name.to_string().into_boxed_str());

        Self {
            tool_id,
            owner_repo: owner_repo.to_string(),
            binary,
            registry_cache_dir,
            client: reqwest::Client::new(),
        }
    }

    /// Fetch the registry YAML for this tool's owner/repo.
    /// Uses a 24-hour file-based cache.
    async fn fetch_registry(&self) -> Result<AquaRegistryFile, ToolError> {
        let parts: Vec<&str> = self.owner_repo.split('/').collect();
        if parts.len() != 2 {
            return Err(ToolError::Other(anyhow::anyhow!(
                "invalid owner/repo: {}",
                self.owner_repo
            )));
        }
        let (owner, repo) = (parts[0], parts[1]);

        let cache_path = self.registry_cache_dir.join(format!("{owner}/{repo}.yaml"));

        // Check cache freshness (24h TTL)
        if let Ok(metadata) = std::fs::metadata(&cache_path) {
            if let Ok(modified) = metadata.modified() {
                let age = std::time::SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_default();
                if age < std::time::Duration::from_secs(24 * 60 * 60) {
                    debug!(tool = self.tool_id, "using cached registry YAML");
                    let contents = std::fs::read_to_string(&cache_path)?;
                    return serde_yaml::from_str(&contents).map_err(|e| {
                        ToolError::Other(anyhow::anyhow!("failed to parse cached registry: {e}"))
                    });
                }
            }
        }

        // Fetch from GitHub
        let url = format!(
            "https://raw.githubusercontent.com/aquaproj/aqua-registry/main/pkgs/{owner}/{repo}/registry.yaml"
        );
        debug!(tool = self.tool_id, url = %url, "fetching registry YAML");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: String::new(),
                reason: format!("failed to fetch registry: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: String::new(),
                reason: format!(
                    "registry fetch returned HTTP {}",
                    response.status().as_u16()
                ),
            });
        }

        let body = response
            .text()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: String::new(),
                reason: format!("failed to read registry response: {e}"),
            })?;

        // Cache the result
        if let Some(parent) = cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&cache_path, &body)?;

        serde_yaml::from_str(&body)
            .map_err(|e| ToolError::Other(anyhow::anyhow!("failed to parse registry YAML: {e}")))
    }

    /// Get the effective package definition for a given version,
    /// applying version overrides and platform overrides.
    fn resolve_package(&self, pkg: &AquaPackage, version: &str) -> AquaPackage {
        let mut resolved = pkg.clone();

        // Apply version overrides (first match wins)
        for vo in &pkg.version_overrides {
            if version_constraint_matches(&vo.version_constraint, version) {
                if let Some(ref asset) = vo.asset {
                    resolved.asset = Some(asset.clone());
                }
                if let Some(ref format) = vo.format {
                    resolved.format = Some(format.clone());
                }
                if let Some(ref files) = vo.files {
                    resolved.files = files.clone();
                }
                if let Some(ref replacements) = vo.replacements {
                    resolved.replacements = replacements.clone();
                }
                if let Some(ref supported_envs) = vo.supported_envs {
                    resolved.supported_envs = supported_envs.clone();
                }
                if let Some(ref overrides) = vo.overrides {
                    resolved.overrides = overrides.clone();
                }
                if let Some(ref version_prefix) = vo.version_prefix {
                    resolved.version_prefix = Some(version_prefix.clone());
                }
                if let Some(ref checksum) = vo.checksum {
                    resolved.checksum = Some(checksum.clone());
                }
                if let Some(ref url) = vo.url {
                    resolved.url = Some(url.clone());
                }
                if let Some(ref pkg_type) = vo.pkg_type {
                    resolved.pkg_type = pkg_type.clone();
                }
                break;
            }
        }

        // Apply platform overrides
        apply_overrides(&resolved)
    }

    /// Build the download URL for a given version
    fn build_download_url(&self, pkg: &AquaPackage, version: &str) -> Result<String, ToolError> {
        let (os, arch) = current_platform();
        let (os, arch) = apply_replacements(os, arch, &pkg.replacements);

        let format_str = pkg.format.as_deref().unwrap_or("tar.gz");

        let vars = TemplateVars {
            version: version.to_string(),
            os: os.clone(),
            arch: arch.clone(),
            format: format_str.to_string(),
        };

        if pkg.pkg_type == "http" {
            if let Some(ref url_template) = pkg.url {
                return Ok(expand(url_template, &vars));
            }
        }

        // github_release type
        let owner = pkg.repo_owner.as_deref().unwrap_or("");
        let repo = pkg.repo_name.as_deref().unwrap_or("");
        let version_prefix = pkg.version_prefix.as_deref().unwrap_or("v");
        let tag = format!("{version_prefix}{version}");

        let asset = pkg.asset.as_ref().ok_or_else(|| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            reason: "no asset template in registry".to_string(),
        })?;

        let asset_name = expand(asset, &vars);
        Ok(format!(
            "https://github.com/{owner}/{repo}/releases/download/{tag}/{asset_name}"
        ))
    }

    /// Download and extract a tool binary
    async fn download_and_extract(
        &self,
        pkg: &AquaPackage,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = self.build_download_url(pkg, version)?;
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

        let format = pkg.format.as_deref().unwrap_or("tar.gz");
        let (os, arch) = current_platform();
        let (os, arch) = apply_replacements(os, arch, &pkg.replacements);
        let vars = TemplateVars {
            version: version.to_string(),
            os,
            arch,
            format: format.to_string(),
        };

        match format {
            "tar.gz" | "tgz" => {
                let decoder = flate2::read::GzDecoder::new(&bytes[..]);
                let mut archive = tar::Archive::new(decoder);
                self.extract_tar(&mut archive, pkg, &vars, &bin_dir)?;
            }
            "zip" => {
                self.extract_zip(&bytes, pkg, &vars, &bin_dir)?;
            }
            "raw" | "" => {
                // Single binary, no archive
                let binary_name = if !pkg.files.is_empty() {
                    pkg.files[0].name.clone()
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
        pkg: &AquaPackage,
        vars: &TemplateVars,
        bin_dir: &Path,
    ) -> Result<(), ToolError> {
        let entries = archive.entries().map_err(|e| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: vars.version.clone(),
            reason: format!("failed to read archive: {e}"),
        })?;

        // Build a map of source paths to destination names
        let file_mappings: Vec<(String, String)> = if pkg.files.is_empty() {
            // No file mappings — extract everything executable
            vec![]
        } else {
            pkg.files
                .iter()
                .map(|f| {
                    let src = f
                        .src
                        .as_ref()
                        .map(|s| expand(s, vars))
                        .unwrap_or_else(|| f.name.clone());
                    (src, f.name.clone())
                })
                .collect()
        };

        for entry_result in entries {
            let mut entry = entry_result.map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: vars.version.clone(),
                reason: format!("failed to read archive entry: {e}"),
            })?;

            let path = entry
                .path()
                .map_err(|e| ToolError::InstallFailed {
                    tool: self.tool_id.to_string(),
                    version: vars.version.clone(),
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
                                version: vars.version.clone(),
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
                                version: vars.version.clone(),
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
        pkg: &AquaPackage,
        vars: &TemplateVars,
        bin_dir: &Path,
    ) -> Result<(), ToolError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).map_err(|e| ToolError::InstallFailed {
            tool: self.tool_id.to_string(),
            version: vars.version.clone(),
            reason: format!("failed to open zip: {e}"),
        })?;

        let file_mappings: Vec<(String, String)> = if pkg.files.is_empty() {
            vec![]
        } else {
            pkg.files
                .iter()
                .map(|f| {
                    let src = f
                        .src
                        .as_ref()
                        .map(|s| expand(s, vars))
                        .unwrap_or_else(|| f.name.clone());
                    (src, f.name.clone())
                })
                .collect()
        };

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: vars.version.clone(),
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
                path.file_name()
                    .map(|n| {
                        Some((
                            n.to_string_lossy().to_string(),
                            n.to_string_lossy().to_string(),
                        ))
                    })
                    .unwrap_or(None)
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
                        version: vars.version.clone(),
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
impl ToolProvider for AquaProvider {
    fn id(&self) -> &'static str {
        self.tool_id
    }

    fn name(&self) -> &'static str {
        self.tool_id
    }

    fn binary_name(&self) -> &'static str {
        self.binary
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        if which::which(self.binary).is_err() {
            return Ok(None);
        }

        let output = tokio::process::Command::new(self.binary)
            .arg("--version")
            .output()
            .await
            .map_err(|e| {
                ToolError::DetectionFailed(format!("failed to run {} --version: {e}", self.binary))
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let text = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(r"(\d+\.\d+\.\d+)").unwrap();
        if let Some(caps) = re.captures(&text) {
            let version = caps[1].to_string();
            debug!(tool = self.tool_id, version = %version, "detected version");
            return Ok(Some(version));
        }

        // Try stderr too (some tools output version there)
        let text = String::from_utf8_lossy(&output.stderr);
        if let Some(caps) = re.captures(&text) {
            return Ok(Some(caps[1].to_string()));
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
        let registry = self.fetch_registry().await?;

        let pkg = registry
            .packages
            .first()
            .ok_or_else(|| ToolError::InstallFailed {
                tool: self.tool_id.to_string(),
                version: version.to_string(),
                reason: "no packages in registry file".to_string(),
            })?;

        let resolved = self.resolve_package(pkg, version);

        if !is_supported(&resolved.supported_envs) {
            return Err(ToolError::UnsupportedPlatform(self.tool_id.to_string()));
        }

        let bin_dir = self
            .download_and_extract(&resolved, version, cache_dir)
            .await?;

        Ok(InstallResult {
            tool: self.tool_id.to_string(),
            version: version.to_string(),
            install_path: bin_dir,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let parts: Vec<&str> = self.owner_repo.split('/').collect();
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

        Ok(releases
            .into_iter()
            .map(|r| r.tag_name.trim_start_matches('v').to_string())
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

/// Check if a version matches an aqua version constraint.
///
/// Supports:
/// - `"true"` — always matches
/// - `semver(EXPR)` — semver range expression
/// - Literal version string — exact match
fn version_constraint_matches(constraint: &str, version: &str) -> bool {
    let constraint = constraint.trim();

    if constraint == "true" {
        return true;
    }

    // semver(...) expression
    if let Some(inner) = constraint
        .strip_prefix("semver(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let inner = inner.trim();
        // Try to parse with the semver crate
        if let Ok(req) = semver::VersionReq::parse(inner) {
            // Ensure we have a valid semver version (pad if needed)
            let padded = pad_version(version);
            if let Ok(ver) = semver::Version::parse(&padded) {
                return req.matches(&ver);
            }
        }
        return false;
    }

    // Literal match
    version == constraint
}

/// Pad a version string to a valid semver (e.g. "14" -> "14.0.0", "14.1" -> "14.1.0")
fn pad_version(version: &str) -> String {
    let version = version.trim_start_matches('v');
    let parts: Vec<&str> = version.split('.').collect();
    match parts.len() {
        1 => format!("{}.0.0", parts[0]),
        2 => format!("{}.{}.0", parts[0], parts[1]),
        _ => version.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constraint_true() {
        assert!(version_constraint_matches("true", "1.0.0"));
        assert!(version_constraint_matches("true", "99.99.99"));
    }

    #[test]
    fn version_constraint_semver() {
        assert!(version_constraint_matches("semver(< 8.0.0)", "7.5.0"));
        assert!(!version_constraint_matches("semver(< 8.0.0)", "8.0.0"));
        assert!(version_constraint_matches("semver(>= 1.0.0)", "1.0.0"));
        assert!(version_constraint_matches("semver(>= 1.0.0)", "2.0.0"));
    }

    #[test]
    fn version_constraint_literal() {
        assert!(version_constraint_matches("1.0.0", "1.0.0"));
        assert!(!version_constraint_matches("1.0.0", "1.0.1"));
    }

    #[test]
    fn pad_version_works() {
        assert_eq!(pad_version("14"), "14.0.0");
        assert_eq!(pad_version("14.1"), "14.1.0");
        assert_eq!(pad_version("14.1.2"), "14.1.2");
        assert_eq!(pad_version("v14.1"), "14.1.0");
    }

    #[test]
    fn from_shortname_known_tool() {
        let provider = AquaProvider::from_shortname("ripgrep");
        assert!(provider.is_some());
        let p = provider.unwrap();
        assert_eq!(p.owner_repo, "BurntSushi/ripgrep");
    }

    #[test]
    fn from_shortname_unknown_tool() {
        assert!(AquaProvider::from_shortname("nonexistent-tool-xyz").is_none());
    }
}
