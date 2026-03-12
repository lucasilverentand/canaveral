//! Go toolchain provider
//!
//! Downloads and installs pre-built Go toolchains directly from
//! <https://go.dev/dl/>.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use regex::Regex;
use serde::Deserialize;
use tracing::{debug, info};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// Go version index types
// ---------------------------------------------------------------------------

/// A single entry from <https://go.dev/dl/?mode=json>.
#[derive(Debug, Clone, Deserialize)]
struct GoVersionEntry {
    /// Version string including the `go` prefix, e.g. `"go1.22.0"`.
    version: String,
    /// Whether this is a stable release.
    stable: bool,
}

/// Cached version index with a TTL.
struct CachedIndex {
    entries: Vec<GoVersionEntry>,
    fetched_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// Platform helpers
// ---------------------------------------------------------------------------

/// Returns the Go platform string for the current OS + arch combination.
///
/// Go uses e.g. `darwin-arm64`, `linux-amd64`, `windows-amd64`.
fn go_platform() -> Result<&'static str, ToolError> {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        Ok("darwin-arm64")
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        Ok("darwin-amd64")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        Ok("linux-amd64")
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        Ok("linux-arm64")
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        Ok("windows-amd64")
    } else {
        Err(ToolError::UnsupportedPlatform(
            "go: unsupported OS/architecture combination".to_string(),
        ))
    }
}

/// Whether the current platform uses a zip archive (Windows) vs tarball.
fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Returns the archive extension for the current platform.
fn archive_ext() -> &'static str {
    if is_windows() {
        "zip"
    } else {
        "tar.gz"
    }
}

// ---------------------------------------------------------------------------
// GoProvider
// ---------------------------------------------------------------------------

/// Provider for Go — downloads pre-built toolchains from go.dev.
pub struct GoProvider {
    client: reqwest::Client,
    /// In-memory cache for the version index (24-hour TTL).
    index_cache: tokio::sync::Mutex<Option<CachedIndex>>,
}

impl GoProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            index_cache: tokio::sync::Mutex::new(None),
        }
    }

    /// Default cache root: `~/.canaveral/tools/go/`
    fn default_cache_root() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".canaveral/tools/go")
    }

    // -- version index ------------------------------------------------------

    /// Fetch the Go version index, using an in-memory 24-hour cache.
    async fn fetch_index(&self) -> Result<Vec<GoVersionEntry>, ToolError> {
        let ttl = std::time::Duration::from_secs(24 * 60 * 60);

        {
            let guard = self.index_cache.lock().await;
            if let Some(ref cached) = *guard {
                if cached.fetched_at.elapsed() < ttl {
                    debug!("using cached Go version index");
                    return Ok(cached.entries.clone());
                }
            }
        }

        info!("fetching Go version index from go.dev");
        let response = self
            .client
            .get("https://go.dev/dl/?mode=json")
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::RegistryFetchFailed {
                tool: "go".into(),
                reason: format!("failed to fetch version index: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::RegistryFetchFailed {
                tool: "go".into(),
                reason: format!("version index returned HTTP {}", response.status().as_u16()),
            });
        }

        let entries: Vec<GoVersionEntry> =
            response
                .json()
                .await
                .map_err(|e| ToolError::RegistryFetchFailed {
                    tool: "go".into(),
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
    /// For example, `"1.22"` resolves to the newest `1.22.x`.
    async fn resolve_version(&self, requested: &str) -> Result<String, ToolError> {
        let requested = requested.trim_start_matches('v');
        let entries = self.fetch_index().await?;

        for entry in &entries {
            if !entry.stable {
                continue;
            }
            let v = entry.version.strip_prefix("go").unwrap_or(&entry.version);
            if version_satisfies(v, requested) {
                debug!(requested = %requested, resolved = %v, "resolved Go version");
                return Ok(v.to_string());
            }
        }

        Err(ToolError::VersionNotAvailable {
            tool: "go".into(),
            version: requested.to_string(),
        })
    }

    // -- download & extract -------------------------------------------------

    /// Build the download URL for a fully resolved version.
    fn download_url(version: &str) -> Result<String, ToolError> {
        let platform = go_platform()?;
        let ext = archive_ext();
        Ok(format!("https://go.dev/dl/go{version}.{platform}.{ext}"))
    }

    /// Download and extract Go into `dest_dir`.
    ///
    /// Returns the path to the `go/bin/` directory inside the extracted tree.
    async fn download_and_extract(
        &self,
        version: &str,
        dest_dir: &Path,
    ) -> Result<PathBuf, ToolError> {
        let url = Self::download_url(version)?;
        info!(version = %version, url = %url, "downloading Go");

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "canaveral")
            .send()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "go".into(),
                version: version.into(),
                reason: format!("download failed: {e}"),
            })?;

        if !response.status().is_success() {
            return Err(ToolError::InstallFailed {
                tool: "go".into(),
                version: version.into(),
                reason: format!("download returned HTTP {}", response.status().as_u16()),
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| ToolError::InstallFailed {
                tool: "go".into(),
                version: version.into(),
                reason: format!("failed to read response body: {e}"),
            })?;

        std::fs::create_dir_all(dest_dir)?;

        if is_windows() {
            Self::extract_zip(&bytes, version, dest_dir)?;
        } else {
            Self::extract_tarball(&bytes, version, dest_dir)?;
        }

        // The archive extracts to `go/` inside dest_dir.
        let go_dir = dest_dir.join("go");
        let bin_dir = go_dir.join("bin");

        if !bin_dir.exists() {
            return Err(ToolError::ExtractionFailed {
                tool: "go".into(),
                version: version.into(),
                reason: format!("expected bin directory not found at {}", bin_dir.display()),
            });
        }

        debug!(bin_dir = %bin_dir.display(), "Go extracted successfully");
        Ok(bin_dir)
    }

    /// Extract a `.tar.gz` archive into `dest_dir`.
    fn extract_tarball(data: &[u8], version: &str, dest_dir: &Path) -> Result<(), ToolError> {
        let decoder = flate2::read::GzDecoder::new(data);
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "go".into(),
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
                tool: "go".into(),
                version: version.into(),
                reason: format!("failed to open zip archive: {e}"),
            })?;

        archive
            .extract(dest_dir)
            .map_err(|e| ToolError::ExtractionFailed {
                tool: "go".into(),
                version: version.into(),
                reason: format!("failed to extract zip archive: {e}"),
            })?;

        Ok(())
    }

    /// Regex for parsing `go version` output.
    ///
    /// Example: `go version go1.22.0 darwin/arm64`
    fn version_regex() -> Regex {
        Regex::new(r"go(\d+\.\d+\.\d+)").expect("go version regex is valid")
    }
}

