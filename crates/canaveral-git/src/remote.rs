//! Remote operations

use tracing::{info, instrument};

use crate::repository::{GitRepo, Result};
use canaveral_core::error::GitError;

impl GitRepo {
    /// Get list of remote names
    pub fn remotes(&self) -> Result<Vec<String>> {
        let remotes = self.repo.remotes()?;
        Ok(remotes
            .iter()
            .filter_map(|r| r.map(|s| s.to_string()))
            .collect())
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
    #[instrument(skip(self), fields(remote_name, tag_name))]
    pub fn push_tag(&self, remote_name: &str, tag_name: &str) -> Result<()> {
        let start = std::time::Instant::now();
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .map_err(|_| GitError::RemoteNotFound(remote_name.to_string()))?;

        let refspec = format!("refs/tags/{}:refs/tags/{}", tag_name, tag_name);

        // Note: This will fail without proper credentials setup
        // In practice, the CLI will shell out to git for push operations
        remote
            .push(&[&refspec], None)
            .map_err(|e| GitError::PushFailed(format!("Failed to push tag {}: {}", tag_name, e)))?;

        info!(
            remote = remote_name,
            tag = tag_name,
            duration_ms = start.elapsed().as_millis(),
            "pushed tag"
        );
        Ok(())
    }

    /// Push commits to a remote
    ///
    /// Note: This requires proper authentication to be configured.
    #[instrument(skip(self), fields(remote_name, branch))]
    pub fn push_commits(&self, remote_name: &str, branch: &str) -> Result<()> {
        let start = std::time::Instant::now();
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .map_err(|_| GitError::RemoteNotFound(remote_name.to_string()))?;

        let refspec = format!("refs/heads/{}:refs/heads/{}", branch, branch);

        remote.push(&[&refspec], None).map_err(|e| {
            GitError::PushFailed(format!(
                "Failed to push to {}/{}: {}",
                remote_name, branch, e
            ))
        })?;

        info!(
            remote = remote_name,
            branch,
            duration_ms = start.elapsed().as_millis(),
            "pushed commits"
        );
        Ok(())
    }

    /// Fetch from a remote
    #[instrument(skip(self), fields(remote_name))]
    pub fn fetch(&self, remote_name: &str, refspecs: &[&str]) -> Result<()> {
        let start = std::time::Instant::now();
        let mut remote = self
            .repo
            .find_remote(remote_name)
            .map_err(|_| GitError::RemoteNotFound(remote_name.to_string()))?;

        remote.fetch(refspecs, None, None).map_err(GitError::Git2)?;

        info!(
            remote = remote_name,
            duration_ms = start.elapsed().as_millis(),
            "fetched from remote"
        );
        Ok(())
    }
}

/// Push using git CLI (more reliable for authentication)
#[instrument(fields(remote, tag))]
pub fn git_push_tag(remote: &str, tag: &str) -> std::io::Result<std::process::Output> {
    let start = std::time::Instant::now();
    let output = std::process::Command::new("git")
        .args(["push", remote, tag])
        .output()?;
    info!(
        remote,
        tag,
        duration_ms = start.elapsed().as_millis(),
        success = output.status.success(),
        "git push tag (CLI)"
    );
    Ok(output)
}

/// Push commits using git CLI
#[instrument(fields(remote, branch))]
pub fn git_push(remote: &str, branch: &str) -> std::io::Result<std::process::Output> {
    let start = std::time::Instant::now();
    let output = std::process::Command::new("git")
        .args(["push", remote, branch])
        .output()?;
    info!(
        remote,
        branch,
        duration_ms = start.elapsed().as_millis(),
        success = output.status.success(),
        "git push (CLI)"
    );
    Ok(output)
}

/// Push both commits and tags using git CLI
#[instrument(fields(remote, branch))]
pub fn git_push_with_tags(remote: &str, branch: &str) -> std::io::Result<std::process::Output> {
    let start = std::time::Instant::now();
    let output = std::process::Command::new("git")
        .args(["push", "--follow-tags", remote, branch])
        .output()?;
    info!(
        remote,
        branch,
        duration_ms = start.elapsed().as_millis(),
        success = output.status.success(),
        "git push with tags (CLI)"
    );
    Ok(output)
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
