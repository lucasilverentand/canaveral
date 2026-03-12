//! Flutter SDK and standalone Dart SDK tool providers
//!
//! **FlutterProvider** downloads the Flutter SDK from Google Cloud Storage,
//! which bundles a Dart runtime in `flutter/bin/dart`.
//!
//! **DartProvider** downloads the standalone Dart SDK from the Dart archive.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// Flutter releases index types
// ---------------------------------------------------------------------------

/// Top-level structure of the Flutter releases JSON from Google Cloud Storage.
///
/// Example URL: `https://storage.googleapis.com/flutter_infra_release/releases/releases_macos.json`
#[derive(Debug, Clone, Deserialize)]
struct FlutterReleasesIndex {
    /// Base URL for all archives, e.g.
    /// `"https://storage.googleapis.com/flutter_infra_release/releases"`.
    base_url: String,
    /// Flat list of every published release (all channels, architectures).
    releases: Vec<FlutterReleaseEntry>,
}

/// A single entry from the Flutter releases JSON.
#[derive(Debug, Clone, Deserialize)]
struct FlutterReleaseEntry {
    /// Git commit hash.
    #[allow(dead_code)]
    hash: String,
    /// Release channel: `"stable"`, `"beta"`, or `"dev"`.
    channel: String,
    /// Version string, e.g. `"3.41.4"`.
    version: String,
    /// Architecture of the bundled Dart SDK: `"x64"` or `"arm64"`.
    #[serde(default)]
    dart_sdk_arch: Option<String>,
    /// Relative archive path, e.g. `"stable/macos/flutter_macos_arm64_3.41.4-stable.zip"`.
    archive: String,
    /// SHA-256 hex digest of the archive.
    sha256: String,
}

/// Cached Flutter releases index with a TTL.
struct CachedFlutterIndex {
    index: FlutterReleasesIndex,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the Flutter releases JSON OS identifier.
fn flutter_os() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") {
        Ok("macos")
    } else if cfg!(target_os = "linux") {
        Ok("linux")
    } else if cfg!(target_os = "windows") {
        Ok("windows")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "flutter: unsupported operating system".to_string(),
        ))
    }
}

/// Returns the architecture identifier used in Flutter releases.
fn flutter_arch() -> Result<&'static str, ToolError> {
    if cfg!(target_arch = "aarch64") {
        Ok("arm64")
    } else if cfg!(target_arch = "x86_64") {
        Ok("x64")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "flutter: unsupported CPU architecture".to_string(),
        ))
    }
}

/// Returns the Dart SDK OS identifier.
fn dart_os() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") {
        Ok("macos")
    } else if cfg!(target_os = "linux") {
        Ok("linux")
    } else if cfg!(target_os = "windows") {
        Ok("windows")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "dart: unsupported operating system".to_string(),
        ))
    }
}

/// Returns the Dart SDK architecture identifier.
fn dart_arch() -> Result<&'static str, ToolError> {
    if cfg!(target_arch = "aarch64") {
        Ok("arm64")
    } else if cfg!(target_arch = "x86_64") {
        Ok("x64")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "dart: unsupported CPU architecture".to_string(),
        ))
    }
}

/// Whether the archive is a `.tar.xz` (Linux) rather than `.zip`.
fn flutter_archive_is_tar_xz() -> bool {
    cfg!(target_os = "linux")
}

// ---------------------------------------------------------------------------
// FlutterProvider
// ---------------------------------------------------------------------------

/// Provider for the Flutter SDK — downloads pre-built archives from Google
/// Cloud Storage.
pub struct FlutterProvider {
    client: reqwest::Client,
    /// In-memory cache for the releases index (24-hour TTL).
    index_cache: tokio::sync::Mutex<Option<CachedFlutterIndex>>,
}

