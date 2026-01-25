//! Storage backends for match sync
//!
//! Supports Git, S3, GCS, and Azure Blob storage backends.

use std::path::PathBuf;
use std::process::Command;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::{Result, SigningError};

/// Storage backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SyncStorage {
    /// Git repository storage
    Git {
        /// Repository URL
        url: String,
        /// Branch name
        branch: String,
    },
    /// S3 bucket storage
    S3 {
        /// Bucket name
        bucket: String,
        /// Prefix (folder) within bucket
        prefix: String,
        /// AWS region
        region: String,
    },
    /// Google Cloud Storage
    GoogleCloudStorage {
        /// Bucket name
        bucket: String,
        /// Prefix (folder) within bucket
        prefix: String,
    },
    /// Azure Blob Storage
    AzureBlob {
        /// Container name
        container: String,
        /// Prefix (folder) within container
        prefix: String,
    },
}

/// Storage backend trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Sync storage (clone/pull for git, no-op for cloud)
    async fn sync(&self) -> Result<()>;

    /// Read a file from storage
    async fn read(&self, path: &str) -> Result<Vec<u8>>;

    /// Write a file to storage
    async fn write(&self, path: &str, data: &[u8]) -> Result<()>;

    /// Delete a file from storage
    async fn delete(&self, path: &str) -> Result<()>;

    /// List files in storage
    async fn list(&self, prefix: &str) -> Result<Vec<String>>;

    /// Check if a file exists
    async fn exists(&self, path: &str) -> Result<bool>;
}

/// Git storage backend
pub struct GitStorage {
    /// Repository URL
    url: String,
    /// Branch name
    branch: String,
    /// Local clone path
    local_path: PathBuf,
}

impl GitStorage {
    /// Create a new git storage backend
    pub fn new(url: String, branch: String) -> Self {
        // Compute local path from URL
        let repo_name = url
            .rsplit('/')
            .next()
            .unwrap_or("match")
            .trim_end_matches(".git");

        let local_path = dirs::cache_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("canaveral")
            .join("match-repos")
            .join(repo_name);

        Self {
            url,
            branch,
            local_path,
        }
    }

    /// Clone or pull the repository
    fn clone_or_pull(&self) -> Result<()> {
        if self.local_path.exists() {
            // Pull latest changes
            let output = Command::new("git")
                .args(["pull", "origin", &self.branch])
                .current_dir(&self.local_path)
                .output()
                .map_err(|e| SigningError::Io(e))?;

            if !output.status.success() {
                return Err(SigningError::Command {
                    command: format!("git pull origin {}", self.branch),
                    status: output.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
        } else {
            // Clone repository
            std::fs::create_dir_all(self.local_path.parent().unwrap())
                .map_err(|e| SigningError::Io(e))?;

            let output = Command::new("git")
                .args([
                    "clone",
                    "--branch",
                    &self.branch,
                    "--single-branch",
                    &self.url,
                    self.local_path.to_str().unwrap_or_default(),
                ])
                .output()
                .map_err(|e| SigningError::Io(e))?;

            if !output.status.success() {
                return Err(SigningError::Command {
                    command: format!("git clone {}", self.url),
                    status: output.status.code().unwrap_or(-1),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                });
            }
        }

        Ok(())
    }

    /// Commit and push changes
    fn commit_and_push(&self, message: &str) -> Result<()> {
        // Add all changes
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.local_path)
            .output()
            .map_err(|e| SigningError::Io(e))?;

        // Commit
        let commit_output = Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.local_path)
            .output()
            .map_err(|e| SigningError::Io(e))?;

        // If nothing to commit, that's fine
        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr);
            if !stderr.contains("nothing to commit") {
                return Err(SigningError::Command {
                    command: "git commit".to_string(),
                    status: commit_output.status.code().unwrap_or(-1),
                    stderr: stderr.to_string(),
                });
            }
            return Ok(());
        }

        // Push
        let push_output = Command::new("git")
            .args(["push", "origin", &self.branch])
            .current_dir(&self.local_path)
            .output()
            .map_err(|e| SigningError::Io(e))?;

        if !push_output.status.success() {
            return Err(SigningError::Command {
                command: format!("git push origin {}", self.branch),
                status: push_output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&push_output.stderr).to_string(),
            });
        }

        Ok(())
    }
}

