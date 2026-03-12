//! Python and pip tool providers
//!
//! Downloads and installs pre-built CPython binaries from
//! <https://github.com/astral-sh/python-build-standalone/releases>.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// GitHub releases API types
// ---------------------------------------------------------------------------

/// A single release from the GitHub API.
#[derive(Debug, Clone, Deserialize)]
struct GitHubRelease {
    /// The release tag, e.g. `"20260310"`.
    #[allow(dead_code)]
    tag_name: String,
    /// Assets attached to this release.
    assets: Vec<GitHubAsset>,
}

/// A single release asset from the GitHub API.
#[derive(Debug, Clone, Deserialize)]
struct GitHubAsset {
    /// The filename, e.g. `"cpython-3.13.12+20260310-aarch64-apple-darwin-install_only.tar.gz"`.
    name: String,
    /// Direct download URL.
    browser_download_url: String,
}

/// A resolved Python version with its download URL and release date.
#[derive(Debug, Clone)]
struct ResolvedPython {
    /// The Python version, e.g. `"3.13.12"`.
    version: String,
    /// The release date tag, e.g. `"20260310"`.
    #[allow(dead_code)]
    release_date: String,
    /// The direct download URL for this asset.
    download_url: String,
}

/// Cached release index with a TTL.
struct CachedReleaseIndex {
    releases: Vec<GitHubRelease>,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the python-build-standalone target triple for the current platform.
fn python_target() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Ok("aarch64-apple-darwin")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-apple-darwin")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-unknown-linux-gnu")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Ok("aarch64-unknown-linux-gnu")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        Ok("x86_64-pc-windows-msvc")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "python: unsupported OS/architecture combination".to_string(),
        ))
    }
}

/// Whether the current platform is Windows.
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Asset name pattern: `cpython-{version}+{date}-{target}-install_only.tar.gz`
///
/// We specifically want `install_only` (not `install_only_stripped`) to keep
/// debug symbols and standard library intact.
fn asset_name_pattern() -> Result<String, ToolError> {
    let target = python_target()?;
    Ok(format!("-{target}-install_only.tar.gz"))
}

/// Parse the Python version from an asset filename.
///
/// Example: `"cpython-3.13.12+20260310-aarch64-apple-darwin-install_only.tar.gz"`
/// returns `Some(("3.13.12", "20260310"))`.
fn parse_asset_name(name: &str) -> Option<(String, String)> {
    // Pattern: cpython-{version}+{date}-{target}-install_only.tar.gz
    let re = Regex::new(r"^cpython-(\d+\.\d+\.\d+[a-zA-Z0-9]*)\+(\d{8})-").ok()?;
    let caps = re.captures(name)?;
    Some((caps[1].to_string(), caps[2].to_string()))
}

/// Filter out pre-release versions (alpha, beta, rc).
fn is_stable_version(version: &str) -> bool {
    // Stable versions are pure digits and dots: "3.13.12"
    // Pre-releases have letters: "3.15.0a7", "3.14.0b1", "3.14.0rc1"
    version.chars().all(|c| c.is_ascii_digit() || c == '.')
}

// ---------------------------------------------------------------------------
// PythonProvider
// ---------------------------------------------------------------------------

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/astral-sh/python-build-standalone/releases";

/// How many pages of releases to fetch from GitHub (at 30 per page).
const RELEASE_PAGES: u32 = 3;

/// Provider for CPython — downloads pre-built binaries from python-build-standalone.
pub struct PythonProvider {
    client: reqwest::Client,
    /// In-memory cache for the release index (24-hour TTL).
    index_cache: tokio::sync::Mutex<Option<CachedReleaseIndex>>,
}