impl FlutterProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            index_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/flutter/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/flutter")
    }

    // -- releases index -----------------------------------------------------

    /// URL for the platform-specific Flutter releases JSON.
    fn releases_url() -> Result<String, ToolError> {
        let os = flutter_os()?;
        Ok(format!(
            "https://storage.googleapis.com/flutter_infra_release/releases/releases_{os}.json"
        ))
    }

    /// Fetch the Flutter releases index, using an in-memory 24-hour cache.
    async fn fetch_index(&self) -> Result<FlutterReleasesIndex, ToolError> {
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);

        {
            let guard = self.index_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    debug!("using cached Flutter releases index");
                    return Ok(cached.index.clone());
                }
            }
        }

        let url = Self::releases_url()?;
        info!(url = %url, "fetching Flutter releases index");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "flutter".into(),
                reason: format!("failed to fetch releases index: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "flutter".into(),
                reason: format!(
                    "releases index returned HTTP {}",
                    response.status().as_u16()
                ),
            });
        }

        let index: FlutterReleasesIndex =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "flutter".into(),
                    reason: format!("failed to parse releases index: {e}"),
                })?;

        {
            let mut guard = self.index_cache.lock().await;
            *guard = Some(CachedFlutterIndex {
                index: index.clone(),
                fetched_at: std::time::Instant::now(),
            });
        }

        Ok(index)
    }

    /// Find the release entry matching the requested version, channel, and
    /// current platform architecture.
    async fn find_release(
        &self,
        requested: &str,
    ) -> Result<(FlutterReleaseEntry, String), ToolError> {
        let arch = flutter_arch()?;
        let index = self.fetch_index().await?;

        // Walk releases (newest first in the JSON) and find the first entry
        // that matches the requested version prefix on the stable channel with
        // the correct architecture.
        for entry in &index.releases {
            if entry.channel != "stable" {
                continue;
            }
            let entry_arch = entry.dart_sdk_arch.as_deref().unwrap_or("x64");
            if entry_arch != arch {
                continue;
            }
            if version_satisfies(&entry.version, requested) {
                debug!(
                    requested = %requested,
                    resolved = %entry.version,
                    archive = %entry.archive,
                    "resolved Flutter version"
                );
                return Ok((entry.clone(), index.base_url.clone()));
            }
        }

        Err(ToolError::VersionNotAvailable {
            tool: "flutter".into(),
            version: requested.to_string(),
        })
    }

    // -- download & extract -------------------------------------------------

    /// Download and extract Flutter SDK into `dest_dir`.
    ///
    /// Returns the path to the `flutter/bin` directory.
    async fn download_and_extract(
        &self,
        entry: &FlutterReleaseEntry,
        base_url: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = format!("{base_url}/{}", entry.archive);
        info!(version = %entry.version, url = %url, "downloading Flutter SDK");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "flutter".into(),
                version: entry.version.clone(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "flutter".into(),
                version: entry.version.clone(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "flutter".into(),
                version: entry.version.clone(),
                reason: format!("failed to read response body: {e}"),
            })?;

        // Verify SHA-256 checksum
        Self::verify_sha256(&bytes, &entry.sha256, &entry.version)?;

        std::fs::create_dir_all(dest_dir)?;

        if flutter_archive_is_tar_xz() {
            Self::extract_tar_xz(&bytes, &entry.version, dest_dir)?;
        } else {
            Self::extract_zip(&bytes, &entry.version, dest_dir)?;
        }

        // The archive extracts to a `flutter/` directory inside dest_dir.
        let flutter_dir = dest_dir.join("flutter");
        let bin_dir = flutter_dir.join("bin");

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "flutter".into(),
                version: entry.version.clone(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(bin_dir = %bin_dir.display(), "Flutter SDK extracted successfully");
        Ok(bin_dir)
    }

    /// Verify SHA-256 checksum of downloaded bytes.
    fn verify_sha256(data: &[u8], expected: &str, version: &str) -> Result<(), ToolError> {
        use sha2::Digest;
        let mut hasher = sha2::Sha256::new();
        hasher.update(data);
        let actual = format!("{:x}", hasher.finalize());

        if actual != expected {
            return Err(ToolError::ChecksumMismatch {
                tool: "flutter".into(),
                version: version.into(),
                expected: expected.into(),
                actual,
            });
        }

        debug!("SHA-256 checksum verified for Flutter {version}");
        Ok(())
    }

    /// Extract a `.zip` archive into `dest_dir`.
    fn extract_zip(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| ToolError::ExtractionFailed {
                tool: "flutter".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "flutter".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        Ok(())
    }

    /// Extract a `.tar.xz` archive into `dest_dir` by shelling out to `tar`.
    ///
    /// We shell out because adding an xz/lzma decompression crate is heavy
    /// and `tar` is universally available on Linux.
    fn extract_tar_xz(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        use std::io::Write;
        use std::process::Command;

        // Write the archive to a temporary file
        let tmp_path = dest_dir.join(format!("flutter-{version}.tar.xz"));
        {
            let mut file =
                std::fs::File::create(&tmp_path).map_err(|e| ToolError::ExtractionFailed {
                    tool: "flutter".into(),
                    version: version.into(),
                    reason: format!("failed to create temp archive file: {e}"),
                })?;
            file.write_all(data)
                .map_err(|e| ToolError::ExtractionFailed {
                    tool: "flutter".into(),
                    version: version.into(),
                    reason: format!("failed to write archive data: {e}"),
                })?;
        }

        let status = Command::new("tar")
            .args(["xf", &tmp_path.display().to_string()])
            .current_dir(dest_dir)
            .status()
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "flutter".into(),
                version: version.into(),
                reason: format!("failed to run `tar`: {e}"),
            })?;

        // Clean up the temp file regardless of outcome
        let _ = std::fs::remove_file(&tmp_path);

        if !status.success() {
            return Err(ToolError::ExtractionFailed {
                tool: "flutter".into(),
                version: version.into(),
                reason: format!("tar exited with status {}", status.code().unwrap_or(-1)),
            });
        }

        Ok(())
    }
}

