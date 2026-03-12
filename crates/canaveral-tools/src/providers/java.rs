//! Java (Eclipse Temurin) and Gradle tool providers
//!
//! Downloads and installs pre-built Temurin JDK binaries from the Adoptium API
//! and Gradle distributions from services.gradle.org.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use tracing::{debug, info};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// Adoptium API types
// ---------------------------------------------------------------------------

/// Response from `GET /v3/info/available_releases`.
#[derive(Debug, Clone, Deserialize)]
struct AvailableReleases {
    available_releases: Vec<u32>,
    #[allow(dead_code)]
    most_recent_lts: u32,
    #[allow(dead_code)]
    most_recent_feature_release: u32,
}

/// A single version entry from the Adoptium release-versions endpoint.
#[derive(Debug, Clone, Deserialize)]
struct AdoptiumVersionEntry {
    /// The semver string, e.g. `"21.0.4+7"`.
    semver: String,
}

/// Wrapper for the paginated release-versions response.
#[derive(Debug, Clone, Deserialize)]
struct ReleaseVersionsResponse {
    versions: Vec<AdoptiumVersionEntry>,
}

/// Cached version list with a TTL.
struct CachedVersions {
    /// Map from major version to list of full version strings (newest first).
    entries: Vec<(u32, Vec<String>)>,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Gradle API types
// ---------------------------------------------------------------------------

/// A single entry from `GET https://services.gradle.org/versions/all`.
#[derive(Debug, Clone, Deserialize)]
struct GradleVersionEntry {
    version: String,
    snapshot: bool,
    #[serde(rename = "activeRc")]
    #[allow(dead_code)]
    active_rc: bool,
    #[serde(rename = "rcFor")]
    #[allow(dead_code)]
    rc_for: String,
    #[serde(rename = "broken")]
    #[allow(dead_code)]
    broken: bool,
}

/// Cached Gradle version list.
struct CachedGradleVersions {
    entries: Vec<GradleVersionEntry>,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the Adoptium OS identifier for the current platform.
fn adoptium_os() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") {
        Ok("mac")
    } else if cfg!(target_os = "linux") {
        Ok("linux")
    } else if cfg!(target_os = "windows") {
        Ok("windows")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "java: unsupported operating system".to_string(),
        ))
    }
}

/// Returns the Adoptium architecture identifier for the current CPU.
fn adoptium_arch() -> Result<&'static str, ToolError> {
    if cfg!(target_arch = "aarch64") {
        Ok("aarch64")
    } else if cfg!(target_arch = "x86_64") {
        Ok("x64")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "java: unsupported CPU architecture".to_string(),
        ))
    }
}

/// Whether the current platform is macOS.
fn is_macos() -> bool {
    cfg!(target_os = "macos")
}

/// Whether the current platform uses a zip archive (Windows) vs tarball.
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Archive extension for the current platform.
#[cfg(test)]
fn archive_ext() -> &'static str {
    if is_windows() {
        "zip"
    } else {
        "tar.gz"
    }
}

// ---------------------------------------------------------------------------
// Shared version regex helpers
// ---------------------------------------------------------------------------