impl Default for GoProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolProvider for GoProvider {
    fn id(&self) -> &'static str {
        "go"
    }

    fn name(&self) -> &'static str {
        "Go"
    }

    fn binary_name(&self) -> &'static str {
        "go"
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        let output = tokio::process::Command::new("go")
            .arg("version")
            .output()
            .await;

        let re = Self::version_regex();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if let Some(caps) = re.captures(&stdout) {
                    let version = caps[1].to_string();
                    debug!(version = %version, "detected Go on PATH");
                    return Ok(Some(version));
                }
                debug!(
                    "go version output did not match expected format: {}",
                    stdout.trim()
                );
                Ok(None)
            }
            Ok(_) => {
                debug!("go version returned non-zero exit status");
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("go not found on PATH");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `go version`: {e}"
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
        let expected_bin = cache_dir.join("go").join("bin");
        if expected_bin.exists() {
            info!(version = %resolved, "Go already installed in cache");
            return Ok(InstallResult {
                tool: "go".into(),
                version: resolved,
                install_path: expected_bin,
            });
        }

        let bin_dir = self.download_and_extract(&resolved, cache_dir).await?;

        info!(version = %resolved, path = %bin_dir.display(), "Go installed");
        Ok(InstallResult {
            tool: "go".into(),
            version: resolved,
            install_path: bin_dir,
        })
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        let entries = self.fetch_index().await?;

        let versions: Vec<String> = entries
            .into_iter()
            .filter(|e| e.stable)
            .map(|e| {
                e.version
                    .strip_prefix("go")
                    .unwrap_or(&e.version)
                    .to_string()
            })
            .collect();

        Ok(versions)
    }

    fn env_vars(&self, install_path: &Path) -> Vec<(String, String)> {
        // install_path points to `go/bin` — GOROOT is the parent (`go/`)
        let goroot = install_path.parent().unwrap_or(install_path).to_path_buf();

        let path = std::env::var("PATH").unwrap_or_default();
        let new_path = if path.is_empty() {
            install_path.display().to_string()
        } else {
            format!("{}:{path}", install_path.display())
        };

        vec![
            ("PATH".into(), new_path),
            ("GOROOT".into(), goroot.display().to_string()),
        ]
    }
}