impl Default for FlutterProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for FlutterProvider {
    fn id(&self) -> &'static str {
        "flutter"
    }

    fn name(&self) -> &'static str {
        "Flutter SDK"
    }

    fn binary_name(&self) -> &'static str {
        "flutter"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let output = tokio::process::Command::new("flutter")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(version) = parse_flutter_version(&stdout) {
                    debug!(version = %version, "detected Flutter on PATH");
                    return Ok(Some(version));
                }
                warn!("flutter --version succeeded but could not parse version");
                Ok(None)
            }
            Ok(_) => {
                debug!("flutter --version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("flutter not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `flutter --version`: {e}"
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
        let (entry, base_url) = self.find_release(version).await?;
        let cache_dir = Self::default_cache_root().join(&entry.version);
        self.install_to_cache_inner(&entry, &base_url, &cache_dir)
            .await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let (entry, base_url) = self.find_release(version).await?;
        self.install_to_cache_inner(&entry, &base_url, cache_dir)
            .await
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let arch = flutter_arch()?;
        let index = self.fetch_index().await?;

        // Deduplicate: only return stable-channel versions for the current arch.
        let mut seen = std::collections::HashSet::new();
        let versions: Vec<String> = index
            .releases
            .iter()
            .filter(|e| {
                e.channel == "stable" && e.dart_sdk_arch.as_deref().unwrap_or("x64") == arch
            })
            .filter_map(|e| {
                if seen.insert(e.version.clone()) {
                    Some(e.version.clone())
                } else {
                    None
                }
            })
            .collect();

        Ok(versions)
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        // install_path is `<cache>/flutter/bin`
        let flutter_root = install_path.parent().unwrap_or(install_path).to_path_buf();

        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            install_path.display().to_string()
        } else {
            format!("{}:{path}", install_path.display())
        };

        vec![
            ("PATH".into(), new_path),
            ("FLUTTER_ROOT".into(), flutter_root.display().to_string()),
        ]
    }
}

impl FlutterProvider {
    /// Shared install logic used by both `install` and `install_to_cache`.
    async fn install_to_cache_inner(
        &self,
        entry: &FlutterReleaseEntry,
        base_url: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        // Check if already extracted
        let expected_bin = cache_dir.join("flutter").join("bin");
        if expected_bin.exists() {
            info!(version = %entry.version, "Flutter SDK already installed in cache");
            return Ok(InstallResult {
                tool: "flutter".into(),
                version: entry.version.clone(),
                install_path: expected_bin,
            });
        }

        let bin_dir = self
            .download_and_extract(entry, base_url, cache_dir)
            .await?;

        info!(version = %entry.version, path = %bin_dir.display(), "Flutter SDK installed");
        Ok(InstallResult {
            tool: "flutter".into(),
            version: entry.version.clone(),
            install_path: bin_dir,
        })
    }
}

// ---------------------------------------------------------------------------
// Version parsing helpers
// ---------------------------------------------------------------------------

