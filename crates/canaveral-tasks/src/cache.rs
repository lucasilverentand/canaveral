//! Content-addressable task cache

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, info};

use crate::task::{TaskDefinition, TaskId};

/// Cache key — SHA-256 hash of all inputs
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CacheKey(pub String);

impl CacheKey {
    /// Compute a cache key from task inputs
    pub fn compute(
        id: &TaskId,
        definition: &TaskDefinition,
        root_dir: &Path,
    ) -> Self {
        let mut hasher = Sha256::new();

        // Hash task identity
        hasher.update(id.package.as_bytes());
        hasher.update(b":");
        hasher.update(id.task_name.as_bytes());

        // Hash command
        if let Some(cmd) = &definition.command {
            hasher.update(cmd.as_bytes());
        }

        // Hash environment variables (sorted for determinism)
        let sorted_env: BTreeMap<_, _> = definition.env.iter().collect();
        for (k, v) in sorted_env {
            hasher.update(k.as_bytes());
            hasher.update(b"=");
            hasher.update(v.as_bytes());
        }

        // Hash input file contents
        let input_globs = if definition.inputs.is_empty() {
            // Default: hash all source files in the package
            vec!["**/*".to_string()]
        } else {
            definition.inputs.clone()
        };

        // Collect and hash file contents
        let pkg_dir = root_dir.join(&id.package);
        if pkg_dir.exists() {
            let mut file_hashes: BTreeMap<String, String> = BTreeMap::new();

            for pattern in &input_globs {
                let full_pattern = pkg_dir.join(pattern).to_string_lossy().to_string();
                if let Ok(paths) = glob::glob(&full_pattern) {
                    for entry in paths.flatten() {
                        if entry.is_file() {
                            if let Ok(contents) = fs::read(&entry) {
                                let mut file_hasher = Sha256::new();
                                file_hasher.update(&contents);
                                let hash = format!("{:x}", file_hasher.finalize());
                                let relative = entry
                                    .strip_prefix(root_dir)
                                    .unwrap_or(&entry)
                                    .to_string_lossy()
                                    .to_string();
                                file_hashes.insert(relative, hash);
                            }
                        }
                    }
                }
            }

            for (path, hash) in &file_hashes {
                hasher.update(path.as_bytes());
                hasher.update(hash.as_bytes());
            }
        }

        let result = format!("{:x}", hasher.finalize());
        CacheKey(result)
    }
}

/// A cached task result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    /// Cache key
    pub key: CacheKey,
    /// Task ID
    pub task_id: TaskId,
    /// Output file paths (relative to root)
    pub output_files: Vec<String>,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
    /// Original task duration
    pub duration_ms: u64,
    /// When this entry was created
    pub created_at: String,
}

/// Content-addressable task cache
#[derive(Debug, Clone)]
pub struct TaskCache {
    /// Cache directory
    cache_dir: PathBuf,
}

impl TaskCache {
    /// Create a new task cache
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { cache_dir }
    }

    /// Create cache with default directory
    pub fn default_dir(root: &Path) -> Self {
        Self::new(root.join(".canaveral").join("cache"))
    }

    /// Look up a cached result
    pub fn lookup(
        &self,
        id: &TaskId,
        definition: &TaskDefinition,
        root_dir: &Path,
    ) -> Result<Option<CacheEntry>, CacheError> {
        let key = CacheKey::compute(id, definition, root_dir);
        let entry_dir = self.cache_dir.join(&key.0);
        let metadata_path = entry_dir.join("metadata.json");

        if !metadata_path.exists() {
            debug!(task = %id, "cache miss");
            return Ok(None);
        }

        let mut file = fs::File::open(&metadata_path).map_err(CacheError::Io)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).map_err(CacheError::Io)?;

        let entry: CacheEntry =
            serde_json::from_str(&contents).map_err(CacheError::Json)?;

        // Verify output archive exists in cache if outputs were stored
        let outputs_path = entry_dir.join("outputs.tar.gz");
        if !entry.output_files.is_empty() && !outputs_path.exists() {
            return Ok(None);
        }

        debug!(task = %id, "cache hit");
        Ok(Some(entry))
    }

    /// Store a task result in the cache
    pub fn store(
        &self,
        id: &TaskId,
        definition: &TaskDefinition,
        root_dir: &Path,
        stdout: &str,
        stderr: &str,
    ) -> Result<CacheKey, CacheError> {
        debug!(task = %id, "storing result in cache");
        let key = CacheKey::compute(id, definition, root_dir);
        let entry_dir = self.cache_dir.join(&key.0);
        fs::create_dir_all(&entry_dir).map_err(CacheError::Io)?;

        // Collect output file list
        let mut output_files = Vec::new();
        let pkg_dir = root_dir.join(&id.package);
        for pattern in &definition.outputs {
            let full_pattern = pkg_dir.join(pattern).to_string_lossy().to_string();
            if let Ok(paths) = glob::glob(&full_pattern) {
                for entry in paths.flatten() {
                    if entry.is_file() {
                        let relative = entry
                            .strip_prefix(root_dir)
                            .unwrap_or(&entry)
                            .to_string_lossy()
                            .to_string();
                        output_files.push(relative);
                    }
                }
            }
        }

        // Write metadata
        let entry = CacheEntry {
            key: key.clone(),
            task_id: id.clone(),
            output_files,
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            duration_ms: 0,
            created_at: chrono::Utc::now().to_rfc3339(),
        };

        let metadata_path = entry_dir.join("metadata.json");
        let json = serde_json::to_string_pretty(&entry).map_err(CacheError::Json)?;
        let mut file = fs::File::create(&metadata_path).map_err(CacheError::Io)?;
        file.write_all(json.as_bytes()).map_err(CacheError::Io)?;

        Ok(key)
    }

    /// Remove old cache entries
    pub fn prune(&self, max_age: Duration) -> Result<PruneStats, CacheError> {
        info!(max_age_secs = max_age.as_secs(), "pruning cache");
        let mut stats = PruneStats::default();

        if !self.cache_dir.exists() {
            return Ok(stats);
        }

        let cutoff = chrono::Utc::now() - chrono::Duration::from_std(max_age).unwrap_or_default();

        for entry in fs::read_dir(&self.cache_dir).map_err(CacheError::Io)? {
            let entry = entry.map_err(CacheError::Io)?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            stats.total += 1;

            let metadata_path = path.join("metadata.json");
            if let Ok(contents) = fs::read_to_string(&metadata_path) {
                if let Ok(cache_entry) = serde_json::from_str::<CacheEntry>(&contents) {
                    if let Ok(created) = chrono::DateTime::parse_from_rfc3339(&cache_entry.created_at) {
                        if created < cutoff {
                            if fs::remove_dir_all(&path).is_ok() {
                                stats.removed += 1;
                            }
                            continue;
                        }
                    }
                }
            }

            stats.kept += 1;
        }

        info!(total = stats.total, removed = stats.removed, kept = stats.kept, "cache prune complete");
        Ok(stats)
    }

    /// Get cache statistics
    pub fn status(&self) -> Result<CacheStats, CacheError> {
        let mut stats = CacheStats::default();

        if !self.cache_dir.exists() {
            return Ok(stats);
        }

        for entry in fs::read_dir(&self.cache_dir).map_err(CacheError::Io)? {
            let entry = entry.map_err(CacheError::Io)?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            stats.entries += 1;

            // Calculate size
            if let Ok(dir_entries) = fs::read_dir(&path) {
                for file in dir_entries.flatten() {
                    if let Ok(meta) = file.metadata() {
                        stats.total_size += meta.len();
                    }
                }
            }
        }

        Ok(stats)
    }

    /// Get the cache directory
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

