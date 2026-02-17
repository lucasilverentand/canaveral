//! Git repository operations

use std::path::{Path, PathBuf};

use git2::Repository;
use tracing::{info, instrument};

use canaveral_core::error::GitError;

/// Result type for git operations
pub type Result<T> = std::result::Result<T, GitError>;

/// Git repository wrapper
pub struct GitRepo {
    pub(crate) repo: Repository,
    path: PathBuf,
}

impl GitRepo {
    /// Open a repository at the given path
    #[instrument(fields(path = %path.display()))]
    pub fn open(path: &Path) -> Result<Self> {
        info!(path = %path.display(), "opening git repository");
        let repo = Repository::open(path).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                GitError::RepositoryNotFound(path.to_path_buf())
            } else {
                GitError::OpenFailed(e.to_string())
            }
        })?;

        Ok(Self {
            path: path.to_path_buf(),
            repo,
        })
    }

    /// Discover and open a repository by searching parent directories
    #[instrument(fields(start_path = %start_path.display()))]
    pub fn discover(start_path: &Path) -> Result<Self> {
        info!(start_path = %start_path.display(), "discovering git repository");
        let repo = Repository::discover(start_path).map_err(|e| {
            if e.code() == git2::ErrorCode::NotFound {
                GitError::NotARepository(start_path.to_path_buf())
            } else {
                GitError::OpenFailed(e.to_string())
            }
        })?;

        let path = repo.workdir().unwrap_or_else(|| repo.path()).to_path_buf();

        Ok(Self { repo, path })
    }

    /// Get the repository path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the workdir path
    pub fn workdir(&self) -> Option<&Path> {
        self.repo.workdir()
    }

    /// Get a reference to the inner git2 Repository
    pub fn inner(&self) -> &Repository {
        &self.repo
    }

    /// Check if the repository is bare
    pub fn is_bare(&self) -> bool {
        self.repo.is_bare()
    }

    /// Get the HEAD reference
    pub fn head(&self) -> Result<git2::Reference<'_>> {
        self.repo.head().map_err(GitError::Git2)
    }

    /// Get the HEAD commit
    pub fn head_commit(&self) -> Result<git2::Commit<'_>> {
        let head = self.head()?;
        head.peel_to_commit().map_err(GitError::Git2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn init_repo() -> (TempDir, GitRepo) {
        let temp = TempDir::new().unwrap();
        Repository::init(temp.path()).unwrap();
        let repo = GitRepo::open(temp.path()).unwrap();
        (temp, repo)
    }

    #[test]
    fn test_open_repo() {
        let (_temp, repo) = init_repo();
        assert!(!repo.is_bare());
    }

    #[test]
    fn test_discover_repo() {
        let temp = TempDir::new().unwrap();
        Repository::init(temp.path()).unwrap();

        let subdir = temp.path().join("sub").join("dir");
        std::fs::create_dir_all(&subdir).unwrap();

        let repo = GitRepo::discover(&subdir).unwrap();
        // Canonicalize both paths to handle macOS /var -> /private/var symlink
        let repo_path = repo.path().canonicalize().unwrap();
        let temp_path = temp.path().canonicalize().unwrap();
        assert_eq!(repo_path, temp_path);
    }

    #[test]
    fn test_not_a_repo() {
        let temp = TempDir::new().unwrap();
        let result = GitRepo::open(temp.path());
        assert!(result.is_err());
    }
}