/// Parse a Flutter version from `flutter --version` output.
///
/// The first line looks like:
/// ```text
/// Flutter 3.41.4 • channel stable • https://github.com/flutter/flutter.git
/// ```
fn parse_flutter_version(output: &str) -> Option<String> {
    let re = Regex::new(r"Flutter\s+(\d+\.\d+\.\d+)").ok()?;
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// Parse a Dart version from `dart --version` output.
///
/// The output looks like:
/// ```text
/// Dart SDK version: 3.11.1 (stable) (Thu Feb 27 10:58:45 2026 +0000) on "macos_arm64"
/// ```
fn parse_dart_version(output: &str) -> Option<String> {
    let re = Regex::new(r"Dart SDK version:\s*(\d+\.\d+\.\d+)").ok()?;
    re.captures(output)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

// ---------------------------------------------------------------------------
// Dart latest version type
// ---------------------------------------------------------------------------

/// Response from the Dart VERSION endpoint.
///
/// Example: `https://storage.googleapis.com/dart-archive/channels/stable/release/latest/VERSION`
#[derive(Debug, Clone, Deserialize)]
struct DartVersionInfo {
    version: String,
    #[allow(dead_code)]
    date: String,
    #[allow(dead_code)]
    revision: String,
}

// ---------------------------------------------------------------------------
// DartProvider
// ---------------------------------------------------------------------------

/// Provider for the standalone Dart SDK — downloads pre-built archives from
/// the Dart archive on Google Cloud Storage.
pub struct DartProvider {
    client: reqwest::Client,
}

impl DartProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// Default cache root: `~/.canaveral/tools/dart/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/dart")
    }

    /// Build the download URL for a specific Dart SDK version.
    fn download_url(version: &str) -> Result<String, ToolError> {
        let os = dart_os()?;
        let arch = dart_arch()?;
        Ok(format!(
            "https://storage.googleapis.com/dart-archive/channels/stable/release/{version}/sdk/dartsdk-{os}-{arch}-release.zip"
        ))
    }

    /// Fetch the latest stable Dart SDK version string.
    async fn fetch_latest_version(&self) -> Result<String, ToolError> {
        let url =
            "https://storage.googleapis.com/dart-archive/channels/stable/release/latest/VERSION";

        let response = self
            .client
            .get(url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "dart".into(),
                reason: format!("failed to fetch latest version: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "dart".into(),
                reason: format!(
                    "latest version endpoint returned HTTP {}",
                    response.status().as_u16()
                ),
            });
        }

        let info: DartVersionInfo =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "dart".into(),
                    reason: format!("failed to parse version info: {e}"),
                })?;

        Ok(info.version)
    }

    /// Check if a specific Dart SDK version exists by issuing a HEAD request.
    async fn version_exists(&self, version: &str) -> Result<bool, ToolError> {
        let url = Self::download_url(version)?;

        let response = self
            .client
            .head(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "dart".into(),
                reason: format!("failed to check version existence: {e}"),
            })?;

        Ok(response.status().is_success())
    }

    /// Download and extract the Dart SDK into `dest_dir`.
    ///
    /// Returns the path to the `dart-sdk/bin` directory.
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = Self::download_url(version)?;
        info!(version = %version, url = %url, "downloading Dart SDK");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        let cursor = std::io::Cursor::new(&bytes[..]);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| ToolError::ExtractionFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        let bin_dir = dest_dir.join("dart-sdk").join("bin");
        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "dart".into(),
                version: version.into(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(bin_dir = %bin_dir.display(), "Dart SDK extracted successfully");
        Ok(bin_dir)
    }
}

