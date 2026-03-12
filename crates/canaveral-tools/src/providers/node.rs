//! Node.js and npm tool providers
//!
//! Downloads and installs pre-built Node.js binaries directly from
//! <https://nodejs.org/dist/>.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// Node.js version index types
// ---------------------------------------------------------------------------

/// A single entry from <https://nodejs.org/dist/index.json>.
#[derive(Debug, Clone, Deserialize)]
struct NodeVersionEntry {
    /// Version string including the `v` prefix, e.g. `"v22.14.0"`.
    version: String,
    /// LTS codename (e.g. `"Jod"`) or `false` when it is a current release.
    lts: LtsField,
}

/// The `lts` field is either a string (LTS codename) or `false`.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
#[allow(dead_code)] // Inner values are needed for serde deserialization
enum LtsField {
    Name(String),
    NotLts(bool),
}

impl LtsField {
    fn is_lts(&self) -> bool {
        matches!(self, LtsField::Name(_))
    }
}

/// Cached version index with a TTL.
struct CachedIndex {
    entries: Vec<NodeVersionEntry>,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the Node.js platform identifier for the current OS.
fn node_os() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") {
        Ok("darwin")
    } else if cfg!(target_os = "linux") {
        Ok("linux")
    } else if cfg!(target_os = "windows") {
        Ok("win")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "node: unsupported operating system".to_string(),
        ))
    }
}

/// Returns the Node.js architecture identifier for the current arch.
fn node_arch() -> Result<&'static str, ToolError> {
    if cfg!(target_arch = "aarch64") {
        Ok("arm64")
    } else if cfg!(target_arch = "x86_64") {
        Ok("x64")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "node: unsupported CPU architecture".to_string(),
        ))
    }
}

/// Whether the current platform uses a zip archive (Windows) vs tarball.
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

// ---------------------------------------------------------------------------
// NodeProvider
// ---------------------------------------------------------------------------

/// Provider for Node.js — downloads pre-built binaries from nodejs.org.
pub struct NodeProvider {
    client: reqwest::Client,
    /// In-memory cache for the version index (24-hour TTL).
    index_cache: tokio::sync::Mutex<Option<CachedIndex>>,
}

impl NodeProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            index_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/node/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/node")
    }

    // -- version index ------------------------------------------------------

    /// Fetch the Node.js version index, using an in-memory 24-hour cache.
    async fn fetch_index(&self) -> Result<Vec<NodeVersionEntry>, ToolError> {
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);

        {
            let guard = self.index_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    debug!("using cached Node.js version index");
                    return Ok(cached.entries.clone());
                }
            }
        }

        info!("fetching Node.js version index from nodejs.org");
        let response = self
            .client
            .get("https://nodejs.org/dist/index.json")
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "node".into(),
                reason: format!("failed to fetch version index: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "node".into(),
                reason: format!("version index returned HTTP {}", response.status().as_u16()),
            });
        }

        let entries: Vec<NodeVersionEntry> =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "node".into(),
                    reason: format!("failed to parse version index: {e}"),
                })?;

        // Store in cache
        {
            let mut guard = self.index_cache.lock().await;
            *guard = Some(CachedIndex {
                entries: entries.clone(),
                fetched_at: std::time::Instant::now(),
            });
        }

        Ok(entries)
    }

    /// Resolve a (possibly partial) version prefix to the latest matching
    /// full version string.
    ///
    /// The index is sorted newest-first, so the first match is the latest.
    /// For example, `"22"` resolves to the newest `22.x.y`.
    async fn resolve_version(&self, requested: &str) -> Result<String, ToolError> {
        let requested = requested.trim_start_matches('v');
        let entries = self.fetch_index().await?;

        for entry in &entries {
            let v = entry.version.trim_start_matches('v');
            if version_satisfies(v, requested) {
                debug!(requested = %requested, resolved = %v, "resolved Node.js version");
                return Ok(v.to_string());
            }
        }

        Err(ToolError::VersionNotAvailable {
            tool: "node".into(),
            version: requested.to_string(),
        })
    }

    // -- download & extract -------------------------------------------------

    /// Build the download URL for a fully resolved version.
    fn download_url(version: &str) -> Result<String, ToolError> {
        let os = node_os()?;
        let arch = node_arch()?;
        let ext = if is_windows() { "zip" } else { "tar.gz" };
        Ok(format!(
            "https://nodejs.org/dist/v{version}/node-v{version}-{os}-{arch}.{ext}"
        ))
    }

    /// Download and extract Node.js into `dest_dir`.
    ///
    /// Returns the path to the `bin/` directory inside the extracted tree.
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = Self::download_url(version)?;
        info!(version = %version, url = %url, "downloading Node.js");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        if is_windows() {
            Self::extract_zip(&bytes, version, dest_dir)?;
        } else {
            Self::extract_tarball(&bytes, version, dest_dir)?;
        }

        // The archive extracts to node-v{version}-{os}-{arch}/ inside dest_dir.
        let os = node_os()?;
        let arch = node_arch()?;
        let extracted_dir = dest_dir.join(format!("node-v{version}-{os}-{arch}"));
        let bin_dir = extracted_dir.join("bin");

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(bin_dir = %bin_dir.display(), "Node.js extracted successfully");
        Ok(bin_dir)
    }

    /// Extract a `.tar.gz` archive into `dest_dir`.
    fn extract_tarball(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let decoder = flate2::read::GzDecoder::new(data);
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("failed to extract tarball: {e}"),
            })?;

        Ok(())
    }

    /// Extract a `.zip` archive into `dest_dir`.
    fn extract_zip(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| ToolError::ExtractionFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "node".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        Ok(())
    }
}

