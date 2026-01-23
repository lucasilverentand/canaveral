//! Remote operations

use crate::repository::{GitRepo, Result};
use canaveral_core::error::GitError;

impl GitRepo {
    /// Get list of remote names
    pub fn remotes(&self) -> Result<Vec<String>> {
        let remotes = self.repo.remotes()?;
        Ok(remotes.iter().filter_map(|r| r.map(|s| s.to_string())).collect())
    }

    /// Check if a remote exists
    pub fn has_remote(&self, name: &str) -> Result<bool> {
        Ok(self.remotes()?.contains(&name.to_string()))
    }

    /// Get the URL for a remote
    pub fn remote_url(&self, name: &str) -> Result<Option<String>> {
        match self.repo.find_remote(name) {
            Ok(remote) => Ok(remote.url().map(|s| s.to_string())),
            Err(e) if e.code() == git2::ErrorCode::NotFound => {
                Err(GitError::RemoteNotFound(name.to_string()))
            }
            Err(e) => Err(GitError::Git2(e)),
        }
    }

    /// Push a tag to a remote
    ///
    /// Note: This requires proper authentication to be configured.
    /// For CLI usage, it's often better to shell out to git directly.
    pub fn push_tag(&self, remote_name: &str, tag_name: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name).map_err(|_| {
            GitError::RemoteNotFound(remote_name.to_string())
        })?;

        let refspec = format!("refs/tags/{}:refs/tags/{}", tag_name, tag_name);

        // Note: This will fail without proper credentials setup
        // In practice, the CLI will shell out to git for push operations
        remote.push(&[&refspec], None).map_err(|e| {
            GitError::PushFailed(format!("Failed to push tag {}: {}", tag_name, e))
        })?;

        Ok(())
    }

    /// Push commits to a remote
    ///
    /// Note: This requires proper authentication to be configured.
    pub fn push_commits(&self, remote_name: &str, branch: &str) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name).map_err(|_| {
            GitError::RemoteNotFound(remote_name.to_string())
        })?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

        remote.push(&[&refspec], None).map_err(|e| {
            GitError::PushFailed(format!("Failed to push to {}/{}: {}", remote_name, branch, e))
        })?;

        Ok(())
    }

    /// Fetch from a remote
    pub fn fetch(&self, remote_name: &str, refspecs: &[&str]) -> Result<()> {
        let mut remote = self.repo.find_remote(remote_name).map_err(|_| {
            GitError::RemoteNotFound(remote_name.to_string())
        })?;

        remote.fetch(refspecs, None, None).map_err(|e| {
            GitError::Git2(e)
        })?;

        Ok(())
    }
}

/// Push using git CLI (more reliable for authentication)
pub fn git_push_tag(remote: &str, tag: &str) -> std::io::Result<std::process::Output> {
    std::process::Command::new("git")
        .args(["push", remote, tag])
        .output()
}

/// Push commits using git CLI
pub fn git_push(remote: &str, branch: &str) -> std::io::Result<std::process::Output> {
    std::process::Command::new("git")
        .args(["push", remote, branch])
        .output()
}

/// Push both commits and tags using git CLI
pub fn git_push_with_tags(remote: &str, branch: &str) -> std::io::Result<std::process::Output> {
    std::process::Command::new("git")
        .args(["push", "--follow-tags", remote, branch])
        .output()
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use std::path::Path;
    use tempfile::TempDir;

    fn setup_repo() -> (TempDir, GitRepo) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("Test", "test@example.com").unwrap();

        std::fs::write(temp.path().join("file.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        let git_repo = GitRepo::open(temp.path()).unwrap();
        (temp, git_repo)
    }

    #[test]
    fn test_remotes_empty() {
        let (_temp, repo) = setup_repo();
        let remotes = repo.remotes().unwrap();
        assert!(remotes.is_empty());
    }

    #[test]
    fn test_has_remote() {
        let (_temp, repo) = setup_repo();
        assert!(!repo.has_remote("origin").unwrap());
    }

    #[test]
    fn test_remote_not_found() {
        let (_temp, repo) = setup_repo();
        let result = repo.remote_url("nonexistent");
        assert!(matches!(result, Err(GitError::RemoteNotFound(_))));
    }
}
