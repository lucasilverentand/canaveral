//! Git hook installation and management
//!
//! Manages `.git/hooks/` scripts that delegate to `canaveral hooks run <name>`.

use std::fmt;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

/// Marker comment embedded in generated hook scripts
const CANAVERAL_MARKER: &str = "# managed by canaveral — do not edit";

/// Types of git hooks canaveral manages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHookType {
    CommitMsg,
    PreCommit,
    PrePush,
}

impl GitHookType {
    /// All managed hook types
    pub fn all() -> &'static [GitHookType] {
        &[
            GitHookType::CommitMsg,
            GitHookType::PreCommit,
            GitHookType::PrePush,
        ]
    }

    /// The filename inside `.git/hooks/`
    pub fn filename(&self) -> &'static str {
        match self {
            GitHookType::CommitMsg => "commit-msg",
            GitHookType::PreCommit => "pre-commit",
            GitHookType::PrePush => "pre-push",
        }
    }
}

impl fmt::Display for GitHookType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.filename())
    }
}

/// Status of a single hook
#[derive(Debug)]
pub struct HookStatus {
    pub hook_type: GitHookType,
    pub installed: bool,
    pub has_backup: bool,
}

/// Generate the shell script for a given hook type
fn hook_script(hook_type: GitHookType) -> String {
    format!(
        r#"#!/bin/sh
{CANAVERAL_MARKER}
exec canaveral hooks run {name} -- "$@"
"#,
        name = hook_type.filename()
    )
}

/// Check whether a file at `path` is a canaveral-managed hook
pub fn is_canaveral_hook(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains(CANAVERAL_MARKER))
        .unwrap_or(false)
}

/// Return the `.git/hooks` directory for a repo root, creating it if needed
fn hooks_dir(repo_root: &Path) -> std::io::Result<PathBuf> {
    let dir = repo_root.join(".git/hooks");
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(dir)
}

/// Backup filename for an existing non-canaveral hook
fn backup_path(hook_path: &Path) -> PathBuf {
    let name = hook_path.file_name().unwrap_or_default().to_string_lossy();
    hook_path.with_file_name(format!("{name}.pre-canaveral"))
}

/// Install a single hook into `.git/hooks/`
pub fn install_hook(
    repo_root: &Path,
    hook_type: GitHookType,
) -> Result<(), canaveral_core::error::GitHookError> {
    let dir =
        hooks_dir(repo_root).map_err(|e| canaveral_core::error::GitHookError::InstallFailed {
            hook: hook_type.to_string(),
            reason: e.to_string(),
        })?;

    let path = dir.join(hook_type.filename());

    // If there is an existing hook that is NOT ours, back it up
    if path.exists() && !is_canaveral_hook(&path) {
        let backup = backup_path(&path);
        info!(hook = %hook_type, backup = %backup.display(), "backing up existing hook");
        fs::rename(&path, &backup).map_err(|e| {
            canaveral_core::error::GitHookError::InstallFailed {
                hook: hook_type.to_string(),
                reason: format!("failed to back up existing hook: {e}"),
            }
        })?;
    }

    let script = hook_script(hook_type);
    fs::write(&path, &script).map_err(|e| canaveral_core::error::GitHookError::InstallFailed {
        hook: hook_type.to_string(),
        reason: e.to_string(),
    })?;

    // Make executable
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(|e| {
        canaveral_core::error::GitHookError::InstallFailed {
            hook: hook_type.to_string(),
            reason: format!("failed to set permissions: {e}"),
        }
    })?;

    debug!(hook = %hook_type, path = %path.display(), "installed hook");
    Ok(())
}

/// Uninstall a single canaveral-managed hook, restoring backup if present
pub fn uninstall_hook(
    repo_root: &Path,
    hook_type: GitHookType,
) -> Result<(), canaveral_core::error::GitHookError> {
    let dir =
        hooks_dir(repo_root).map_err(|e| canaveral_core::error::GitHookError::UninstallFailed {
            hook: hook_type.to_string(),
            reason: e.to_string(),
        })?;

    let path = dir.join(hook_type.filename());

    if !path.exists() {
        debug!(hook = %hook_type, "hook not installed, nothing to remove");
        return Ok(());
    }

    if !is_canaveral_hook(&path) {
        warn!(hook = %hook_type, "hook exists but is not managed by canaveral, skipping");
        return Ok(());
    }

    fs::remove_file(&path).map_err(|e| canaveral_core::error::GitHookError::UninstallFailed {
        hook: hook_type.to_string(),
        reason: e.to_string(),
    })?;

    // Restore backup if one exists
    let backup = backup_path(&path);
    if backup.exists() {
        info!(hook = %hook_type, "restoring pre-canaveral backup");
        fs::rename(&backup, &path).map_err(|e| {
            canaveral_core::error::GitHookError::UninstallFailed {
                hook: hook_type.to_string(),
                reason: format!("failed to restore backup: {e}"),
            }
        })?;
    }

    debug!(hook = %hook_type, "uninstalled hook");
    Ok(())
}