impl Default for NodeProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for NodeProvider {
    fn id(&self) -> &'static str {
        "node"
    }

    fn name(&self) -> &'static str {
        "Node.js"
    }

    fn binary_name(&self) -> &'static str {
        "node"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        // Try PATH first
        let output = tokio::process::Command::new("node")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout)
                    .trim()
                    .trim_start_matches('v')
                    .to_string();
                debug!(version = %version, "detected Node.js on PATH");
                return Ok(Some(version));
            }
            Ok(_) => {
                debug!("node --version returned non-zero exit status");
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("node not found on PATH");
            }
            Err(e) => {
                return Err(ToolError::DetectionFailed(format!(
                    "failed to run `node --version`: {e}"
                )));
            }
        }

        // Check the default cache location for any installed version
        let cache_root = Self::default_cache_root();
        if cache_root.exists() {
            if let Ok(entries) = std::fs::read_dir(&cache_root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }
                    if let Some(bin_path) = find_node_binary_in_cache(&path) {
                        let output = tokio::process::Command::new(&bin_path)
                            .arg("--version")
                            .output()
                            .await;
                        if let Ok(out) = output {
                            if out.status.success() {
                                let version = String::from_utf8_lossy(&out.stdout)
                                    .trim()
                                    .trim_start_matches('v')
                                    .to_string();
                                debug!(version = %version, path = %bin_path.display(), "detected Node.js in cache");
                                return Ok(Some(version));
                            }
                        }
                    }
                }
            }
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
        let resolved = self.resolve_version(version).await?;
        let cache_dir = Self::default_cache_root().join(&resolved);
        self.install_to_cache(&resolved, &cache_dir).await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let resolved = self.resolve_version(version).await?;

        // Check if already extracted
        let os = node_os()?;
        let arch = node_arch()?;
        let expected_bin = cache_dir
            .join(format!("node-v{resolved}-{os}-{arch}"))
            .join("bin");
        if expected_bin.exists() {
            info!(version = %resolved, "Node.js already installed in cache");
            return Ok(InstallResult {
                tool: "node".into(),
                version: resolved,
                install_path: expected_bin,
            });
        }

        let bin_dir = self.download_and_extract(&resolved, cache_dir).await?;

        info!(version = %resolved, path = %bin_dir.display(), "Node.js installed");
        Ok(InstallResult {
            tool: "node".into(),
            version: resolved,
            install_path: bin_dir,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let entries = self.fetch_index().await?;

        // Only return LTS + current versions to keep the list manageable.
        // Current versions are the latest major line that hasn't entered LTS
        // yet. The index is sorted newest-first and current versions appear
        // before the first LTS entry.
        let mut seen_lts = false;
        let current_major: Option<u32> = entries.iter().find_map(|e| {
            if !e.lts.is_lts() {
                e.version
                    .trim_start_matches('v')
                    .split('.')
                    .next()
                    .and_then(|s| s.parse().ok())
            } else {
                None
            }
        });

        let versions: Vec<String> = entries
            .into_iter()
            .filter(|e| {
                if e.lts.is_lts() {
                    seen_lts = true;
                    return true;
                }
                // Include current (non-LTS) versions from the active major line
                if !seen_lts {
                    if let Some(cm) = current_major {
                        let major: Option<u32> = e
                            .version
                            .trim_start_matches('v')
                            .split('.')
                            .next()
                            .and_then(|s| s.parse().ok());
                        return major == Some(cm);
                    }
                }
                false
            })
            .map(|e| e.version.trim_start_matches('v').to_string())
            .collect();

        Ok(versions)
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

/// Search for the `node` binary inside a cache version directory.
///
/// The directory structure is:
/// `{version_dir}/node-v{version}-{os}-{arch}/bin/node`
fn find_node_binary_in_cache(version_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(version_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name()?.to_string_lossy();
            if name.starts_with("node-v") {
                let bin = if cfg!(target_os = "windows") {
                    path.join("node.exe")
                } else {
                    path.join("bin").join("node")
                };
                if bin.exists() {
                    return Some(bin);
                }
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// NpmProvider
// ---------------------------------------------------------------------------

/// Provider for npm — bundled with Node.js, not independently installable.
pub struct NpmProvider;

impl NpmProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NpmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for NpmProvider {
    fn id(&self) -> &'static str {
        "npm"
    }

    fn name(&self) -> &'static str {
        "npm"
    }

    fn binary_name(&self) -> &'static str {
        "npm"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let output = tokio::process::Command::new("npm")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                // npm --version outputs e.g. "10.2.0" — no v prefix
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                debug!(version = %version, "detected npm version");
                Ok(Some(version))
            }
            Ok(_) => {
                warn!("npm --version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("npm not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `npm --version`: {e}"
            ))),
        }
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        Err(ToolError::InstallFailed {
            tool: "npm".into(),
            version: version.into(),
            reason: "npm is bundled with Node.js — install Node.js via \
                     `canaveral tools install node`"
                .to_string(),
        })
    }

    async fn install_to_cache(
        &self,
        version: &str,
        _cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        self.install(version).await
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        Ok(Vec::new())
    }

    fn env_vars(&self, _install_path: &Path) -> Vec<(String, String)> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- NodeProvider basic properties --------------------------------------

    #[test]
    fn node_provider_id_and_name() {
        let provider = NodeProvider::new();
        assert_eq!(provider.id(), "node");
        assert_eq!(provider.name(), "Node.js");
        assert_eq!(provider.binary_name(), "node");
    }

    #[test]
    fn npm_provider_id_and_name() {
        let provider = NpmProvider::new();
        assert_eq!(provider.id(), "npm");
        assert_eq!(provider.name(), "npm");
        assert_eq!(provider.binary_name(), "npm");
    }

    // -- version stripping --------------------------------------------------

    #[test]
    fn node_strips_v_prefix() {
        let raw = "v22.1.0";
        let stripped = raw.trim_start_matches('v');
        assert_eq!(stripped, "22.1.0");
    }

    #[test]
    fn npm_no_v_prefix_needed() {
        let raw = "10.2.0";
        assert_eq!(raw, "10.2.0");
    }

    // -- version matching ---------------------------------------------------

    #[test]
    fn node_version_matching() {
        assert!(version_satisfies("22.1.0", "22"));
        assert!(version_satisfies("22.1.0", "22.1"));
        assert!(version_satisfies("22.1.0", "22.1.0"));
        assert!(!version_satisfies("22.1.0", "20"));
        assert!(!version_satisfies("22.1.0", "22.2"));
    }

    #[test]
    fn npm_version_matching() {
        assert!(version_satisfies("10.2.0", "10"));
        assert!(version_satisfies("10.2.0", "10.2"));
        assert!(version_satisfies("10.2.0", "10.2.0"));
        assert!(!version_satisfies("10.2.0", "9"));
    }

    // -- platform helpers ---------------------------------------------------

    #[test]
    fn node_os_returns_known_value() {
        let os = node_os().unwrap();
        assert!(
            ["darwin", "linux", "win"].contains(&os),
            "unexpected OS: {os}"
        );
    }

    #[test]
    fn node_arch_returns_known_value() {
        let arch = node_arch().unwrap();
        assert!(["arm64", "x64"].contains(&arch), "unexpected arch: {arch}");
    }

    // -- download URL construction ------------------------------------------

    #[test]
    fn download_url_format() {
        let url = NodeProvider::download_url("22.14.0").unwrap();
        let os = node_os().unwrap();
        let arch = node_arch().unwrap();
        let ext = if is_windows() { "zip" } else { "tar.gz" };
        assert_eq!(
            url,
            format!("https://nodejs.org/dist/v22.14.0/node-v22.14.0-{os}-{arch}.{ext}")
        );
    }

    // -- LtsField parsing ---------------------------------------------------

    #[test]
    fn lts_field_string_is_lts() {
        let field: LtsField = serde_json::from_str(r#""Jod""#).unwrap();
        assert!(field.is_lts());
    }

    #[test]
    fn lts_field_false_is_not_lts() {
        let field: LtsField = serde_json::from_str("false").unwrap();
        assert!(!field.is_lts());
    }

    // -- version index parsing ----------------------------------------------

    #[test]
    fn parse_version_index_entry() {
        let json = r#"[
            {"version": "v22.14.0", "lts": "Jod"},
            {"version": "v23.7.0", "lts": false}
        ]"#;
        let entries: Vec<NodeVersionEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].version, "v22.14.0");
        assert!(entries[0].lts.is_lts());
        assert_eq!(entries[1].version, "v23.7.0");
        assert!(!entries[1].lts.is_lts());
    }

    // -- version resolution (unit, no network) ------------------------------

    #[test]
    fn resolve_version_prefix_matching() {
        // Simulate what resolve_version does: walk the index (newest first)
        // and find the first entry whose version satisfies the prefix.
        let entries = vec![
            NodeVersionEntry {
                version: "v23.7.0".into(),
                lts: LtsField::NotLts(false),
            },
            NodeVersionEntry {
                version: "v22.14.0".into(),
                lts: LtsField::Name("Jod".into()),
            },
            NodeVersionEntry {
                version: "v22.13.1".into(),
                lts: LtsField::Name("Jod".into()),
            },
            NodeVersionEntry {
                version: "v22.13.0".into(),
                lts: LtsField::Name("Jod".into()),
            },
            NodeVersionEntry {
                version: "v20.18.2".into(),
                lts: LtsField::Name("Iron".into()),
            },
        ];

        // "22" should resolve to the first 22.x.y (newest)
        let resolved = entries
            .iter()
            .find(|e| version_satisfies(e.version.trim_start_matches('v'), "22"))
            .map(|e| e.version.trim_start_matches('v').to_string());
        assert_eq!(resolved, Some("22.14.0".to_string()));

        // "22.13" should resolve to 22.13.1 (newest 22.13.x)
        let resolved = entries
            .iter()
            .find(|e| version_satisfies(e.version.trim_start_matches('v'), "22.13"))
            .map(|e| e.version.trim_start_matches('v').to_string());
        assert_eq!(resolved, Some("22.13.1".to_string()));

        // "20" should resolve to 20.18.2
        let resolved = entries
            .iter()
            .find(|e| version_satisfies(e.version.trim_start_matches('v'), "20"))
            .map(|e| e.version.trim_start_matches('v').to_string());
        assert_eq!(resolved, Some("20.18.2".to_string()));

        // "21" should resolve to nothing
        let resolved = entries
            .iter()
            .find(|e| version_satisfies(e.version.trim_start_matches('v'), "21"))
            .map(|e| e.version.trim_start_matches('v').to_string());
        assert_eq!(resolved, None);

        // Exact version match
        let resolved = entries
            .iter()
            .find(|e| version_satisfies(e.version.trim_start_matches('v'), "22.14.0"))
            .map(|e| e.version.trim_start_matches('v').to_string());
        assert_eq!(resolved, Some("22.14.0".to_string()));
    }

    // -- env_vars -----------------------------------------------------------

    #[test]
    fn node_env_vars_prepends_path() {
        let provider = NodeProvider::new();
        let bin = Path::new("/home/user/.canaveral/tools/node/22.14.0/node-v22.14.0-linux-x64/bin");
        let vars = provider.env_vars(bin);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].0, "PATH");
        assert!(vars[0].1.starts_with(&bin.display().to_string()));
    }

    #[test]
    fn npm_env_vars_empty() {
        let provider = NpmProvider::new();
        let vars = provider.env_vars(Path::new("/usr/local/bin"));
        assert!(vars.is_empty());
    }

    // -- NpmProvider install error ------------------------------------------

    #[tokio::test]
    async fn npm_install_returns_helpful_error() {
        let provider = NpmProvider::new();
        let result = provider.install("10").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("bundled with Node.js"),
            "error should mention npm is bundled: {err}"
        );
        assert!(
            err.contains("canaveral tools install node"),
            "error should reference canaveral: {err}"
        );
    }

    #[tokio::test]
    async fn npm_install_to_cache_also_errors() {
        let provider = NpmProvider::new();
        let result = provider
            .install_to_cache("10", Path::new("/tmp/cache"))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn npm_list_available_returns_empty() {
        let provider = NpmProvider::new();
        let result = provider.list_available().await.unwrap();
        assert!(result.is_empty());
    }

    // -- find_node_binary_in_cache ------------------------------------------

    #[test]
    fn find_node_binary_nonexistent_dir() {
        let result = find_node_binary_in_cache(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_none());
    }

    // -- list_available filtering -------------------------------------------

    #[test]
    fn list_available_filters_lts_and_current() {
        let entries = vec![
            NodeVersionEntry {
                version: "v23.7.0".into(),
                lts: LtsField::NotLts(false),
            },
            NodeVersionEntry {
                version: "v23.6.0".into(),
                lts: LtsField::NotLts(false),
            },
            NodeVersionEntry {
                version: "v22.14.0".into(),
                lts: LtsField::Name("Jod".into()),
            },
            NodeVersionEntry {
                version: "v22.13.1".into(),
                lts: LtsField::Name("Jod".into()),
            },
            NodeVersionEntry {
                version: "v20.18.2".into(),
                lts: LtsField::Name("Iron".into()),
            },
            // Old non-LTS that should be excluded (major 21, after LTS starts)
            NodeVersionEntry {
                version: "v21.7.3".into(),
                lts: LtsField::NotLts(false),
            },
        ];

        // Replicate the filter logic from list_available
        let mut seen_lts = false;
        let current_major: Option<u32> = entries.iter().find_map(|e| {
            if !e.lts.is_lts() {
                e.version
                    .trim_start_matches('v')
                    .split('.')
                    .next()
                    .and_then(|s| s.parse().ok())
            } else {
                None
            }
        });

        let versions: Vec<String> = entries
            .into_iter()
            .filter(|e| {
                if e.lts.is_lts() {
                    seen_lts = true;
                    return true;
                }
                if !seen_lts {
                    if let Some(cm) = current_major {
                        let major: Option<u32> = e
                            .version
                            .trim_start_matches('v')
                            .split('.')
                            .next()
                            .and_then(|s| s.parse().ok());
                        return major == Some(cm);
                    }
                }
                false
            })
            .map(|e| e.version.trim_start_matches('v').to_string())
            .collect();

        // Should include current 23.x and all LTS, but not old non-LTS 21.x
        assert!(versions.contains(&"23.7.0".to_string()));
        assert!(versions.contains(&"23.6.0".to_string()));
        assert!(versions.contains(&"22.14.0".to_string()));
        assert!(versions.contains(&"22.13.1".to_string()));
        assert!(versions.contains(&"20.18.2".to_string()));
        assert!(!versions.contains(&"21.7.3".to_string()));
    }
}
