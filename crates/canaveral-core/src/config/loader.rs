//! Configuration loading

use std::path::{Path, PathBuf};

use tracing::{debug, info, warn};

use crate::error::{ConfigError, Result};

use super::defaults::config_file_names;
use super::types::Config;
use super::validation::validate_config;

/// Load configuration from a file
pub fn load_config(path: &Path) -> Result<Config> {
    let format = if path.extension().is_some_and(|e| e == "toml") {
        "TOML"
    } else {
        "YAML"
    };
    info!(path = %path.display(), format, "loading config");

    let content = std::fs::read_to_string(path).map_err(ConfigError::Io)?;

    let config: Config = if format == "TOML" {
        toml::from_str(&content).map_err(ConfigError::TomlError)?
    } else {
        serde_yaml::from_str(&content).map_err(ConfigError::YamlError)?
    };

    validate_config(&config)?;
    debug!(path = %path.display(), "config loaded and validated");
    Ok(config)
}

/// Find configuration file in directory or parent directories
pub fn find_config(start_dir: &Path) -> Option<PathBuf> {
    debug!(start_dir = %start_dir.display(), "searching for config file");
    let mut current = start_dir.to_path_buf();

    loop {
        for name in config_file_names() {
            let config_path = current.join(name);
            if config_path.exists() {
                info!(path = %config_path.display(), "found config file");
                return Some(config_path);
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
    let config_path =
        find_config(dir).ok_or_else(|| ConfigError::NotFound(dir.to_path_buf()))?;

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
    fn test_find_config() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("canaveral.yaml");
        std::fs::write(&config_path, "versioning:\n  strategy: semver").unwrap();

        let found = find_config(temp.path());
        assert!(found.is_some());
        assert_eq!(found.unwrap(), config_path);
    }

    #[test]
    fn test_load_config_yaml() {
        let temp = TempDir::new().unwrap();
        let config_path = temp.path().join("canaveral.yaml");
        std::fs::write(
            &config_path,
            r#"
versioning:
  strategy: semver
git:
  remote: origin
"#,
        )
        .unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(config.versioning.strategy, "semver");
    }
}
