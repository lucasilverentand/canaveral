//! Configuration loading

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::{ConfigError, Result};

use super::defaults::{config_file_names, LEGACY_YAML_NAMES};
use super::root::Config;
use super::validation::validate_config;

/// Load configuration from a TOML file
pub fn load_config(path: &Path) -> Result<Config> {
    info!(path = %path.display(), "loading config");

    if !path.extension().is_some_and(|e| e == "toml") {
        return Err(ConfigError::UnsupportedFormat(format!(
            "Found '{}'. Canaveral only supports TOML configuration. \
             Rename your config to canaveral.toml and convert the syntax.",
            path.display()
        ))
        .into());
    }

    let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;
    let config: Config = toml::from_str(&content).map_err(ConfigError::TomlError)?;

    validate_config(&config)?;
    debug!(path = %path.display(), "config loaded and validated");
    Ok(config)
}

/// Find configuration file in directory or parent directories.
///
/// At each directory level the search checks:
///   1. `<dir>/<name>`          (e.g. `canaveral.toml`)
///   2. `<dir>/.github/<name>`  (e.g. `.github/canaveral.toml`)
///
/// The first match wins. Parents are walked until the filesystem root.
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    debug!(start_dir = %start_dir.display(), "searching for config file");
    let mut current = start_dir.to_path_buf();

    loop {
        for name in config_file_names() {
            // Check the directory itself
            let config_path = current.join(name);
            if config_path.exists() {
                info!(path = %config_path.display(), "found config file");
                return Some(config_path);
            }

            // Check .github/ subdirectory
            let github_path = current.join(".github").join(name);
            if github_path.exists() {
                info!(path = %github_path.display(), "found config file in .github/");
                return Some(github_path);
            }
        }

        if !current.pop() {
            break;
        }
    }

    // Check for legacy YAML files and warn
    let mut current = start_dir.to_path_buf();
    loop {
        for name in LEGACY_YAML_NAMES {
            let yaml_path = current.join(name);
            if yaml_path.exists() {
                warn!(
                    path = %yaml_path.display(),
                    "Found legacy YAML config file. Canaveral now only supports TOML. \
                     Please rename to canaveral.toml and convert the syntax."
                );
            }
        }
        if !current.pop() {
            break;
        }
    }

    debug!("no config file found");
    None
}

/// Load configuration from directory (searching parent directories)
pub fn load_config_from_dir(dir: &Path) -> Result<(Config, PathBuf)> {
    let config_path = find_config(dir).ok_or_else(|| ConfigError::NotFound(dir.to_path_buf()))?;

    let config = load_config(&config_path)?;
    Ok((config, config_path))
}

/// Load configuration or use defaults
pub fn load_config_or_default(dir: &Path) -> (Config, Option<PathBuf>) {
    match load_config_from_dir(dir) {
        Ok((config, path)) => (config, Some(path)),
        Err(_) => {
            warn!(dir = %dir.display(), "no config found, using defaults");
            (Config::default(), None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_find_config_toml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("canaveral.toml");
        std::fs::write(&config_path, "[versioning]\nstrategy = \"semver\"").unwrap();

        let found = find_config(temp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), config_path);
    }

    #[test]
    fn test_find_config_ignores_yaml() {
        let temp = TempDir::new().unwrap();
        let yaml_path = temp.path().join("canaveral.yaml");
        std::fs::write(&yaml_path, "versioning:\n  strategy: semver").unwrap();

        // YAML-only should not be found
        let found = find_config(temp.path());
        assert!(found.is_none());
    }

    #[test]
    fn test_find_config_in_github_dir() {
        let temp = TempDir::new().unwrap();
        let github_dir = temp.path().join(".github");
        std::fs::create_dir_all(&github_dir).unwrap();
        let config_path = github_dir.join("canaveral.toml");
        std::fs::write(&config_path, "[versioning]\nstrategy = \"semver\"").unwrap();

        let found = find_config(temp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), config_path);
    }

    #[test]
    fn test_root_level_preferred_over_github_dir() {
        let temp = TempDir::new().unwrap();
        let root_path = temp.path().join("canaveral.toml");
        let github_dir = temp.path().join(".github");
        std::fs::create_dir_all(&github_dir).unwrap();
        let github_path = github_dir.join("canaveral.toml");
        std::fs::write(&root_path, "[versioning]\nstrategy = \"semver\"").unwrap();
        std::fs::write(&github_path, "[versioning]\nstrategy = \"calver\"").unwrap();

        let found = find_config(temp.path()).unwrap();
        assert_eq!(found, root_path);
    }

    #[test]
    fn test_load_config_toml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("canaveral.toml");
        std::fs::write(
            &config_path,
            "[versioning]\nstrategy = \"semver\"\n\n[git]\nremote = \"origin\"\n",
        )
        .unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.versioning.strategy, "semver");
    }

    #[test]
    fn test_load_config_yaml_returns_unsupported() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("canaveral.yaml");
        std::fs::write(
            &config_path,
            "versioning:\n  strategy: semver\ngit:\n  remote: origin\n",
        )
        .unwrap();

        let result = load_config(&config_path);
        assert!(matches!(
            result,
            Err(crate::error::CanaveralError::Config(
                ConfigError::UnsupportedFormat(_)
            ))
        ));
    }
}
