//! Tool installation cache

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tracing::{debug, info};

use canaveral_core::config::ToolsCacheConfig;

use crate::error::ToolError;

/// An entry representing a cached tool version
#[derive(Debug, Clone)]
pub struct CachedVersion {
    pub version: String,
    pub size_bytes: u64,
    pub last_used: SystemTime,
    pub path: PathBuf,
}

/// Result of a prune operation
#[derive(Debug, Default)]
pub struct PruneResult {
    /// (tool, version) pairs that were removed
    pub removed: Vec<(String, String)>,
    pub freed_bytes: u64,
}

/// Overall cache status
#[derive(Debug, Default)]
pub struct CacheStatus {
    pub total_size: u64,
    pub entry_count: usize,
    pub tools: HashMap<String, Vec<CachedVersion>>,
}

/// Tool installation cache — stores downloaded/built tool versions under a
/// base directory with the layout `base_dir/<tool>/<version>/`.
#[derive(Debug, Clone)]
pub struct ToolCache {
    base_dir: PathBuf,
    max_age: Duration,
    max_size: Option<u64>,
}

impl ToolCache {
    /// Create a new ToolCache from config.
    pub fn new(config: &ToolsCacheConfig) -> Self {
        let max_age = Duration::from_secs(config.max_age_days * 24 * 60 * 60);
        let max_size = config.max_size.as_deref().and_then(parse_size);
        Self {
            base_dir: config.dir.clone(),
            max_age,
            max_size,
        }
    }

    /// Get the directory for a specific tool version.
    /// Does not check if the directory exists.
    pub fn version_dir(&self, tool: &str, version: &str) -> PathBuf {
        self.base_dir.join(tool).join(version)
    }

    /// Returns true if the tool version directory exists and has content.
    pub fn is_cached(&self, tool: &str, version: &str) -> bool {
        let dir = self.version_dir(tool, version);
        dir.exists() && dir.is_dir()
    }

    /// Update the `.last_used` timestamp for a cached version.
    pub fn touch(&self, tool: &str, version: &str) -> Result<(), ToolError> {
        let dir = self.version_dir(tool, version);
        if !dir.exists() {
            return Err(ToolError::NotFound(format!("{tool}@{version}")));
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = format_rfc3339(ts);
        let last_used_path = dir.join(".last_used");
        fs::write(&last_used_path, timestamp)?;
        debug!(tool, version, "updated .last_used");
        Ok(())
    }

    /// List all cached versions for a tool.
    pub fn list_versions(&self, tool: &str) -> Result<Vec<CachedVersion>, ToolError> {
        let tool_dir = self.base_dir.join(tool);
        if !tool_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&tool_dir)? {
            let entry = entry?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let version = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if version.is_empty() {
                continue;
            }

            let size_bytes = dir_size(&path);
            let last_used = read_last_used(&path);

            versions.push(CachedVersion {
                version,
                size_bytes,
                last_used,
                path,
            });
        }
        Ok(versions)
    }

    /// Remove a specific cached version from disk.
    pub fn remove(&self, tool: &str, version: &str) -> Result<(), ToolError> {
        let dir = self.version_dir(tool, version);
        if !dir.exists() {
            return Err(ToolError::NotFound(format!("{tool}@{version}")));
        }
        fs::remove_dir_all(&dir)?;
        debug!(tool, version, "removed cached version");
        Ok(())
    }

    /// Prune all versions whose `.last_used` timestamp is older than `max_age`.
    pub fn prune(&self) -> Result<PruneResult, ToolError> {
        info!(max_age_secs = self.max_age.as_secs(), "pruning tools cache");
        let mut result = PruneResult::default();

        if !self.base_dir.exists() {
            return Ok(result);
        }

        let cutoff = SystemTime::now()
            .checked_sub(self.max_age)
            .unwrap_or(UNIX_EPOCH);

        for tool_entry in fs::read_dir(&self.base_dir)? {
            let tool_entry = tool_entry?;
            let tool_path = tool_entry.path();
            if !tool_path.is_dir() {
                continue;
            }
            let tool_name = tool_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            for ver_entry in fs::read_dir(&tool_path)? {
                let ver_entry = ver_entry?;
                let ver_path = ver_entry.path();
                if !ver_path.is_dir() {
                    continue;
                }
                let version = ver_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();

                let last_used = read_last_used(&ver_path);
                if last_used < cutoff {
                    let freed = dir_size(&ver_path);
                    if fs::remove_dir_all(&ver_path).is_ok() {
                        debug!(tool = tool_name, version, "pruned stale cached version");
                        result.removed.push((tool_name.clone(), version));
                        result.freed_bytes += freed;
                    }
                }
            }
        }

        info!(
            removed = result.removed.len(),
            freed_bytes = result.freed_bytes,
            "tools cache prune complete"
        );
        Ok(result)
    }