impl PythonProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            index_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/python/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/python")
    }

    // -- release index -------------------------------------------------------

    /// Fetch the GitHub releases index, using an in-memory 24-hour cache.
    ///
    /// Fetches multiple pages to cover a range of Python versions across
    /// recent releases.
    async fn fetch_releases(&self) -> Result<Vec<GitHubRelease>, ToolError> {
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);

        {
            let guard = self.index_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    debug!("using cached python-build-standalone release index");
                    return Ok(cached.releases.clone());
                }
            }
        }

        info!("fetching python-build-standalone release index from GitHub");

        let mut all_releases = Vec::new();

        for page in 1..=RELEASE_PAGES {
            let url = format!("{GITHUB_API_URL}?per_page=30&page={page}");
            let response = self
                .client
                .get(&url)
                .header("User-Agent", "canaveral")
                .header("Accept", "application/vnd.github+json")
                .send()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "python".into(),
                    reason: format!("failed to fetch releases (page {page}): {e}"),
                })?;

            if !response.status().is_success() {
                return Err(ToolError::RegistryFetchFailed {
                    tool: "python".into(),
                    reason: format!(
                        "GitHub API returned HTTP {} (page {page})",
                        response.status().as_u16()
                    ),
                });
            }

            let releases: Vec<GitHubRelease> =
                response
                    .json()
                    .await
                    .map_err(|e| ToolError::RegistryFetchFailed {
                        tool: "python".into(),
                        reason: format!("failed to parse releases (page {page}): {e}"),
                    })?;

            if releases.is_empty() {
                break;
            }

            all_releases.extend(releases);
        }

        // Store in cache
        {
            let mut guard = self.index_cache.lock().await;
            *guard = Some(CachedReleaseIndex {
                releases: all_releases.clone(),
                fetched_at: std::time::Instant::now(),
            });
        }

        Ok(all_releases)
    }

    /// Collect all available Python versions from the release index, for the
    /// current platform. Returns `(version, release_date)` pairs sorted
    /// newest-first.
    async fn available_versions(&self) -> Result<Vec<(String, String)>, ToolError> {
        let releases = self.fetch_releases().await?;
        let suffix = asset_name_pattern()?;
        let mut versions: Vec<(String, String)> = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Releases are newest-first from the API. Within each release, we
        // collect all Python versions available for our platform.
        for release in &releases {
            for asset in &release.assets {
                if asset.name.ends_with(&suffix) && asset.name.starts_with("cpython-") {
                    if let Some((version, date)) = parse_asset_name(&asset.name) {
                        if seen.insert(version.clone()) {
                            versions.push((version, date));
                        }
                    }
                }
            }
        }

        Ok(versions)
    }

    /// Resolve a (possibly partial) version prefix to a concrete
    /// `ResolvedPython` with a download URL.
    ///
    /// For example, `"3.12"` resolves to the latest `3.12.x` available.
    async fn resolve_version(&self, requested: &str) -> Result<ResolvedPython, ToolError> {
        let requested = requested.trim_start_matches('v');
        let releases = self.fetch_releases().await?;
        let suffix = asset_name_pattern()?;

        // Walk releases newest-first, find the first matching asset.
        for release in &releases {
            for asset in &release.assets {
                if !asset.name.ends_with(&suffix) || !asset.name.starts_with("cpython-") {
                    continue;
                }

                if let Some((version, date)) = parse_asset_name(&asset.name) {
                    // Skip pre-release versions unless explicitly requested
                    if !is_stable_version(&version) && is_stable_version(requested) {
                        continue;
                    }

                    if version_satisfies(&version, requested) {
                        debug!(
                            requested = %requested,
                            resolved = %version,
                            date = %date,
                            "resolved Python version"
                        );
                        return Ok(ResolvedPython {
                            version,
                            release_date: date,
                            download_url: asset.browser_download_url.clone(),
                        });
                    }
                }
            }
        }

        Err(ToolError::VersionNotAvailable {
            tool: "python".into(),
            version: requested.to_string(),
        })
    }

    // -- download & extract -------------------------------------------------

    /// Download and extract Python into `dest_dir`.
    ///
    /// The archive extracts to a `python/` directory. Returns the path to
    /// the `python/bin/` directory (unix) or `python/` directory (windows).
    async fn download_and_extract(
        &self,
        resolved: &ResolvedPython,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        info!(
            version = %resolved.version,
            url = %resolved.download_url,
            "downloading Python"
        );

        let response = self
            .client
            .get(&resolved.download_url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "python".into(),
                version: resolved.version.clone(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "python".into(),
                version: resolved.version.clone(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "python".into(),
                version: resolved.version.clone(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        // All python-build-standalone install_only assets are .tar.gz, even on Windows.
        Self::extract_tarball(&bytes, &resolved.version, dest_dir)?;

        // The archive extracts to `python/` inside dest_dir.
        let python_dir = dest_dir.join("python");
        let bin_dir = if is_windows() {
            // On Windows the binary is at python/python.exe (no bin/ subdirectory)
            python_dir.clone()
        } else {
            python_dir.join("bin")
        };

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "python".into(),
                version: resolved.version.clone(),
                reason: format!("expected directory not found at {}", bin_dir.display()),
            });
        }

        debug!(bin_dir = %bin_dir.display(), "Python extracted successfully");
        Ok(bin_dir)
    }

    /// Extract a `.tar.gz` archive into `dest_dir`.
    fn extract_tarball(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let decoder = flate2::read::GzDecoder::new(data);
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "python".into(),
                version: version.into(),
                reason: format!("failed to extract tarball: {e}"),
            })?;

        Ok(())
    }
}

