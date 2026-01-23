//! Commit history operations

use chrono::{TimeZone, Utc};
use git2::{Oid, Sort};

use crate::repository::{GitRepo, Result};
use crate::types::CommitInfo;

impl GitRepo {
    /// Get commits since a specific commit hash
    pub fn commits_since(&self, since: &str) -> Result<Vec<CommitInfo>> {
        let since_oid = self.repo.revparse_single(since)?.id();
        self.commits_since_oid(since_oid)
    }

    /// Get commits since a specific OID
    pub fn commits_since_oid(&self, since: Oid) -> Result<Vec<CommitInfo>> {
        let head = self.head_commit()?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        revwalk.push(head.id())?;
        revwalk.hide(since)?;

        let mut commits = Vec::new();

        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            commits.push(commit_to_info(&commit));
        }

        Ok(commits)
    }

    /// Get commits since a tag
    pub fn commits_since_tag(&self, tag_name: &str) -> Result<Vec<CommitInfo>> {
        // Try to find the tag
        let tag_ref = format!("refs/tags/{}", tag_name);
        let reference = self.repo.find_reference(&tag_ref)?;
        let target = reference.peel_to_commit()?;

        self.commits_since_oid(target.id())
    }

    /// Get all commits on the current branch
    pub fn all_commits(&self) -> Result<Vec<CommitInfo>> {
        let head = self.head_commit()?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        revwalk.push(head.id())?;

        let mut commits = Vec::new();

        for oid in revwalk {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            commits.push(commit_to_info(&commit));
        }

        Ok(commits)
    }

    /// Get the most recent N commits
    pub fn recent_commits(&self, count: usize) -> Result<Vec<CommitInfo>> {
        let head = self.head_commit()?;

        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        revwalk.push(head.id())?;

        let mut commits = Vec::new();

        for oid in revwalk.take(count) {
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            commits.push(commit_to_info(&commit));
        }

        Ok(commits)
    }

    /// Get a specific commit by hash
    pub fn get_commit(&self, hash: &str) -> Result<CommitInfo> {
        let oid = Oid::from_str(hash)?;
        let commit = self.repo.find_commit(oid)?;
        Ok(commit_to_info(&commit))
    }
}

/// Convert a git2 Commit to CommitInfo
fn commit_to_info(commit: &git2::Commit<'_>) -> CommitInfo {
    let hash = commit.id().to_string();
    let author = commit.author();

    let message = commit
        .summary()
        .unwrap_or("(no message)")
        .to_string();

    let body = commit.body().map(|b| b.to_string());

    let timestamp = Utc
        .timestamp_opt(commit.time().seconds(), 0)
        .single()
        .unwrap_or_else(Utc::now);

    CommitInfo::new(
        hash,
        message,
        author.name().unwrap_or("Unknown"),
        author.email().unwrap_or("unknown@example.com"),
        timestamp,
    )
    .with_body(body.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use git2::{Repository, Signature};
    use tempfile::TempDir;

    fn setup_repo_with_commits() -> (TempDir, GitRepo) {
        let temp = TempDir::new().unwrap();
        let repo = Repository::init(temp.path()).unwrap();

        // Create initial commit
        let sig = Signature::now("Test", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        // Create a file and second commit
        std::fs::write(temp.path().join("file.txt"), "content").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        index.write().unwrap();

        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();

        repo.commit(
            Some("HEAD"),
            &sig,
            &sig,
            "feat: add file",
            &tree,
            &[&parent],
        )
        .unwrap();

        let git_repo = GitRepo::open(temp.path()).unwrap();
        (temp, git_repo)
    }

    use std::path::Path;

    #[test]
    fn test_recent_commits() {
        let (_temp, repo) = setup_repo_with_commits();
        let commits = repo.recent_commits(10).unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].message, "feat: add file");
    }

    #[test]
    fn test_all_commits() {
        let (_temp, repo) = setup_repo_with_commits();
        let commits = repo.all_commits().unwrap();
        assert!(!commits.is_empty());
    }
}