    /// Get the overall cache status (size and all cached versions).
    pub fn status(&self) -> Result<CacheStatus, ToolError> {
        let mut status = CacheStatus::default();

        if !self.base_dir.exists() {
            return Ok(status);
        }

        for tool_entry in fs::read_dir(&self.base_dir)? {
            let tool_entry = tool_entry?;
            let tool_path = tool_entry.path();
            if !tool_path.is_dir() {
                continue;
            }
            let tool_name = tool_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let versions = self.list_versions(&tool_name)?;
            for v in &versions {
                status.total_size += v.size_bytes;
                status.entry_count += 1;
            }
            if !versions.is_empty() {
                status.tools.insert(tool_name, versions);
            }
        }

        Ok(status)
    }

    /// The configured base directory.
    pub fn base_dir(&self) -> &std::path::Path {
        &self.base_dir
    }

    /// The configured max_size in bytes, if set.
    pub fn max_size_bytes(&self) -> Option<u64> {
        self.max_size
    }
}

// --- helpers ---

/// Recursively compute directory size in bytes.
fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += dir_size(&p);
            } else if let Ok(meta) = entry.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

/// Read the `.last_used` file and parse the timestamp.
/// Falls back to `UNIX_EPOCH` if the file is absent or malformed.
fn read_last_used(dir: &std::path::Path) -> SystemTime {
    let path = dir.join(".last_used");
    let Ok(contents) = fs::read_to_string(&path) else {
        return UNIX_EPOCH;
    };
    parse_rfc3339_approx(contents.trim()).unwrap_or(UNIX_EPOCH)
}