/// Search for the `go` binary inside a cache version directory.
///
/// The directory structure is: `{version_dir}/go/bin/go`
#[allow(dead_code)]
fn find_go_binary_in_cache(version_dir: &Path) -> Option<PathBuf> {
    let bin = if cfg!(target_os = "windows") {
        version_dir.join("go").join("bin").join("go.exe")
    } else {
        version_dir.join("go").join("bin").join("go")
    };
    if bin.exists() {
        Some(bin)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GoProvider basic properties ----------------------------------------

    #[test]
    fn go_provider_id() {
        let provider = GoProvider::new();
        assert_eq!(provider.id(), "go");
    }

    #[test]
    fn go_provider_name() {
        let provider = GoProvider::new();
        assert_eq!(provider.name(), "Go");
    }

    #[test]
    fn go_provider_binary_name() {
        let provider = GoProvider::new();
        assert_eq!(provider.binary_name(), "go");
    }

    // -- version regex matching ---------------------------------------------

    #[test]
    fn version_regex_matches_standard_output() {
        let re = GoProvider::version_regex();
        let output = "go version go1.22.0 darwin/arm64";
        let caps = re.captures(output).unwrap();
        assert_eq!(&caps[1], "1.22.0");
    }

    #[test]
    fn version_regex_matches_linux_output() {
        let re = GoProvider::version_regex();
        let output = "go version go1.21.6 linux/amd64";
        let caps = re.captures(output).unwrap();
        assert_eq!(&caps[1], "1.21.6");
    }

    #[test]
    fn version_regex_matches_windows_output() {
        let re = GoProvider::version_regex();
        let output = "go version go1.23.4 windows/amd64";
        let caps = re.captures(output).unwrap();
        assert_eq!(&caps[1], "1.23.4");
    }

    #[test]
    fn version_regex_no_match_on_garbage() {
        let re = GoProvider::version_regex();
        assert!(re.captures("not a go version string").is_none());
    }

    // -- platform string mapping --------------------------------------------

    #[test]
    fn go_platform_returns_known_value() {
        let platform = go_platform().unwrap();
        assert!(
            [
                "darwin-arm64",
                "darwin-amd64",
                "linux-amd64",
                "linux-arm64",
                "windows-amd64",
            ]
            .contains(&platform),
            "unexpected platform: {platform}"
        );
    }

    // -- download URL construction ------------------------------------------

    #[test]
    fn download_url_format() {
        let url = GoProvider::download_url("1.22.0").unwrap();
        let platform = go_platform().unwrap();
        let ext = archive_ext();
        assert_eq!(url, format!("https://go.dev/dl/go1.22.0.{platform}.{ext}"));
    }

    #[test]
    fn download_url_contains_version() {
        let url = GoProvider::download_url("1.21.6").unwrap();
        assert!(
            url.contains("go1.21.6"),
            "URL should contain version: {url}"
        );
    }

    #[test]
    fn download_url_uses_go_dev() {
        let url = GoProvider::download_url("1.23.0").unwrap();
        assert!(
            url.starts_with("https://go.dev/dl/"),
            "URL should start with go.dev: {url}"
        );
    }

    // -- version list parsing from JSON -------------------------------------

    #[test]
    fn parse_version_index_entries() {
        let json = r#"[
            {"version": "go1.22.0", "stable": true, "files": []},
            {"version": "go1.22rc1", "stable": false, "files": []},
            {"version": "go1.21.6", "stable": true, "files": []}
        ]"#;
        let entries: Vec<GoVersionEntry> = serde_json::from_str(json).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].version, "go1.22.0");
        assert!(entries[0].stable);
        assert_eq!(entries[1].version, "go1.22rc1");
        assert!(!entries[1].stable);
        assert_eq!(entries[2].version, "go1.21.6");
        assert!(entries[2].stable);
    }

    #[test]
    fn parse_version_index_stable_only() {
        let json = r#"[
            {"version": "go1.22.0", "stable": true, "files": []},
            {"version": "go1.22rc1", "stable": false, "files": []},
            {"version": "go1.21.6", "stable": true, "files": []}
        ]"#;
        let entries: Vec<GoVersionEntry> = serde_json::from_str(json).unwrap();
        let stable: Vec<_> = entries
            .iter()
            .filter(|e| e.stable)
            .map(|e| e.version.strip_prefix("go").unwrap_or(&e.version))
            .collect();
        assert_eq!(stable, vec!["1.22.0", "1.21.6"]);
    }

    // -- version prefix resolution ------------------------------------------

    #[test]
    fn resolve_version_prefix_matching() {
        // Simulate what resolve_version does: walk the index (newest first)
        // and find the first stable entry whose version satisfies the prefix.
        let entries = [
            GoVersionEntry {
                version: "go1.22.2".into(),
                stable: true,
            },
            GoVersionEntry {
                version: "go1.22.1".into(),
                stable: true,
            },
            GoVersionEntry {
                version: "go1.22.0".into(),
                stable: true,
            },
            GoVersionEntry {
                version: "go1.21.6".into(),
                stable: true,
            },
            GoVersionEntry {
                version: "go1.20.14".into(),
                stable: true,
            },
        ];

        // "1.22" should resolve to 1.22.2 (newest 1.22.x)
        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "1.22")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, Some("1.22.2".to_string()));

        // "1" should resolve to 1.22.2 (newest 1.x.y)
        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "1")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, Some("1.22.2".to_string()));

        // "1.21" should resolve to 1.21.6
        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "1.21")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, Some("1.21.6".to_string()));

        // Exact version match
        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "1.22.1")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, Some("1.22.1".to_string()));

        // "2" should resolve to nothing
        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "2")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, None);
    }

    #[test]
    fn resolve_skips_unstable_versions() {
        let entries = [
            GoVersionEntry {
                version: "go1.23rc1".into(),
                stable: false,
            },
            GoVersionEntry {
                version: "go1.22.2".into(),
                stable: true,
            },
        ];

        let resolved = entries
            .iter()
            .filter(|e| e.stable)
            .find(|e| {
                let v = e.version.strip_prefix("go").unwrap_or(&e.version);
                version_satisfies(v, "1")
            })
            .map(|e| e.version.strip_prefix("go").unwrap().to_string());
        assert_eq!(resolved, Some("1.22.2".to_string()));
    }

    // -- env_vars -----------------------------------------------------------

    #[test]
    fn env_vars_includes_path_and_goroot() {
        let provider = GoProvider::new();
        let bin = Path::new("/home/user/.canaveral/tools/go/1.22.0/go/bin");
        let vars = provider.env_vars(bin);
        assert_eq!(vars.len(), 2);

        let path_var = vars.iter().find(|(k, _)| k == "PATH").unwrap();
        assert!(
            path_var.1.starts_with(&bin.display().to_string()),
            "PATH should start with bin dir: {}",
            path_var.1
        );

        let goroot_var = vars.iter().find(|(k, _)| k == "GOROOT").unwrap();
        assert_eq!(
            goroot_var.1, "/home/user/.canaveral/tools/go/1.22.0/go",
            "GOROOT should be parent of bin"
        );
    }

    #[test]
    fn env_vars_goroot_is_parent_of_install_path() {
        let provider = GoProvider::new();
        let bin = Path::new("/tmp/cache/go/bin");
        let vars = provider.env_vars(bin);
        let goroot = vars.iter().find(|(k, _)| k == "GOROOT").unwrap();
        assert_eq!(goroot.1, "/tmp/cache/go");
    }

    #[test]
    fn env_vars_path_prepends_bin_dir() {
        let provider = GoProvider::new();
        let bin = Path::new("/cache/go/1.22.0/go/bin");
        let vars = provider.env_vars(bin);
        let path_val = vars
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        assert!(
            path_val.starts_with("/cache/go/1.22.0/go/bin"),
            "PATH should be prepended with bin dir: {path_val}"
        );
    }

    // -- version stripping --------------------------------------------------

    #[test]
    fn go_prefix_stripped_from_version() {
        let raw = "go1.22.0";
        let stripped = raw.strip_prefix("go").unwrap_or(raw);
        assert_eq!(stripped, "1.22.0");
    }

    #[test]
    fn version_without_go_prefix_unchanged() {
        let raw = "1.22.0";
        let stripped = raw.strip_prefix("go").unwrap_or(raw);
        assert_eq!(stripped, "1.22.0");
    }

    // -- version matching ---------------------------------------------------

    #[test]
    fn go_version_matching() {
        assert!(version_satisfies("1.22.0", "1.22"));
        assert!(version_satisfies("1.22.0", "1.22.0"));
        assert!(version_satisfies("1.22.0", "1"));
        assert!(!version_satisfies("1.22.0", "1.21"));
        assert!(!version_satisfies("1.22.0", "2"));
    }

    // -- find_go_binary_in_cache --------------------------------------------

    #[test]
    fn find_go_binary_nonexistent_dir() {
        let result = find_go_binary_in_cache(Path::new("/nonexistent/path/xyz"));
        assert!(result.is_none());
    }

    // -- archive extension --------------------------------------------------

    #[test]
    fn archive_extension_for_current_platform() {
        let ext = archive_ext();
        if cfg!(target_os = "windows") {
            assert_eq!(ext, "zip");
        } else {
            assert_eq!(ext, "tar.gz");
        }
    }

    // -- GoVersionEntry deserialization with extra fields --------------------

    #[test]
    fn deserialize_entry_ignores_extra_fields() {
        let json = r#"{"version": "go1.22.0", "stable": true, "files": [{"filename": "go1.22.0.linux-amd64.tar.gz"}]}"#;
        let entry: GoVersionEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.version, "go1.22.0");
        assert!(entry.stable);
    }

    // -- default cache root -------------------------------------------------

    #[test]
    fn default_cache_root_ends_with_go() {
        let root = GoProvider::default_cache_root();
        assert!(
            root.ends_with(".canaveral/tools/go"),
            "cache root should end with .canaveral/tools/go: {}",
            root.display()
        );
    }
}