/// Install all managed hooks
pub fn install_all(repo_root: &Path) -> Result<(), canaveral_core::error::GitHookError> {
    for &hook_type in GitHookType::all() {
        install_hook(repo_root, hook_type)?;
    }
    Ok(())
}

/// Uninstall all managed hooks
pub fn uninstall_all(repo_root: &Path) -> Result<(), canaveral_core::error::GitHookError> {
    for &hook_type in GitHookType::all() {
        uninstall_hook(repo_root, hook_type)?;
    }
    Ok(())
}

/// Return the status of each managed hook
pub fn status(repo_root: &Path) -> Vec<HookStatus> {
    let dir = repo_root.join(".git/hooks");

    GitHookType::all()
        .iter()
        .map(|&hook_type| {
            let path = dir.join(hook_type.filename());
            let installed = path.exists() && is_canaveral_hook(&path);
            let has_backup = backup_path(&path).exists();
            HookStatus {
                hook_type,
                installed,
                has_backup,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_repo() -> TempDir {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path().join(".git/hooks")).unwrap();
        dir
    }

    #[test]
    fn test_hook_script_contains_marker() {
        let script = hook_script(GitHookType::PreCommit);
        assert!(script.contains(CANAVERAL_MARKER));
        assert!(script.contains("canaveral hooks run pre-commit"));
    }

    #[test]
    fn test_install_and_detect() {
        let repo = setup_repo();
        install_hook(repo.path(), GitHookType::PreCommit).unwrap();

        let path = repo.path().join(".git/hooks/pre-commit");
        assert!(path.exists());
        assert!(is_canaveral_hook(&path));
    }

    #[test]
    fn test_install_backs_up_existing() {
        let repo = setup_repo();
        let hook_path = repo.path().join(".git/hooks/pre-commit");
        fs::write(&hook_path, "#!/bin/sh\necho old hook\n").unwrap();

        install_hook(repo.path(), GitHookType::PreCommit).unwrap();

        assert!(is_canaveral_hook(&hook_path));
        let backup = repo.path().join(".git/hooks/pre-commit.pre-canaveral");
        assert!(backup.exists());
        assert_eq!(
            fs::read_to_string(&backup).unwrap(),
            "#!/bin/sh\necho old hook\n"
        );
    }

    #[test]
    fn test_uninstall_removes_hook() {
        let repo = setup_repo();
        install_hook(repo.path(), GitHookType::CommitMsg).unwrap();

        let path = repo.path().join(".git/hooks/commit-msg");
        assert!(path.exists());

        uninstall_hook(repo.path(), GitHookType::CommitMsg).unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_uninstall_restores_backup() {
        let repo = setup_repo();
        let hook_path = repo.path().join(".git/hooks/pre-push");
        fs::write(&hook_path, "#!/bin/sh\necho original\n").unwrap();

        install_hook(repo.path(), GitHookType::PrePush).unwrap();
        uninstall_hook(repo.path(), GitHookType::PrePush).unwrap();

        assert!(hook_path.exists());
        assert_eq!(
            fs::read_to_string(&hook_path).unwrap(),
            "#!/bin/sh\necho original\n"
        );
    }

    #[test]
    fn test_install_all_and_status() {
        let repo = setup_repo();
        install_all(repo.path()).unwrap();

        let statuses = status(repo.path());
        assert_eq!(statuses.len(), 3);
        for s in &statuses {
            assert!(s.installed, "{} should be installed", s.hook_type);
        }
    }

    #[test]
    fn test_uninstall_all() {
        let repo = setup_repo();
        install_all(repo.path()).unwrap();
        uninstall_all(repo.path()).unwrap();

        let statuses = status(repo.path());
        for s in &statuses {
            assert!(!s.installed, "{} should not be installed", s.hook_type);
        }
    }

    #[test]
    fn test_uninstall_skips_non_canaveral_hook() {
        let repo = setup_repo();
        let hook_path = repo.path().join(".git/hooks/pre-commit");
        fs::write(&hook_path, "#!/bin/sh\necho foreign\n").unwrap();

        uninstall_hook(repo.path(), GitHookType::PreCommit).unwrap();
        // Should still exist because it's not ours
        assert!(hook_path.exists());
    }
}