impl Default for DartProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for DartProvider {
    fn id(&self) -> &'static str {
        "dart"
    }

    fn name(&self) -> &'static str {
        "Dart SDK"
    }

    fn binary_name(&self) -> &'static str {
        "dart"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let output = tokio::process::Command::new("dart")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                // dart --version writes to stderr on some versions, stdout on others
                let combined = format!(
                    "{}{}",
                    String::from_utf8_lossy(&out.stdout),
                    String::from_utf8_lossy(&out.stderr),
                );
                if let Some(version) = parse_dart_version(&combined) {
                    debug!(version = %version, "detected Dart on PATH");
                    return Ok(Some(version));
                }
                warn!("dart --version succeeded but could not parse version");
                Ok(None)
            }
            Ok(_) => {
                debug!("dart --version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("dart not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `dart --version`: {e}"
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
        // If "latest" is requested, resolve it first
        let resolved = if version == "latest" {
            self.fetch_latest_version().await?
        } else {
            version.to_string()
        };

        // Verify the version exists before downloading
        if !self.version_exists(&resolved).await? {
            return Err(ToolError::VersionNotAvailable {
                tool: "dart".into(),
                version: resolved,
            });
        }

        let cache_dir = Self::default_cache_root().join(&resolved);
        self.install_to_cache_inner(&resolved, &cache_dir).await
    }

    async fn install_to_cache(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let resolved = if version == "latest" {
            self.fetch_latest_version().await?
        } else {
            version.to_string()
        };

        if !self.version_exists(&resolved).await? {
            return Err(ToolError::VersionNotAvailable {
                tool: "dart".into(),
                version: resolved,
            });
        }

        self.install_to_cache_inner(&resolved, cache_dir).await
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        // The Dart archive does not provide a full version listing API.
        // Return the latest stable version as the only known-available version.
        let latest = self.fetch_latest_version().await?;
        Ok(vec![latest])
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

impl DartProvider {
    /// Shared install logic for both `install` and `install_to_cache`.
    async fn install_to_cache_inner(
        &self,
        version: &str,
        cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        let expected_bin = cache_dir.join("dart-sdk").join("bin");
        if expected_bin.exists() {
            info!(version = %version, "Dart SDK already installed in cache");
            return Ok(InstallResult {
                tool: "dart".into(),
                version: version.to_string(),
                install_path: expected_bin,
            });
        }

        let bin_dir = self.download_and_extract(version, cache_dir).await?;

        info!(version = %version, path = %bin_dir.display(), "Dart SDK installed");
        Ok(InstallResult {
            tool: "dart".into(),
            version: version.to_string(),
            install_path: bin_dir,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- FlutterProvider basic properties ------------------------------------

    #[test]
    fn flutter_provider_id() {
        let provider = FlutterProvider::new();
        assert_eq!(provider.id(), "flutter");
    }

    #[test]
    fn flutter_provider_name() {
        let provider = FlutterProvider::new();
        assert_eq!(provider.name(), "Flutter SDK");
    }

    #[test]
    fn flutter_provider_binary_name() {
        let provider = FlutterProvider::new();
        assert_eq!(provider.binary_name(), "flutter");
    }

    // -- DartProvider basic properties --------------------------------------

    #[test]
    fn dart_provider_id() {
        let provider = DartProvider::new();
        assert_eq!(provider.id(), "dart");
    }

    #[test]
    fn dart_provider_name() {
        let provider = DartProvider::new();
        assert_eq!(provider.name(), "Dart SDK");
    }

    #[test]
    fn dart_provider_binary_name() {
        let provider = DartProvider::new();
        assert_eq!(provider.binary_name(), "dart");
    }

    // -- Flutter version regex ----------------------------------------------

    #[test]
    fn parse_flutter_version_stable_output() {
        let output = "Flutter 3.41.4 • channel stable • https://github.com/flutter/flutter.git\n\
                       Framework • revision ff37bef603 (5 days ago) • 2026-03-04 18:34:51\n\
                       Engine • revision 1234567890\n\
                       Tools • Dart 3.11.1 • DevTools 2.40.2";
        assert_eq!(parse_flutter_version(output), Some("3.41.4".to_string()));
    }

    #[test]
    fn parse_flutter_version_no_match() {
        assert_eq!(parse_flutter_version("not flutter output"), None);
    }

    #[test]
    fn parse_flutter_version_prerelease() {
        let output = "Flutter 3.42.0 • channel beta • https://github.com/flutter/flutter.git";
        assert_eq!(parse_flutter_version(output), Some("3.42.0".to_string()));
    }

    // -- Dart version regex -------------------------------------------------

    #[test]
    fn parse_dart_version_stable_output() {
        let output = r#"Dart SDK version: 3.11.1 (stable) (Thu Feb 27 10:58:45 2026 +0000) on "macos_arm64""#;
        assert_eq!(parse_dart_version(output), Some("3.11.1".to_string()));
    }

    #[test]
    fn parse_dart_version_no_match() {
        assert_eq!(parse_dart_version("unknown output"), None);
    }

    #[test]
    fn parse_dart_version_stderr_format() {
        // Some Dart versions print to stderr with slightly different format
        let output = "Dart SDK version: 3.2.0 (stable) on \"linux_x64\"";
        assert_eq!(parse_dart_version(output), Some("3.2.0".to_string()));
    }

    // -- Platform helpers ---------------------------------------------------

    #[test]
    fn flutter_os_returns_known_value() {
        let os = flutter_os().unwrap();
        assert!(
            ["macos", "linux", "windows"].contains(&os),
            "unexpected Flutter OS: {os}"
        );
    }

    #[test]
    fn flutter_arch_returns_known_value() {
        let arch = flutter_arch().unwrap();
        assert!(
            ["arm64", "x64"].contains(&arch),
            "unexpected Flutter arch: {arch}"
        );
    }

    #[test]
    fn dart_os_returns_known_value() {
        let os = dart_os().unwrap();
        assert!(
            ["macos", "linux", "windows"].contains(&os),
            "unexpected Dart OS: {os}"
        );
    }

    #[test]
    fn dart_arch_returns_known_value() {
        let arch = dart_arch().unwrap();
        assert!(
            ["arm64", "x64"].contains(&arch),
            "unexpected Dart arch: {arch}"
        );
    }

    // -- Dart download URL format -------------------------------------------

    #[test]
    fn dart_download_url_format() {
        let url = DartProvider::download_url("3.11.1").unwrap();
        let os = dart_os().unwrap();
        let arch = dart_arch().unwrap();
        assert_eq!(
            url,
            format!(
                "https://storage.googleapis.com/dart-archive/channels/stable/release/3.11.1/sdk/dartsdk-{os}-{arch}-release.zip"
            )
        );
    }

    #[test]
    fn dart_download_url_different_version() {
        let url = DartProvider::download_url("3.2.0").unwrap();
        assert!(url.contains("/3.2.0/"));
        assert!(url.ends_with("-release.zip"));
    }

    // -- Flutter releases index parsing -------------------------------------

    #[test]
    fn parse_flutter_releases_index() {
        let json = r#"{
            "base_url": "https://storage.googleapis.com/flutter_infra_release/releases",
            "current_release": {
                "stable": "ff37bef603469fb030f2b72995ab929ccfc227f0"
            },
            "releases": [
                {
                    "hash": "ff37bef603469fb030f2b72995ab929ccfc227f0",
                    "channel": "stable",
                    "version": "3.41.4",
                    "dart_sdk_arch": "arm64",
                    "archive": "stable/macos/flutter_macos_arm64_3.41.4-stable.zip",
                    "sha256": "16984a0dae1f13c7b3e05b973913a939144b7e9e3aa6439fa3382599a6326f8c"
                },
                {
                    "hash": "ff37bef603469fb030f2b72995ab929ccfc227f0",
                    "channel": "stable",
                    "version": "3.41.4",
                    "dart_sdk_arch": "x64",
                    "archive": "stable/macos/flutter_macos_3.41.4-stable.zip",
                    "sha256": "05fd35d2e4c29def0acb02e08e4faa6853115c3d9cf05c4e30c7e687b51f746d"
                },
                {
                    "hash": "9d59b38b5755777570b861e93be1e7c7f9913027",
                    "channel": "beta",
                    "version": "3.42.0-0.4.pre",
                    "dart_sdk_arch": "arm64",
                    "archive": "beta/macos/flutter_macos_arm64_3.42.0-0.4.pre-beta.zip",
                    "sha256": "7e20aaac04166b34556a9151b14e19a7e8ddba58bd39d987805b834f55b5c2d6"
                }
            ]
        }"#;

        let index: FlutterReleasesIndex = serde_json::from_str(json).unwrap();
        assert_eq!(
            index.base_url,
            "https://storage.googleapis.com/flutter_infra_release/releases"
        );
        assert_eq!(index.releases.len(), 3);

        let first = &index.releases[0];
        assert_eq!(first.channel, "stable");
        assert_eq!(first.version, "3.41.4");
        assert_eq!(first.dart_sdk_arch.as_deref(), Some("arm64"));
        assert_eq!(
            first.archive,
            "stable/macos/flutter_macos_arm64_3.41.4-stable.zip"
        );

        // Beta entry should be filtered out in list_available
        assert_eq!(index.releases[2].channel, "beta");
    }

    #[test]
    fn parse_flutter_release_entry_without_dart_sdk_arch() {
        // Very old releases may not have dart_sdk_arch
        let json = r#"{
            "hash": "abc123",
            "channel": "stable",
            "version": "1.0.0",
            "archive": "stable/macos/flutter_macos_1.0.0-stable.zip",
            "sha256": "deadbeef"
        }"#;

        let entry: FlutterReleaseEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.version, "1.0.0");
        assert!(entry.dart_sdk_arch.is_none());
    }

    // -- Dart VERSION JSON parsing ------------------------------------------

    #[test]
    fn parse_dart_version_info() {
        let json = r#"{
            "date": "2026-03-10",
            "version": "3.11.2",
            "revision": "e2156f3acaca397d466a8f8370bef922f939a859"
        }"#;

        let info: DartVersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.version, "3.11.2");
        assert_eq!(info.date, "2026-03-10");
    }

    // -- env_vars -----------------------------------------------------------

    #[test]
    fn flutter_env_vars_sets_path_and_flutter_root() {
        let provider = FlutterProvider::new();
        let bin = Path::new("/home/user/.canaveral/tools/flutter/3.41.4/flutter/bin");
        let vars = provider.env_vars(bin);

        assert_eq!(vars.len(), 2);

        // PATH should be first and prepended
        assert_eq!(vars[0].0, "PATH");
        assert!(vars[0].1.starts_with(&bin.display().to_string()));

        // FLUTTER_ROOT should point to the flutter dir (parent of bin)
        assert_eq!(vars[1].0, "FLUTTER_ROOT");
        assert_eq!(
            vars[1].1,
            "/home/user/.canaveral/tools/flutter/3.41.4/flutter"
        );
    }

    #[test]
    fn dart_env_vars_prepends_path() {
        let provider = DartProvider::new();
        let bin = Path::new("/home/user/.canaveral/tools/dart/3.11.1/dart-sdk/bin");
        let vars = provider.env_vars(bin);

        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].0, "PATH");
        assert!(vars[0].1.starts_with(&bin.display().to_string()));
    }

    // -- version matching ---------------------------------------------------

    #[test]
    fn flutter_version_matching() {
        assert!(version_satisfies("3.41.4", "3"));
        assert!(version_satisfies("3.41.4", "3.41"));
        assert!(version_satisfies("3.41.4", "3.41.4"));
        assert!(!version_satisfies("3.41.4", "3.40"));
        assert!(!version_satisfies("3.41.4", "2"));
    }

    #[test]
    fn dart_version_matching() {
        assert!(version_satisfies("3.11.1", "3"));
        assert!(version_satisfies("3.11.1", "3.11"));
        assert!(version_satisfies("3.11.1", "3.11.1"));
        assert!(!version_satisfies("3.11.1", "3.12"));
        assert!(!version_satisfies("3.11.1", "2"));
    }

    // -- Flutter releases URL -----------------------------------------------

    #[test]
    fn flutter_releases_url_format() {
        let url = FlutterProvider::releases_url().unwrap();
        let os = flutter_os().unwrap();
        assert_eq!(
            url,
            format!(
                "https://storage.googleapis.com/flutter_infra_release/releases/releases_{os}.json"
            )
        );
    }

    // -- SHA-256 verification -----------------------------------------------

    #[test]
    fn verify_sha256_correct() {
        let data = b"hello world";
        // SHA-256 of "hello world"
        let expected = "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9";
        assert!(FlutterProvider::verify_sha256(data, expected, "test").is_ok());
    }

    #[test]
    fn verify_sha256_mismatch() {
        let data = b"hello world";
        let expected = "0000000000000000000000000000000000000000000000000000000000000000";
        let result = FlutterProvider::verify_sha256(data, expected, "test");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("checksum mismatch"), "unexpected error: {err}");
    }

    // -- Default cache roots ------------------------------------------------

    #[test]
    fn flutter_default_cache_root_ends_with_flutter() {
        let root = FlutterProvider::default_cache_root();
        assert!(
            root.ends_with(".canaveral/tools/flutter"),
            "unexpected cache root: {}",
            root.display()
        );
    }

    #[test]
    fn dart_default_cache_root_ends_with_dart() {
        let root = DartProvider::default_cache_root();
        assert!(
            root.ends_with(".canaveral/tools/dart"),
            "unexpected cache root: {}",
            root.display()
        );
    }

    // -- Archive type detection ---------------------------------------------

    #[test]
    fn flutter_archive_type_matches_platform() {
        let is_tar_xz = flutter_archive_is_tar_xz();
        if cfg!(target_os = "linux") {
            assert!(is_tar_xz, "Linux should use tar.xz");
        } else {
            assert!(!is_tar_xz, "macOS/Windows should use zip");
        }
    }
}