/// Parse a version string from `java -version` stderr output.
///
/// Example output:
/// ```text
/// openjdk version "21.0.4" 2024-07-16 LTS
/// ```
fn parse_java_version(output: &str) -> Option<String> {
    let re = Regex::new(r#"version "(\d+[\d.]*\d+)""#).ok()?;
    re.captures(output).map(|c| c[1].to_string())
}

/// Parse a version string from `gradle --version` output.
///
/// Example output:
/// ```text
/// Gradle 8.10.2
/// ```
fn parse_gradle_version(output: &str) -> Option<String> {
    let re = Regex::new(r"Gradle (\d+\.\d+[\.\d]*)").ok()?;
    re.captures(output).map(|c| c[1].to_string())
}

/// Extract the major version number from a version string like `"21.0.4"` or `"21"`.
fn major_version(version: &str) -> Option<u32> {
    version.split('.').next()?.parse().ok()
}

/// Strip the Adoptium build metadata suffix (e.g. `"+7"`) from a version string.
///
/// `"21.0.4+7"` becomes `"21.0.4"`, `"21.0.4"` stays `"21.0.4"`.
fn strip_build_metadata(version: &str) -> &str {
    version.split('+').next().unwrap_or(version)
}

// ---------------------------------------------------------------------------
// JavaProvider
// ---------------------------------------------------------------------------

/// Provider for Java (Eclipse Temurin) — downloads pre-built JDK binaries from
/// the Adoptium API.
pub struct JavaProvider {
    client: reqwest::Client,
    /// In-memory cache for resolved version lists (24-hour TTL).
    version_cache: tokio::sync::Mutex<Option<CachedVersions>>,
}

impl JavaProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            version_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/java/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/java")
    }

    // -- version fetching ---------------------------------------------------

    /// Fetch available major release versions from Adoptium.
    async fn fetch_available_releases(&self) -> Result<AvailableReleases, ToolError> {
        let url = "https://api.adoptium.net/v3/info/available_releases";
        debug!(url = %url, "fetching available Java releases");

        let response = self
            .client
            .get(url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "java".into(),
                reason: format!("failed to fetch available releases: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "java".into(),
                reason: format!(
                    "available releases returned HTTP {}",
                    response.status().as_u16()
                ),
            });
        }

        response
            .json()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "java".into(),
                reason: format!("failed to parse available releases: {e}"),
            })
    }

    /// Fetch GA release versions for a specific major version from Adoptium.
    ///
    /// Returns versions newest-first.
    async fn fetch_versions_for_major(&self, major: u32) -> Result<Vec<String>, ToolError> {
        let url = format!(
            "https://api.adoptium.net/v3/info/release_versions?release_type=ga&page_size=50&project=jdk&version=%5B{major}%2C{}%29",
            major + 1
        );
        debug!(url = %url, major = major, "fetching Java versions for major");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "java".into(),
                reason: format!("failed to fetch versions for Java {major}: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "java".into(),
                reason: format!(
                    "version listing for Java {major} returned HTTP {}",
                    response.status().as_u16()
                ),
            });
        }

        let body: ReleaseVersionsResponse =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "java".into(),
                    reason: format!("failed to parse version list for Java {major}: {e}"),
                })?;

        let versions: Vec<String> = body
            .versions
            .into_iter()
            .map(|v| strip_build_metadata(&v.semver).to_string())
            .collect();

        Ok(versions)
    }

    /// Resolve a (possibly partial) version request to a full version string.
    ///
    /// - `"21"` → latest `21.x.y` GA release
    /// - `"21.0.4"` → exact match `"21.0.4"`
    async fn resolve_version(&self, requested: &str) -> Result<String, ToolError> {
        let requested = requested.trim_start_matches('v');

        let major = major_version(requested).ok_or_else(|| ToolError::VersionNotAvailable {
            tool: "java".into(),
            version: requested.to_string(),
        })?;

        // Check cache first
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);
        {
            let guard = self.version_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    if let Some((_, versions)) = cached.entries.iter().find(|(m, _)| *m == major) {
                        for v in versions {
                            if version_satisfies(v, requested) {
                                debug!(requested = %requested, resolved = %v, "resolved Java version from cache");
                                return Ok(v.clone());
                            }
                        }
                    }
                }
            }
        }

        // Fetch from API
        let versions = self.fetch_versions_for_major(major).await?;

        // Find the matching version before consuming the list for caching
        let matched = versions
            .iter()
            .find(|v| version_satisfies(v, requested))
            .cloned();

        // Update cache
        {
            let mut guard = self.version_cache.lock().await;
            if let Some(ref mut cached) = *guard {
                if let Some(entry) = cached.entries.iter_mut().find(|(m, _)| *m == major) {
                    entry.1 = versions;
                } else {
                    cached.entries.push((major, versions));
                }
            } else {
                *guard = Some(CachedVersions {
                    entries: vec![(major, versions)],
                    fetched_at: std::time::Instant::now(),
                });
            }
        }

        match matched {
            Some(v) => {
                debug!(requested = %requested, resolved = %v, "resolved Java version");
                Ok(v)
            }
            None => Err(ToolError::VersionNotAvailable {
                tool: "java".into(),
                version: requested.to_string(),
            }),
        }
    }

    // -- download & extract -------------------------------------------------

    /// Build the download URL for a fully resolved version.
    ///
    /// Uses the Adoptium "latest" endpoint which redirects to the actual binary.
    fn download_url(version: &str) -> Result<String, ToolError> {
        let os = adoptium_os()?;
        let arch = adoptium_arch()?;
        Ok(format!(
            "https://api.adoptium.net/v3/binary/version/jdk-{version}/{os}/{arch}/jdk/hotspot/normal/eclipse?project=jdk"
        ))
    }

    /// Build the download URL using just the major (feature) version to get the
    /// latest GA release in that line.
    #[cfg(test)]
    fn download_url_latest(major: u32) -> Result<String, ToolError> {
        let os = adoptium_os()?;
        let arch = adoptium_arch()?;
        Ok(format!(
            "https://api.adoptium.net/v3/binary/latest/{major}/ga/{os}/{arch}/jdk/hotspot/normal/eclipse"
        ))
    }

    /// Determine the JDK home directory inside the extraction directory.
    ///
    /// On macOS the layout is: `jdk-{version}/Contents/Home/`
    /// On Linux/Windows it is: `jdk-{version}/`
    fn jdk_home(extract_dir: &Path, version: &str) -> PathBuf {
        // The extracted directory name includes the full Adoptium version with
        // build metadata, e.g. `jdk-21.0.4+7`. We search for a directory
        // starting with `jdk-{version}` to tolerate the build suffix.
        let prefix = format!("jdk-{version}");

        // Try to find the actual directory name
        if let Ok(entries) = std::fs::read_dir(extract_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    if let Some(name) = path.file_name() {
                        let name_str = name.to_string_lossy();
                        if name_str.starts_with(&prefix) {
                            return if is_macos() {
                                path.join("Contents").join("Home")
                            } else {
                                path
                            };
                        }
                    }
                }
            }
        }

        // Fallback: assume exact name
        let jdk_dir = extract_dir.join(&prefix);
        if is_macos() {
            jdk_dir.join("Contents").join("Home")
        } else {
            jdk_dir
        }
    }

    /// Download and extract the JDK into `dest_dir`.
    ///
    /// Returns the path to JAVA_HOME (the JDK root containing `bin/`).
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = Self::download_url(version)?;
        info!(version = %version, url = %url, "downloading Java (Temurin)");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "java".into(),
                version: version.into(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "java".into(),
                version: version.into(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "java".into(),
                version: version.into(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        if is_windows() {
            Self::extract_zip(&bytes, version, dest_dir)?;
        } else {
            Self::extract_tarball(&bytes, version, dest_dir)?;
        }

        let java_home = Self::jdk_home(dest_dir, version);
        let bin_dir = java_home.join("bin");

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "java".into(),
                version: version.into(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(java_home = %java_home.display(), "Java extracted successfully");
        Ok(java_home)
    }

    /// Extract a `.tar.gz` archive into `dest_dir`.
    fn extract_tarball(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let decoder = flate2::read::GzDecoder::new(data);
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "java".into(),
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
                tool: "java".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "java".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        Ok(())
    }
}

impl Default for JavaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for JavaProvider {
    fn id(&self) -> &'static str {
        "java"
    }

    fn name(&self) -> &'static str {
        "Java (Temurin)"
    }

    fn binary_name(&self) -> &'static str {
        "java"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        // `java -version` writes to stderr
        let output = tokio::process::Command::new("java")
            .arg("-version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                if let Some(version) = parse_java_version(&stderr) {
                    debug!(version = %version, "detected Java on PATH");
                    return Ok(Some(version));
                }
                // Some JVMs write to stdout instead
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(version) = parse_java_version(&stdout) {
                    debug!(version = %version, "detected Java on PATH (stdout)");
                    return Ok(Some(version));
                }
                debug!("java -version output did not match expected pattern");
                Ok(None)
            }
            Ok(_) => {
                debug!("java -version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("java not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `java -version`: {e}"
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
        let java_home = Self::jdk_home(cache_dir, &resolved);
        let bin_dir = java_home.join("bin");
        if bin_dir.exists() {
            info!(version = %resolved, "Java already installed in cache");
            return Ok(InstallResult {
                tool: "java".into(),
                version: resolved,
                install_path: java_home,
            });
        }

        let java_home = self.download_and_extract(&resolved, cache_dir).await?;

        info!(version = %resolved, path = %java_home.display(), "Java installed");
        Ok(InstallResult {
            tool: "java".into(),
            version: resolved,
            install_path: java_home,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let releases = self.fetch_available_releases().await?;
        let mut all_versions = Vec::new();

        for major in releases.available_releases.iter().rev() {
            match self.fetch_versions_for_major(*major).await {
                Ok(versions) => all_versions.extend(versions),
                Err(e) => {
                    debug!(major = %major, error = %e, "failed to fetch versions for major, skipping");
                }
            }
        }

        Ok(all_versions)
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        let java_home = install_path.display().to_string();
        let bin_dir = install_path.join("bin");
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            bin_dir.display().to_string()
        } else {
            format!("{}:{path}", bin_dir.display())
        };
        vec![("JAVA_HOME".into(), java_home), ("PATH".into(), new_path)]
    }
}

// ---------------------------------------------------------------------------
// GradleProvider
// ---------------------------------------------------------------------------

/// Provider for Gradle — downloads distributions from services.gradle.org.
pub struct GradleProvider {
    client: reqwest::Client,
    /// In-memory cache for the version list (24-hour TTL).
    version_cache: tokio::sync::Mutex<Option<CachedGradleVersions>>,
}

impl GradleProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            version_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/gradle/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/gradle")
    }

    // -- version fetching ---------------------------------------------------

    /// Fetch the Gradle version list from services.gradle.org, with 24-hour caching.
    async fn fetch_versions(&self) -> Result<Vec<GradleVersionEntry>, ToolError> {
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);

        {
            let guard = self.version_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    debug!("using cached Gradle version list");
                    return Ok(cached.entries.clone());
                }
            }
        }

        info!("fetching Gradle version list from services.gradle.org");
        let response = self
            .client
            .get("https://services.gradle.org/versions/all")
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "gradle".into(),
                reason: format!("failed to fetch version list: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "gradle".into(),
                reason: format!("version list returned HTTP {}", response.status().as_u16()),
            });
        }

        let entries: Vec<GradleVersionEntry> =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "gradle".into(),
                    reason: format!("failed to parse version list: {e}"),
                })?;

        // Store in cache
        {
            let mut guard = self.version_cache.lock().await;
            *guard = Some(CachedGradleVersions {
                entries: entries.clone(),
                fetched_at: std::time::Instant::now(),
            });
        }

        Ok(entries)
    }

    /// Resolve a (possibly partial) version request to a full Gradle version.
    ///
    /// Only GA (non-snapshot) versions are considered.
    async fn resolve_version(&self, requested: &str) -> Result<String, ToolError> {
        let requested = requested.trim_start_matches('v');
        let entries = self.fetch_versions().await?;

        for entry in &entries {
            if entry.snapshot {
                continue;
            }
            if version_satisfies(&entry.version, requested) {
                debug!(requested = %requested, resolved = %entry.version, "resolved Gradle version");
                return Ok(entry.version.clone());
            }
        }

        Err(ToolError::VersionNotAvailable {
            tool: "gradle".into(),
            version: requested.to_string(),
        })
    }

    // -- download & extract -------------------------------------------------

    /// Build the download URL for a fully resolved Gradle version.
    fn download_url(version: &str) -> String {
        format!("https://services.gradle.org/distributions/gradle-{version}-bin.zip")
    }

    /// Download and extract Gradle into `dest_dir`.
    ///
    /// Returns the path to GRADLE_HOME (the root containing `bin/`).
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = Self::download_url(version);
        info!(version = %version, url = %url, "downloading Gradle");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        // Gradle always distributes as .zip
        Self::extract_zip(&bytes, version, dest_dir)?;

        let gradle_home = dest_dir.join(format!("gradle-{version}"));
        let bin_dir = gradle_home.join("bin");

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(gradle_home = %gradle_home.display(), "Gradle extracted successfully");
        Ok(gradle_home)
    }

    /// Extract a `.zip` archive into `dest_dir`.
    fn extract_zip(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let cursor = std::io::Cursor::new(data);
        let mut archive =
            zip::ZipArchive::new(cursor).map_err(|e| ToolError::ExtractionFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "gradle".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        Ok(())
    }
}