#[async_trait]
impl StorageBackend for GitStorage {
    async fn sync(&self) -> Result<()> {
        self.clone_or_pull()
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let file_path = self.local_path.join(path);
        std::fs::read(&file_path).map_err(|e| SigningError::Io(e))
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let file_path = self.local_path.join(path);

        // Create parent directories
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SigningError::Io(e))?;
        }

        std::fs::write(&file_path, data).map_err(|e| SigningError::Io(e))?;

        // Commit and push
        self.commit_and_push(&format!("Update {}", path))
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let file_path = self.local_path.join(path);

        if file_path.exists() {
            std::fs::remove_file(&file_path).map_err(|e| SigningError::Io(e))?;
            self.commit_and_push(&format!("Delete {}", path))?;
        }

        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let search_path = self.local_path.join(prefix);
        let mut files = Vec::new();

        if search_path.exists() {
            for entry in walkdir::WalkDir::new(&search_path)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.file_type().is_file())
            {
                if let Ok(relative) = entry.path().strip_prefix(&self.local_path) {
                    files.push(relative.to_string_lossy().to_string());
                }
            }
        }

        Ok(files)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let file_path = self.local_path.join(path);
        Ok(file_path.exists())
    }
}

/// S3 storage backend
pub struct S3Storage {
    /// Bucket name
    bucket: String,
    /// Prefix within bucket
    prefix: String,
    /// AWS region
    region: String,
}

impl S3Storage {
    /// Create a new S3 storage backend
    pub fn new(bucket: String, prefix: String, region: String) -> Self {
        Self {
            bucket,
            prefix,
            region,
        }
    }

    /// Build the full S3 key for a path
    fn full_key(&self, path: &str) -> String {
        if self.prefix.is_empty() {
            path.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), path)
        }
    }

    /// Run AWS CLI command
    fn aws_cli(&self, args: &[&str]) -> Result<std::process::Output> {
        let output = Command::new("aws")
            .args(args)
            .env("AWS_DEFAULT_REGION", &self.region)
            .output()
            .map_err(|e| SigningError::Io(e))?;

        Ok(output)
    }
}

#[async_trait]
impl StorageBackend for S3Storage {
    async fn sync(&self) -> Result<()> {
        // S3 doesn't need explicit sync
        Ok(())
    }

    async fn read(&self, path: &str) -> Result<Vec<u8>> {
        let key = self.full_key(path);
        let output = self.aws_cli(&[
            "s3",
            "cp",
            &format!("s3://{}/{}", self.bucket, key),
            "-",
        ])?;

        if !output.status.success() {
            return Err(SigningError::Command {
                command: format!("aws s3 cp s3://{}/{}", self.bucket, key),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(output.stdout)
    }

    async fn write(&self, path: &str, data: &[u8]) -> Result<()> {
        let key = self.full_key(path);

        // Write to temp file first
        let temp_file = tempfile::NamedTempFile::new()
            .map_err(|e| SigningError::Io(e))?;
        std::fs::write(temp_file.path(), data)
            .map_err(|e| SigningError::Io(e))?;

        let output = self.aws_cli(&[
            "s3",
            "cp",
            temp_file.path().to_str().unwrap_or_default(),
            &format!("s3://{}/{}", self.bucket, key),
        ])?;

        if !output.status.success() {
            return Err(SigningError::Command {
                command: format!("aws s3 cp to s3://{}/{}", self.bucket, key),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let key = self.full_key(path);
        let output = self.aws_cli(&[
            "s3",
            "rm",
            &format!("s3://{}/{}", self.bucket, key),
        ])?;

        if !output.status.success() {
            return Err(SigningError::Command {
                command: format!("aws s3 rm s3://{}/{}", self.bucket, key),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>> {
        let key = self.full_key(prefix);
        let output = self.aws_cli(&[
            "s3",
            "ls",
            &format!("s3://{}/{}", self.bucket, key),
            "--recursive",
        ])?;

        if !output.status.success() {
            return Err(SigningError::Command {
                command: format!("aws s3 ls s3://{}/{}", self.bucket, key),
                status: output.status.code().unwrap_or(-1),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        // Parse output - format: "2024-01-01 12:00:00 1234 path/to/file"
        let stdout = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = stdout
            .lines()
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                parts.get(3).map(|s| s.to_string())
            })
            .collect();

        Ok(files)
    }

    async fn exists(&self, path: &str) -> Result<bool> {
        let key = self.full_key(path);
        let output = self.aws_cli(&[
            "s3",
            "ls",
            &format!("s3://{}/{}", self.bucket, key),
        ])?;

        Ok(output.status.success() && !output.stdout.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_storage_path() {
        let storage = GitStorage::new(
            "git@github.com:org/certs.git".to_string(),
            "main".to_string(),
        );
        assert!(storage.local_path.ends_with("certs"));
    }

    #[test]
    fn test_s3_full_key() {
        let storage = S3Storage::new(
            "my-bucket".to_string(),
            "match/team".to_string(),
            "us-east-1".to_string(),
        );
        assert_eq!(storage.full_key("manifest.enc"), "match/team/manifest.enc");
    }
}