/// Statistics from a prune operation
#[derive(Debug, Default)]
pub struct PruneStats {
    /// Total entries found
    pub total: usize,
    /// Entries removed
    pub removed: usize,
    /// Entries kept
    pub kept: usize,
}

/// Cache statistics
#[derive(Debug, Default)]
pub struct CacheStats {
    /// Number of cache entries
    pub entries: usize,
    /// Total size in bytes
    pub total_size: u64,
}

impl CacheStats {
    /// Format total size in human-readable form
    pub fn formatted_size(&self) -> String {
        if self.total_size < 1024 {
            format!("{} B", self.total_size)
        } else if self.total_size < 1024 * 1024 {
            format!("{:.1} KB", self.total_size as f64 / 1024.0)
        } else if self.total_size < 1024 * 1024 * 1024 {
            format!("{:.1} MB", self.total_size as f64 / (1024.0 * 1024.0))
        } else {
            format!(
                "{:.1} GB",
                self.total_size as f64 / (1024.0 * 1024.0 * 1024.0)
            )
        }
    }
}

/// Cache errors
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    /// IO error
    #[error("Cache IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization error
    #[error("Cache serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use crate::task::TaskDefinition;

    #[test]
    fn test_cache_key_deterministic() {
        let id = TaskId::new("pkg", "build");
        let def = TaskDefinition::new("build").with_command("echo hello");
        let dir = PathBuf::from("/tmp/nonexistent");

        let key1 = CacheKey::compute(&id, &def, &dir);
        let key2 = CacheKey::compute(&id, &def, &dir);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_differs_on_command() {
        let id = TaskId::new("pkg", "build");
        let def1 = TaskDefinition::new("build").with_command("echo hello");
        let def2 = TaskDefinition::new("build").with_command("echo world");
        let dir = PathBuf::from("/tmp/nonexistent");

        let key1 = CacheKey::compute(&id, &def1, &dir);
        let key2 = CacheKey::compute(&id, &def2, &dir);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_store_and_lookup() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("cache");
        let cache = TaskCache::new(cache_dir);

        let id = TaskId::new("pkg", "build");
        let def = TaskDefinition::new("build")
            .with_command("echo hello")
            .with_outputs(vec!["dist/**".to_string()]);

        let key = cache
            .store(&id, &def, temp.path(), "hello\n", "")
            .unwrap();

        let entry = cache.lookup(&id, &def, temp.path()).unwrap();
        assert!(entry.is_some());
        let entry = entry.unwrap();
        assert_eq!(entry.key, key);
        assert_eq!(entry.stdout, "hello\n");
    }

    #[test]
    fn test_cache_miss() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("cache");
        let cache = TaskCache::new(cache_dir);

        let id = TaskId::new("pkg", "build");
        let def = TaskDefinition::new("build").with_command("echo hello");

        let entry = cache.lookup(&id, &def, temp.path()).unwrap();
        assert!(entry.is_none());
    }

    #[test]
    fn test_cache_status_empty() {
        let temp = TempDir::new().unwrap();
        let cache_dir = temp.path().join("cache");
        let cache = TaskCache::new(cache_dir);

        let stats = cache.status().unwrap();
        assert_eq!(stats.entries, 0);
        assert_eq!(stats.total_size, 0);
    }

    #[test]
    fn test_cache_stats_formatted_size() {
        let stats = CacheStats {
            entries: 0,
            total_size: 1536,
        };
        assert_eq!(stats.formatted_size(), "1.5 KB");

        let stats = CacheStats {
            entries: 0,
            total_size: 500,
        };
        assert_eq!(stats.formatted_size(), "500 B");
    }
}