/// Format a Unix epoch second as a minimal RFC3339 UTC timestamp string,
/// e.g. `"2026-02-18T12:34:56Z"`.
fn format_rfc3339(secs: u64) -> String {
    // We derive date/time components without pulling in chrono.
    let mut s = secs;

    let sec = s % 60;
    s /= 60;
    let min = s % 60;
    s /= 60;
    let hour = s % 24;
    s /= 24;

    // Days since 1970-01-01 (s is now days)
    let (year, month, day) = days_to_ymd(s);

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Parse a minimal RFC3339 UTC string back to Unix epoch seconds.
fn parse_rfc3339_approx(s: &str) -> Option<SystemTime> {
    // Expected: "YYYY-MM-DDTHH:MM:SSZ"
    if s.len() < 20 {
        return None;
    }
    let year: u64 = s[0..4].parse().ok()?;
    let month: u64 = s[5..7].parse().ok()?;
    let day: u64 = s[8..10].parse().ok()?;
    let hour: u64 = s[11..13].parse().ok()?;
    let min: u64 = s[14..16].parse().ok()?;
    let sec: u64 = s[17..19].parse().ok()?;

    let days = ymd_to_days(year, month, day)?;
    let total_secs = days * 86400 + hour * 3600 + min * 60 + sec;
    Some(UNIX_EPOCH + Duration::from_secs(total_secs))
}

/// Convert days-since-epoch (1970-01-01) to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Shift epoch to 1 March 0000 for easier leap-year math (Gregorian)
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Convert (year, month, day) to days-since-epoch (1970-01-01).
fn ymd_to_days(y: u64, m: u64, d: u64) -> Option<u64> {
    // Shift to era-based system
    let y = if m <= 2 { y.checked_sub(1)? } else { y };
    let era = y / 400;
    let yoe = y % 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe;
    // Shift back from 1 Mar 0000 epoch to 1970-01-01
    days.checked_sub(719_468)
}

/// Parse a human-readable size string like "10GB", "500MB", "1TB" to bytes.
/// Case-insensitive. Returns `None` if the string is not parseable.
pub fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    // Find where the numeric part ends
    let split_pos = s.find(|c: char| c.is_alphabetic())?;
    let (num_part, unit_part) = s.split_at(split_pos);
    let value: f64 = num_part.trim().parse().ok()?;
    let multiplier: u64 = match unit_part.trim().to_ascii_uppercase().as_str() {
        "B" => 1,
        "KB" | "K" => 1_024,
        "MB" | "M" => 1_024 * 1_024,
        "GB" | "G" => 1_024 * 1_024 * 1_024,
        "TB" | "T" => 1_024u64 * 1_024 * 1_024 * 1_024,
        _ => return None,
    };
    Some((value * multiplier as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_cache(tmp: &TempDir, max_age_days: u64) -> ToolCache {
        let config = canaveral_core::config::ToolsCacheConfig {
            dir: tmp.path().join("tools"),
            max_age_days,
            max_size: None,
        };
        ToolCache::new(&config)
    }

    // --- parse_size ---

    #[test]
    fn test_parse_size_bytes() {
        assert_eq!(parse_size("512B"), Some(512));
    }

    #[test]
    fn test_parse_size_kb() {
        assert_eq!(parse_size("1KB"), Some(1_024));
        assert_eq!(parse_size("1K"), Some(1_024));
        assert_eq!(parse_size("1kb"), Some(1_024));
    }

    #[test]
    fn test_parse_size_mb() {
        assert_eq!(parse_size("500MB"), Some(500 * 1_024 * 1_024));
        assert_eq!(parse_size("500mb"), Some(500 * 1_024 * 1_024));
    }

    #[test]
    fn test_parse_size_gb() {
        assert_eq!(parse_size("10GB"), Some(10 * 1_024 * 1_024 * 1_024));
    }

    #[test]
    fn test_parse_size_tb() {
        assert_eq!(parse_size("1TB"), Some(1_024u64 * 1_024 * 1_024 * 1_024));
    }

    #[test]
    fn test_parse_size_invalid() {
        assert_eq!(parse_size("notasize"), None);
        assert_eq!(parse_size(""), None);
    }

    // --- cache dir creation ---

    #[test]
    fn test_version_dir_path() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        let dir = cache.version_dir("bun", "1.2.0");
        assert_eq!(dir, tmp.path().join("tools/bun/1.2.0"));
    }

    // --- is_cached ---

    #[test]
    fn test_is_cached_false_when_absent() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        assert!(!cache.is_cached("bun", "1.2.0"));
    }

    #[test]
    fn test_is_cached_true_after_creation() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        let dir = cache.version_dir("bun", "1.2.0");
        fs::create_dir_all(&dir).unwrap();
        assert!(cache.is_cached("bun", "1.2.0"));
    }

    // --- touch ---

    #[test]
    fn test_touch_creates_last_used_file() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        let dir = cache.version_dir("node", "22.0.0");
        fs::create_dir_all(&dir).unwrap();

        cache.touch("node", "22.0.0").unwrap();

        let last_used_path = dir.join(".last_used");
        assert!(last_used_path.exists());
        let content = fs::read_to_string(&last_used_path).unwrap();
        assert!(content.contains('T'));
        assert!(content.ends_with('Z'));
    }

    #[test]
    fn test_touch_missing_tool_returns_error() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        let result = cache.touch("bun", "9.9.9");
        assert!(result.is_err());
    }

    // --- prune ---

    #[test]
    fn test_prune_removes_stale_entries() {
        let tmp = TempDir::new().unwrap();
        // max_age = 1 day; we'll write a timestamp from 2 days ago
        let cache = make_cache(&tmp, 1);

        let ver_dir = cache.version_dir("go", "1.21.0");
        fs::create_dir_all(&ver_dir).unwrap();

        // Write a timestamp 2 days in the past
        let two_days_ago = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(2 * 24 * 3600);
        let ts = format_rfc3339(two_days_ago);
        fs::write(ver_dir.join(".last_used"), &ts).unwrap();

        let result = cache.prune().unwrap();
        assert_eq!(result.removed.len(), 1);
        assert_eq!(result.removed[0], ("go".to_string(), "1.21.0".to_string()));
        assert!(!ver_dir.exists());
    }

    #[test]
    fn test_prune_keeps_recent_entries() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);

        let ver_dir = cache.version_dir("rust", "1.75.0");
        fs::create_dir_all(&ver_dir).unwrap();
        cache.touch("rust", "1.75.0").unwrap();

        let result = cache.prune().unwrap();
        assert_eq!(result.removed.len(), 0);
        assert!(ver_dir.exists());
    }

    #[test]
    fn test_prune_empty_cache_dir() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        // base_dir doesn't exist yet
        let result = cache.prune().unwrap();
        assert_eq!(result.removed.len(), 0);
    }

    // --- status ---

    #[test]
    fn test_status_empty() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);
        let status = cache.status().unwrap();
        assert_eq!(status.entry_count, 0);
        assert_eq!(status.total_size, 0);
        assert!(status.tools.is_empty());
    }

    #[test]
    fn test_status_counts_entries() {
        let tmp = TempDir::new().unwrap();
        let cache = make_cache(&tmp, 30);

        let v1 = cache.version_dir("python", "3.12.0");
        let v2 = cache.version_dir("python", "3.11.0");
        fs::create_dir_all(&v1).unwrap();
        fs::create_dir_all(&v2).unwrap();
        // Write a small file so size > 0
        fs::write(v1.join("bin"), b"fake").unwrap();

        let status = cache.status().unwrap();
        assert_eq!(status.entry_count, 2);
        assert!(status.total_size >= 4);
        assert!(status.tools.contains_key("python"));
        assert_eq!(status.tools["python"].len(), 2);
    }

    // --- timestamp round-trip ---

    #[test]
    fn test_rfc3339_round_trip() {
        let secs: u64 = 1_739_880_000; // a known timestamp
        let formatted = format_rfc3339(secs);
        let parsed = parse_rfc3339_approx(&formatted).unwrap();
        let parsed_secs = parsed.duration_since(UNIX_EPOCH).unwrap().as_secs();
        assert_eq!(parsed_secs, secs);
    }
}