impl Default for GradleProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for GradleProvider {
    fn id(&self) -> &'static str {
        "gradle"
    }

    fn name(&self) -> &'static str {
        "Gradle"
    }

    fn binary_name(&self) -> &'static str {
        "gradle"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let output = tokio::process::Command::new("gradle")
            .arg("--version")
            .output()
            .await;

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(version) = parse_gradle_version(&stdout) {
                    debug!(version = %version, "detected Gradle on PATH");
                    return Ok(Some(version));
                }
                debug!("gradle --version output did not match expected pattern");
                Ok(None)
            }
            Ok(_) => {
                debug!("gradle --version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("gradle not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `gradle --version`: {e}"
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
        let gradle_home = cache_dir.join(format!("gradle-{resolved}"));
        let bin_dir = gradle_home.join("bin");
        if bin_dir.exists() {
            info!(version = %resolved, "Gradle already installed in cache");
            return Ok(InstallResult {
                tool: "gradle".into(),
                version: resolved,
                install_path: gradle_home,
            });
        }

        let gradle_home = self.download_and_extract(&resolved, cache_dir).await?;

        info!(version = %resolved, path = %gradle_home.display(), "Gradle installed");
        Ok(InstallResult {
            tool: "gradle".into(),
            version: resolved,
            install_path: gradle_home,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let entries = self.fetch_versions().await?;

        let versions: Vec<String> = entries
            .into_iter()
            .filter(|e| !e.snapshot)
            .map(|e| e.version)
            .collect();

        Ok(versions)
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        let gradle_home = install_path.display().to_string();
        let bin_dir = install_path.join("bin");
        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            bin_dir.display().to_string()
        } else {
            format!("{}:{path}", bin_dir.display())
        };
        vec![
            ("GRADLE_HOME".into(), gradle_home),
            ("PATH".into(), new_path),
        ]
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // == JavaProvider basic properties ========================================

    #[test]
    fn java_provider_id() {
        let provider = JavaProvider::new();
        assert_eq!(provider.id(), "java");
    }

    #[test]
    fn java_provider_name() {
        let provider = JavaProvider::new();
        assert_eq!(provider.name(), "Java (Temurin)");
    }

    #[test]
    fn java_provider_binary_name() {
        let provider = JavaProvider::new();
        assert_eq!(provider.binary_name(), "java");
    }

    // == GradleProvider basic properties =====================================

    #[test]
    fn gradle_provider_id() {
        let provider = GradleProvider::new();
        assert_eq!(provider.id(), "gradle");
    }

    #[test]
    fn gradle_provider_name() {
        let provider = GradleProvider::new();
        assert_eq!(provider.name(), "Gradle");
    }

    #[test]
    fn gradle_provider_binary_name() {
        let provider = GradleProvider::new();
        assert_eq!(provider.binary_name(), "gradle");
    }

    // == Version regex: java -version ========================================

    #[test]
    fn parse_java_version_temurin_21() {
        let output = r#"openjdk version "21.0.4" 2024-07-16 LTS
OpenJDK Runtime Environment Temurin-21.0.4+7 (build 21.0.4+7-LTS)
OpenJDK 64-Bit Server VM Temurin-21.0.4+7 (build 21.0.4+7-LTS, mixed mode, sharing)"#;
        assert_eq!(parse_java_version(output), Some("21.0.4".to_string()));
    }

    #[test]
    fn parse_java_version_temurin_17() {
        let output = r#"openjdk version "17.0.12" 2024-07-16
OpenJDK Runtime Environment Temurin-17.0.12+7 (build 17.0.12+7)
OpenJDK 64-Bit Server VM Temurin-17.0.12+7 (build 17.0.12+7, mixed mode, sharing)"#;
        assert_eq!(parse_java_version(output), Some("17.0.12".to_string()));
    }

    #[test]
    fn parse_java_version_temurin_11() {
        let output = r#"openjdk version "11.0.24" 2024-07-16
OpenJDK Runtime Environment Temurin-11.0.24+8 (build 11.0.24+8)
OpenJDK 64-Bit Server VM Temurin-11.0.24+8 (build 11.0.24+8, mixed mode)"#;
        assert_eq!(parse_java_version(output), Some("11.0.24".to_string()));
    }

    #[test]
    fn parse_java_version_oracle_jdk() {
        let output = r#"java version "22.0.2" 2024-07-16
Java(TM) SE Runtime Environment (build 22.0.2+9-70)
Java HotSpot(TM) 64-Bit Server VM (build 22.0.2+9-70, mixed mode, sharing)"#;
        assert_eq!(parse_java_version(output), Some("22.0.2".to_string()));
    }

    #[test]
    fn parse_java_version_no_match() {
        assert_eq!(parse_java_version("not a java version"), None);
    }

    #[test]
    fn parse_java_version_empty() {
        assert_eq!(parse_java_version(""), None);
    }

    // == Version regex: gradle --version =====================================

    #[test]
    fn parse_gradle_version_8() {
        let output = r#"
------------------------------------------------------------
Gradle 8.10.2
------------------------------------------------------------

Build time:    2024-09-23 21:28:39 UTC
Revision:      415adb9e07a2a1cfb31bc4df6d80a52ebb283bdd
"#;
        assert_eq!(parse_gradle_version(output), Some("8.10.2".to_string()));
    }

    #[test]
    fn parse_gradle_version_7() {
        let output = r#"
------------------------------------------------------------
Gradle 7.6.4
------------------------------------------------------------
"#;
        assert_eq!(parse_gradle_version(output), Some("7.6.4".to_string()));
    }

    #[test]
    fn parse_gradle_version_major_minor_only() {
        let output = "Gradle 8.5";
        assert_eq!(parse_gradle_version(output), Some("8.5".to_string()));
    }

    #[test]
    fn parse_gradle_version_no_match() {
        assert_eq!(parse_gradle_version("not gradle output"), None);
    }

    // == Platform helpers ====================================================

    #[test]
    fn adoptium_os_returns_known_value() {
        let os = adoptium_os().unwrap();
        assert!(
            ["mac", "linux", "windows"].contains(&os),
            "unexpected OS: {os}"
        );
    }

    #[test]
    fn adoptium_arch_returns_known_value() {
        let arch = adoptium_arch().unwrap();
        assert!(
            ["aarch64", "x64"].contains(&arch),
            "unexpected arch: {arch}"
        );
    }

    #[test]
    fn archive_ext_matches_platform() {
        let ext = archive_ext();
        if is_windows() {
            assert_eq!(ext, "zip");
        } else {
            assert_eq!(ext, "tar.gz");
        }
    }

    // == Download URL construction ===========================================

    #[test]
    fn java_download_url_specific_version() {
        let url = JavaProvider::download_url("21.0.4").unwrap();
        assert!(
            url.contains("jdk-21.0.4"),
            "URL should contain version: {url}"
        );
        assert!(url.contains("hotspot"), "URL should contain hotspot: {url}");
        assert!(
            url.starts_with("https://api.adoptium.net/v3/binary/version/"),
            "URL should use version endpoint: {url}"
        );
        let os = adoptium_os().unwrap();
        let arch = adoptium_arch().unwrap();
        assert!(url.contains(os), "URL should contain OS '{os}': {url}");
        assert!(
            url.contains(arch),
            "URL should contain arch '{arch}': {url}"
        );
    }

    #[test]
    fn java_download_url_latest_major() {
        let url = JavaProvider::download_url_latest(21).unwrap();
        assert_eq!(
            url,
            format!(
                "https://api.adoptium.net/v3/binary/latest/21/ga/{}/{}/jdk/hotspot/normal/eclipse",
                adoptium_os().unwrap(),
                adoptium_arch().unwrap(),
            )
        );
    }

    #[test]
    fn gradle_download_url() {
        let url = GradleProvider::download_url("8.10.2");
        assert_eq!(
            url,
            "https://services.gradle.org/distributions/gradle-8.10.2-bin.zip"
        );
    }

    // == Version matching ====================================================

    #[test]
    fn java_version_matching_major() {
        assert!(version_satisfies("21.0.4", "21"));
        assert!(version_satisfies("21.0.0", "21"));
        assert!(!version_satisfies("22.0.1", "21"));
    }

    #[test]
    fn java_version_matching_minor() {
        assert!(version_satisfies("21.0.4", "21.0"));
        assert!(version_satisfies("21.0.0", "21.0"));
        assert!(!version_satisfies("21.1.0", "21.0"));
    }

    #[test]
    fn java_version_matching_exact() {
        assert!(version_satisfies("21.0.4", "21.0.4"));
        assert!(!version_satisfies("21.0.3", "21.0.4"));
    }

    #[test]
    fn gradle_version_matching() {
        assert!(version_satisfies("8.10.2", "8"));
        assert!(version_satisfies("8.10.2", "8.10"));
        assert!(version_satisfies("8.10.2", "8.10.2"));
        assert!(!version_satisfies("8.10.2", "7"));
        assert!(!version_satisfies("8.10.2", "8.9"));
    }

    // == Version resolution (unit, no network) ===============================

    #[test]
    fn resolve_java_version_from_list() {
        // Simulate the resolve logic with a pre-built list (newest first)
        let versions = [
            "21.0.4".to_string(),
            "21.0.3".to_string(),
            "21.0.2".to_string(),
            "21.0.1".to_string(),
        ];

        // "21" should resolve to 21.0.4 (newest)
        let resolved = versions
            .iter()
            .find(|v| version_satisfies(v, "21"))
            .cloned();
        assert_eq!(resolved, Some("21.0.4".to_string()));

        // "21.0.2" should resolve to exact
        let resolved = versions
            .iter()
            .find(|v| version_satisfies(v, "21.0.2"))
            .cloned();
        assert_eq!(resolved, Some("21.0.2".to_string()));

        // "22" should resolve to nothing
        let resolved = versions
            .iter()
            .find(|v| version_satisfies(v, "22"))
            .cloned();
        assert_eq!(resolved, None);
    }

    #[test]
    fn resolve_gradle_version_from_list() {
        let entries = vec![
            GradleVersionEntry {
                version: "8.11".to_string(),
                snapshot: true,
                active_rc: false,
                rc_for: String::new(),
                broken: false,
            },
            GradleVersionEntry {
                version: "8.10.2".to_string(),
                snapshot: false,
                active_rc: false,
                rc_for: String::new(),
                broken: false,
            },
            GradleVersionEntry {
                version: "8.10.1".to_string(),
                snapshot: false,
                active_rc: false,
                rc_for: String::new(),
                broken: false,
            },
            GradleVersionEntry {
                version: "7.6.4".to_string(),
                snapshot: false,
                active_rc: false,
                rc_for: String::new(),
                broken: false,
            },
        ];

        // "8" should skip snapshot and resolve to 8.10.2
        let resolved = entries
            .iter()
            .filter(|e| !e.snapshot)
            .find(|e| version_satisfies(&e.version, "8"))
            .map(|e| e.version.clone());
        assert_eq!(resolved, Some("8.10.2".to_string()));

        // "7" should resolve to 7.6.4
        let resolved = entries
            .iter()
            .filter(|e| !e.snapshot)
            .find(|e| version_satisfies(&e.version, "7"))
            .map(|e| e.version.clone());
        assert_eq!(resolved, Some("7.6.4".to_string()));

        // "9" should not match
        let resolved = entries
            .iter()
            .filter(|e| !e.snapshot)
            .find(|e| version_satisfies(&e.version, "9"))
            .map(|e| e.version.clone());
        assert_eq!(resolved, None);
    }

    // == Helper functions ====================================================

    #[test]
    fn major_version_parsing() {
        assert_eq!(major_version("21"), Some(21));
        assert_eq!(major_version("21.0.4"), Some(21));
        assert_eq!(major_version("8"), Some(8));
        assert_eq!(major_version(""), None);
        assert_eq!(major_version("abc"), None);
    }

    #[test]
    fn strip_build_metadata_works() {
        assert_eq!(strip_build_metadata("21.0.4+7"), "21.0.4");
        assert_eq!(strip_build_metadata("17.0.12+8"), "17.0.12");
        assert_eq!(strip_build_metadata("21.0.4"), "21.0.4");
        assert_eq!(strip_build_metadata("11.0.24+8"), "11.0.24");
    }

    // == env_vars ============================================================

    #[test]
    fn java_env_vars_sets_java_home_and_path() {
        let provider = JavaProvider::new();
        let java_home = Path::new("/home/user/.canaveral/tools/java/21.0.4/jdk-21.0.4+7");
        let vars = provider.env_vars(java_home);
        assert_eq!(vars.len(), 2);

        assert_eq!(vars[0].0, "JAVA_HOME");
        assert_eq!(vars[0].1, java_home.display().to_string());

        assert_eq!(vars[1].0, "PATH");
        let expected_bin = java_home.join("bin").display().to_string();
        assert!(
            vars[1].1.starts_with(&expected_bin),
            "PATH should start with bin dir: {}",
            vars[1].1
        );
    }

    #[test]
    fn gradle_env_vars_sets_gradle_home_and_path() {
        let provider = GradleProvider::new();
        let gradle_home = Path::new("/home/user/.canaveral/tools/gradle/8.10.2/gradle-8.10.2");
        let vars = provider.env_vars(gradle_home);
        assert_eq!(vars.len(), 2);

        assert_eq!(vars[0].0, "GRADLE_HOME");
        assert_eq!(vars[0].1, gradle_home.display().to_string());

        assert_eq!(vars[1].0, "PATH");
        let expected_bin = gradle_home.join("bin").display().to_string();
        assert!(
            vars[1].1.starts_with(&expected_bin),
            "PATH should start with bin dir: {}",
            vars[1].1
        );
    }

    // == JDK home directory layout ===========================================

    #[test]
    fn jdk_home_macos_layout() {
        // On macOS, jdk_home should add Contents/Home
        let extract_dir = Path::new("/tmp/java");
        let home = JavaProvider::jdk_home(extract_dir, "21.0.4");
        if is_macos() {
            assert!(
                home.to_string_lossy().contains("Contents/Home"),
                "macOS JDK home should contain Contents/Home: {}",
                home.display()
            );
        }
    }

    #[test]
    fn jdk_home_fallback_path() {
        // When the directory doesn't exist, jdk_home falls back to the constructed path
        let extract_dir = Path::new("/nonexistent/path");
        let home = JavaProvider::jdk_home(extract_dir, "21.0.4");
        if is_macos() {
            assert_eq!(
                home,
                extract_dir.join("jdk-21.0.4").join("Contents").join("Home")
            );
        } else {
            assert_eq!(home, extract_dir.join("jdk-21.0.4"));
        }
    }

    // == Adoptium API response parsing =======================================

    #[test]
    fn parse_available_releases_response() {
        let json = r#"{
            "available_releases": [8, 11, 17, 21, 22, 23],
            "available_lts_releases": [8, 11, 17, 21],
            "most_recent_lts": 21,
            "most_recent_feature_release": 23,
            "most_recent_feature_version": 24,
            "tip_version": 25
        }"#;
        let parsed: AvailableReleases = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.available_releases, vec![8, 11, 17, 21, 22, 23]);
        assert_eq!(parsed.most_recent_lts, 21);
        assert_eq!(parsed.most_recent_feature_release, 23);
    }

    #[test]
    fn parse_release_versions_response() {
        let json = r#"{
            "versions": [
                {"semver": "21.0.4+7"},
                {"semver": "21.0.3+9"},
                {"semver": "21.0.2+13"}
            ]
        }"#;
        let parsed: ReleaseVersionsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.versions.len(), 3);
        assert_eq!(parsed.versions[0].semver, "21.0.4+7");
        assert_eq!(strip_build_metadata(&parsed.versions[0].semver), "21.0.4");
    }

    #[test]
    fn parse_gradle_version_entries() {
        let json = r#"[
            {"version": "8.11-20241001120000+0000", "snapshot": true, "activeRc": false, "rcFor": "", "broken": false},
            {"version": "8.10.2", "snapshot": false, "activeRc": false, "rcFor": "", "broken": false}
        ]"#;
        let entries: Vec<GradleVersionEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert!(entries[0].snapshot);
        assert!(!entries[1].snapshot);
        assert_eq!(entries[1].version, "8.10.2");
    }

    // == Default cache root ==================================================

    #[test]
    fn java_cache_root_contains_java() {
        let root = JavaProvider::default_cache_root();
        assert!(
            root.to_string_lossy().contains("java"),
            "cache root should contain 'java': {}",
            root.display()
        );
        assert!(
            root.to_string_lossy().contains(".canaveral/tools"),
            "cache root should be under .canaveral/tools: {}",
            root.display()
        );
    }

    #[test]
    fn gradle_cache_root_contains_gradle() {
        let root = GradleProvider::default_cache_root();
        assert!(
            root.to_string_lossy().contains("gradle"),
            "cache root should contain 'gradle': {}",
            root.display()
        );
        assert!(
            root.to_string_lossy().contains(".canaveral/tools"),
            "cache root should be under .canaveral/tools: {}",
            root.display()
        );
    }
}
