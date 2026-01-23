//! Repository status operations

use crate::repository::{GitRepo, Result};

impl GitRepo {
    /// Check if the working directory is clean (no uncommitted changes)
    pub fn is_clean(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;

        for entry in statuses.iter() {
            let status = entry.status();

            // Check for any changes that would make the repo "dirty"
            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_renamed()
                || status.is_index_typechange()
                || status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_renamed()
                || status.is_wt_typechange()
            {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Get the current branch name
    pub fn current_branch(&self) -> Result<Option<String>> {
        let head = match self.repo.head() {
            Ok(head) => head,
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => return Ok(None),
            Err(e) => return Err(e.into()),
        };

        if head.is_branch() {
            Ok(head.shorthand().map(|s| s.to_string()))
        } else {
            // Detached HEAD
            Ok(None)
        }
    }

    /// Check if we're on a specific branch
    pub fn is_on_branch(&self, branch_name: &str) -> Result<bool> {
        match self.current_branch()? {
            Some(current) => Ok(current == branch_name),
            None => Ok(false),
        }
    }

    /// Check if HEAD is detached
    pub fn is_head_detached(&self) -> Result<bool> {
        Ok(self.repo.head_detached()?)
    }

    /// Get list of modified files
    pub fn modified_files(&self) -> Result<Vec<String>> {
        let statuses = self.repo.statuses(None)?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                let status = entry.status();
                if status.is_wt_modified()
                    || status.is_index_modified()
                    || status.is_wt_new()
                    || status.is_index_new()
                    || status.is_wt_deleted()
                    || status.is_index_deleted()
                {
                    files.push(path.to_string());
                }
            }
        }

        Ok(files)
    }

    /// Get list of untracked files
    pub fn untracked_files(&self) -> Result<Vec<String>> {
        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;
        let mut files = Vec::new();

        for entry in statuses.iter() {
            if let Some(path) = entry.path() {
                if entry.status().is_wt_new() {
                    files.push(path.to_string());
                }
            }
        }

        Ok(files)
    }
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
    fn test_is_clean() {
        let (_temp, repo) = setup_repo();
        assert!(repo.is_clean().unwrap());
    }

    #[test]
    fn test_is_dirty() {
        let (temp, repo) = setup_repo();
        std::fs::write(temp.path().join("new_file.txt"), "new").unwrap();
        assert!(!repo.is_clean().unwrap());
    }

    #[test]
    fn test_current_branch() {
        let (_temp, repo) = setup_repo();
        let branch = repo.current_branch().unwrap();
        // Git might default to 'master' or 'main' depending on config
        assert!(branch.is_some());
    }

    #[test]
    fn test_modified_files() {
        let (temp, repo) = setup_repo();
        std::fs::write(temp.path().join("file.txt"), "modified").unwrap();
        let modified = repo.modified_files().unwrap();
        assert!(modified.contains(&"file.txt".to_string()));
    }
}