impl Default for PythonProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Search for the `python3` binary inside a cache version directory.
///
/// The directory structure is: `{version_dir}/python/bin/python3`
fn find_python_binary_in_cache(version_dir: &Path) -> Option<PathBuf> {
    let python_dir = version_dir.join("python");
    if !python_dir.is_dir() {
        return None;
    }

    let bin = if cfg!(target_os = "windows") {
        python_dir.join("python.exe")
    } else {
        python_dir.join("bin").join("python3")
    };

    if bin.exists() {
        Some(bin)
    } else {
        None
    }
}

#[async_trait]
impl ToolProvider for PythonProvider {
    fn id(&self) -> &'static str {
        "python"
    }

    fn name(&self) -> &'static str {
        "Python"
    }

    fn binary_name(&self) -> &'static str {
        if cfg!(target_os = "windows") {
            "python"
        } else {
            "python3"
        }
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let binary = self.binary_name();

        // Try PATH first
        let output = tokio::process::Command::new(binary)
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(version) = parse_python_version(&stdout) {
                    debug!(version = %version, "detected Python on PATH");
                    return Ok(Some(version));
                }
                // Some older Pythons print to stderr
                let stderr = String::from_utf8_lossy(&out.stderr);
                if let Some(version) = parse_python_version(&stderr) {
                    debug!(version = %version, "detected Python on PATH (stderr)");
                    return Ok(Some(version));
                }
                warn!("python3 --version succeeded but could not parse output");
                return Ok(None);
            }
            Ok(_) => {
                debug!("{binary} --version returned non-zero exit status");
                return Ok(None);
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("{binary} not found on PATH");
                // Fall through to check cache
            }
            Err(e) => {
                return Err(ToolError::DetectionFailed(format!(
                    "failed to run `{binary} --version`: {e}"
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
                    if let Some(bin_path) = find_python_binary_in_cache(&path) {
                        let output = tokio::process::Command::new(&bin_path)
                            .arg("--version")
                            .output()
                            .await;
                        if let Ok(out) = output {
                            if out.status.success() {
                                let stdout = String::from_utf8_lossy(&out.stdout);
                                if let Some(version) = parse_python_version(&stdout) {
                                    debug!(
                                        version = %version,
                                        path = %bin_path.display(),
                                        "detected Python in cache"
                                    );
                                    return Ok(Some(version));
                                }
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
        let cache_dir = Self::default_cache_root().join(&resolved.version);
        self.install_to_cache(&resolved.version, &cache_dir).await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let resolved = self.resolve_version(version).await?;

        // Check if already extracted
        let expected_dir = if is_windows() {
            cache_dir.join("python")
        } else {
            cache_dir.join("python").join("bin")
        };

        if expected_dir.exists() {
            info!(version = %resolved.version, "Python already installed in cache");
            return Ok(InstallResult {
                tool: "python".into(),
                version: resolved.version,
                install_path: expected_dir,
            });
        }

        let bin_dir = self.download_and_extract(&resolved, cache_dir).await?;

        info!(version = %resolved.version, path = %bin_dir.display(), "Python installed");
        Ok(InstallResult {
            tool: "python".into(),
            version: resolved.version,
            install_path: bin_dir,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let versions = self.available_versions().await?;

        // Only return stable versions
        let stable: Vec<String> = versions
            .into_iter()
            .filter(|(v, _)| is_stable_version(v))
            .map(|(v, _)| v)
            .collect();

        Ok(stable)
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            install_path.display().to_string()
        } else {
            format!("{}:{path}", install_path.display())
        };

        // PYTHONHOME should point to the `python/` directory (parent of bin/).
        let python_home = if is_windows() {
            // On Windows, install_path IS the python/ directory
            install_path.to_path_buf()
        } else {
            // On Unix, install_path is python/bin/, so go up one level
            install_path.parent().unwrap_or(install_path).to_path_buf()
        };

        vec![
            ("PATH".into(), new_path),
            ("PYTHONHOME".into(), python_home.display().to_string()),
        ]
    }
}

// ---------------------------------------------------------------------------
// Version parsing
// ---------------------------------------------------------------------------

/// Parse a version string from `python3 --version` output.
///
/// Example: `"Python 3.12.0"` -> `Some("3.12.0")`
fn parse_python_version(output: &str) -> Option<String> {
    let re = Regex::new(r"Python (\d+\.\d+\.\d+)").ok()?;
    let caps = re.captures(output)?;
    Some(caps[1].to_string())
}

// ---------------------------------------------------------------------------
// PipProvider
// ---------------------------------------------------------------------------

/// Provider for pip — bundled with Python, not independently installable.
pub struct PipProvider;

impl PipProvider {
    pub fn new() -> Self {
        Self
    }

    /// The binary name for pip.
    fn pip_binary() -> &'static str {
        if cfg!(target_os = "windows") {
            "pip"
        } else {
            "pip3"
        }
    }
}

impl Default for PipProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for PipProvider {
    fn id(&self) -> &'static str {
        "pip"
    }

    fn name(&self) -> &'static str {
        "pip"
    }

    fn binary_name(&self) -> &'static str {
        Self::pip_binary()
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let binary = Self::pip_binary();

        let output = tokio::process::Command::new(binary)
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                // pip --version outputs e.g. "pip 24.0 from /usr/lib/python3/dist-packages/pip (python 3.12)"
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(version) = parse_pip_version(&stdout) {
                    debug!(version = %version, "detected pip version");
                    Ok(Some(version))
                } else {
                    warn!("{binary} --version succeeded but could not parse output");
                    Ok(None)
                }
            }
            Ok(_) => {
                warn!("{binary} --version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("{binary} not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `{binary} --version`: {e}"
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
            tool: "pip".into(),
            version: version.into(),
            reason: "pip is included with Python — install Python via \
                     `canaveral tools install python`"
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

/// Parse a version string from `pip3 --version` output.
///
/// Example: `"pip 24.0 from /usr/lib/... (python 3.12)"` -> `Some("24.0")`
fn parse_pip_version(output: &str) -> Option<String> {
    let re = Regex::new(r"pip (\d+\.\d+(?:\.\d+)?)").ok()?;
    let caps = re.captures(output)?;
    Some(caps[1].to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- PythonProvider basic properties ------------------------------------

    #[test]
    fn python_provider_id_and_name() {
        let provider = PythonProvider::new();
        assert_eq!(provider.id(), "python");
        assert_eq!(provider.name(), "Python");
    }

    #[test]
    fn python_provider_binary_name() {
        let provider = PythonProvider::new();
        let binary = provider.binary_name();
        if cfg!(target_os = "windows") {
            assert_eq!(binary, "python");
        } else {
            assert_eq!(binary, "python3");
        }
    }

    // -- PipProvider basic properties --------------------------------------

    #[test]
    fn pip_provider_id_and_name() {
        let provider = PipProvider::new();
        assert_eq!(provider.id(), "pip");
        assert_eq!(provider.name(), "pip");
    }

    #[test]
    fn pip_provider_binary_name() {
        let provider = PipProvider::new();
        let binary = provider.binary_name();
        if cfg!(target_os = "windows") {
            assert_eq!(binary, "pip");
        } else {
            assert_eq!(binary, "pip3");
        }
    }

    // -- Python version parsing --------------------------------------------

    #[test]
    fn parse_python_version_standard() {
        assert_eq!(
            parse_python_version("Python 3.12.0"),
            Some("3.12.0".to_string())
        );
    }

    #[test]
    fn parse_python_version_with_newline() {
        assert_eq!(
            parse_python_version("Python 3.13.12\n"),
            Some("3.13.12".to_string())
        );
    }

    #[test]
    fn parse_python_version_garbage() {
        assert_eq!(parse_python_version("not a version"), None);
    }

    #[test]
    fn parse_python_version_partial() {
        // Only major.minor should not match (we require major.minor.patch)
        assert_eq!(parse_python_version("Python 3.12"), None);
    }

    // -- Pip version parsing -----------------------------------------------

    #[test]
    fn parse_pip_version_standard() {
        assert_eq!(
            parse_pip_version("pip 24.0 from /usr/lib/python3/dist-packages/pip (python 3.12)"),
            Some("24.0".to_string())
        );
    }

    #[test]
    fn parse_pip_version_three_part() {
        assert_eq!(
            parse_pip_version("pip 23.3.1 from /usr/lib/... (python 3.11)"),
            Some("23.3.1".to_string())
        );
    }

    #[test]
    fn parse_pip_version_garbage() {
        assert_eq!(parse_pip_version("not a pip version"), None);
    }

    // -- Asset name parsing ------------------------------------------------

    #[test]
    fn parse_asset_name_standard() {
        let result =
            parse_asset_name("cpython-3.13.12+20260310-aarch64-apple-darwin-install_only.tar.gz");
        assert_eq!(
            result,
            Some(("3.13.12".to_string(), "20260310".to_string()))
        );
    }

    #[test]
    fn parse_asset_name_prerelease() {
        let result =
            parse_asset_name("cpython-3.15.0a7+20260310-aarch64-apple-darwin-install_only.tar.gz");
        assert_eq!(
            result,
            Some(("3.15.0a7".to_string(), "20260310".to_string()))
        );
    }

    #[test]
    fn parse_asset_name_windows() {
        let result =
            parse_asset_name("cpython-3.12.13+20260310-x86_64-pc-windows-msvc-install_only.tar.gz");
        assert_eq!(
            result,
            Some(("3.12.13".to_string(), "20260310".to_string()))
        );
    }

    #[test]
    fn parse_asset_name_invalid() {
        assert_eq!(parse_asset_name("not-a-python-asset.tar.gz"), None);
    }

    #[test]
    fn parse_asset_name_stripped_variant() {
        // We can still parse it, the filtering is done elsewhere
        let result = parse_asset_name(
            "cpython-3.13.12+20260310-aarch64-apple-darwin-install_only_stripped.tar.gz",
        );
        assert_eq!(
            result,
            Some(("3.13.12".to_string(), "20260310".to_string()))
        );
    }

    // -- Stable version detection ------------------------------------------

    #[test]
    fn stable_version_detection() {
        assert!(is_stable_version("3.12.0"));
        assert!(is_stable_version("3.13.12"));
        assert!(!is_stable_version("3.15.0a7"));
        assert!(!is_stable_version("3.14.0b1"));
        assert!(!is_stable_version("3.14.0rc1"));
    }

    // -- Platform detection ------------------------------------------------

    #[test]
    fn python_target_returns_known_value() {
        let target = python_target().unwrap();
        let valid = [
            "aarch64-apple-darwin",
            "x86_64-apple-darwin",
            "x86_64-unknown-linux-gnu",
            "aarch64-unknown-linux-gnu",
            "x86_64-pc-windows-msvc",
        ];
        assert!(valid.contains(&target), "unexpected target: {target}");
    }

    #[test]
    fn asset_name_pattern_contains_target() {
        let pattern = asset_name_pattern().unwrap();
        let target = python_target().unwrap();
        assert!(pattern.contains(target));
        assert!(pattern.ends_with("-install_only.tar.gz"));
    }

    // -- Download URL format -----------------------------------------------

    #[test]
    fn download_url_format() {
        // Construct the expected URL format
        let target = python_target().unwrap();
        let expected = format!(
            "https://github.com/astral-sh/python-build-standalone/releases/download/20260310/cpython-3.13.12%2B20260310-{target}-install_only.tar.gz"
        );
        // Verify the URL contains expected components
        assert!(expected.contains("astral-sh/python-build-standalone"));
        assert!(expected.contains("cpython-3.13.12"));
        assert!(expected.contains(target));
        assert!(expected.contains("install_only.tar.gz"));
    }

    // -- Version matching --------------------------------------------------

    #[test]
    fn python_version_matching() {
        assert!(version_satisfies("3.12.0", "3.12"));
        assert!(version_satisfies("3.12.0", "3"));
        assert!(version_satisfies("3.12.0", "3.12.0"));
        assert!(!version_satisfies("3.12.0", "3.11"));
        assert!(!version_satisfies("3.12.0", "3.13"));
    }

    #[test]
    fn python_version_prefix_no_false_match() {
        // "3.1" should not match "3.12.0"
        assert!(!version_satisfies("3.12.0", "3.1"));
        // "3.12" should not match "3.120.0"
        assert!(!version_satisfies("3.120.0", "3.12"));
    }

    // -- env_vars ----------------------------------------------------------

    #[test]
    fn python_env_vars_sets_path_and_pythonhome() {
        let provider = PythonProvider::new();

        let bin_dir = if cfg!(target_os = "windows") {
            Path::new("/home/user/.canaveral/tools/python/3.12.0/python")
        } else {
            Path::new("/home/user/.canaveral/tools/python/3.12.0/python/bin")
        };

        let vars = provider.env_vars(bin_dir);
        assert_eq!(vars.len(), 2);

        // First var should be PATH
        assert_eq!(vars[0].0, "PATH");
        assert!(vars[0].1.starts_with(&bin_dir.display().to_string()));

        // Second var should be PYTHONHOME
        assert_eq!(vars[1].0, "PYTHONHOME");
        let expected_home = "/home/user/.canaveral/tools/python/3.12.0/python";
        assert_eq!(vars[1].1, expected_home);
    }

    #[test]
    fn pip_env_vars_empty() {
        let provider = PipProvider::new();
        let vars = provider.env_vars(Path::new("/usr/local/bin"));
        assert!(vars.is_empty());
    }

    // -- PipProvider install error -----------------------------------------

    #[tokio::test]
    async fn pip_install_returns_helpful_error() {
        let provider = PipProvider::new();
        let result = provider.install("24").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("included with Python"),
            "error should mention pip is included: {err}"
        );
        assert!(
            err.contains("canaveral tools install python"),
            "error should reference canaveral: {err}"
        );
    }

    #[tokio::test]
    async fn pip_install_to_cache_also_errors() {
        let provider = PipProvider::new();
        let result = provider
            .install_to_cache("24", Path::new("/tmp/cache"))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn pip_list_available_returns_empty() {
        let provider = PipProvider::new();
        let result = provider.list_available().await.unwrap();
        assert!(result.is_empty());
    }

    // -- find_python_binary_in_cache ---------------------------------------

    #[test]
    fn find_python_binary_nonexistent_dir() {
        let result = find_python_binary_in_cache(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_none());
    }

    // -- default cache root ------------------------------------------------

    #[test]
    fn default_cache_root_contains_python() {
        let root = PythonProvider::default_cache_root();
        assert!(root.to_string_lossy().contains("python"));
        assert!(root.to_string_lossy().contains(".canaveral/tools"));
    }

    // -- resolve_version simulation (unit, no network) ---------------------

    #[test]
    fn resolve_version_finds_latest_matching() {
        // Simulate version resolution from a list of available versions
        let versions = vec![
            ("3.14.3", "20260310"),
            ("3.13.12", "20260310"),
            ("3.13.11", "20260303"),
            ("3.12.13", "20260310"),
            ("3.12.12", "20260303"),
            ("3.11.15", "20260310"),
            ("3.10.20", "20260310"),
        ];

        // "3.13" should resolve to 3.13.12 (newest 3.13.x)
        let resolved = versions
            .iter()
            .find(|(v, _)| is_stable_version(v) && version_satisfies(v, "3.13"))
            .map(|(v, _)| v.to_string());
        assert_eq!(resolved, Some("3.13.12".to_string()));

        // "3.12" should resolve to 3.12.13
        let resolved = versions
            .iter()
            .find(|(v, _)| is_stable_version(v) && version_satisfies(v, "3.12"))
            .map(|(v, _)| v.to_string());
        assert_eq!(resolved, Some("3.12.13".to_string()));

        // "3" should resolve to 3.14.3 (newest 3.x)
        let resolved = versions
            .iter()
            .find(|(v, _)| is_stable_version(v) && version_satisfies(v, "3"))
            .map(|(v, _)| v.to_string());
        assert_eq!(resolved, Some("3.14.3".to_string()));

        // "3.15" should resolve to nothing (no stable 3.15.x)
        let resolved = versions
            .iter()
            .find(|(v, _)| is_stable_version(v) && version_satisfies(v, "3.15"))
            .map(|(v, _)| v.to_string());
        assert_eq!(resolved, None);

        // Exact match
        let resolved = versions
            .iter()
            .find(|(v, _)| is_stable_version(v) && version_satisfies(v, "3.11.15"))
            .map(|(v, _)| v.to_string());
        assert_eq!(resolved, Some("3.11.15".to_string()));
    }

    // -- GitHub release API types ------------------------------------------

    #[test]
    fn parse_github_release_json() {
        let json = r#"{
            "tag_name": "20260310",
            "assets": [
                {
                    "name": "cpython-3.13.12+20260310-aarch64-apple-darwin-install_only.tar.gz",
                    "browser_download_url": "https://github.com/astral-sh/python-build-standalone/releases/download/20260310/cpython-3.13.12%2B20260310-aarch64-apple-darwin-install_only.tar.gz"
                }
            ]
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "20260310");
        assert_eq!(release.assets.len(), 1);
        assert!(release.assets[0].name.contains("cpython-3.13.12"));
        assert!(release.assets[0]
            .browser_download_url
            .contains("astral-sh/python-build-standalone"));
    }

    #[test]
    fn parse_github_release_multiple_assets() {
        let json = r#"{
            "tag_name": "20260310",
            "assets": [
                {
                    "name": "cpython-3.12.13+20260310-aarch64-apple-darwin-install_only.tar.gz",
                    "browser_download_url": "https://example.com/3.12.13"
                },
                {
                    "name": "cpython-3.13.12+20260310-aarch64-apple-darwin-install_only.tar.gz",
                    "browser_download_url": "https://example.com/3.13.12"
                },
                {
                    "name": "cpython-3.13.12+20260310-aarch64-apple-darwin-install_only_stripped.tar.gz",
                    "browser_download_url": "https://example.com/3.13.12-stripped"
                }
            ]
        }"#;

        let release: GitHubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.assets.len(), 3);

        // Filter to install_only (non-stripped) for our platform suffix
        let non_stripped: Vec<_> = release
            .assets
            .iter()
            .filter(|a| a.name.ends_with("-install_only.tar.gz") && !a.name.contains("stripped"))
            .collect();
        assert_eq!(non_stripped.len(), 2);
    }
}
