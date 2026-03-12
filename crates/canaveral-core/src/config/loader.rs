//! Configuration loading
//!
//! Supports:
//! - `${ENV_VAR}` interpolation in any string value
//! - `canaveral.local.toml` for private overrides (gitignored secrets)
//! - Deep merging of local config on top of committed config

use std::path::{Path, PathBuf};

use regex::Regex;
use tracing::{debug, info, warn};

use crate::error::{ConfigError, Result};

use super::defaults::{config_file_names, LEGACY_YAML_NAMES};
use super::root::Config;
use super::validation::validate_config;

/// Interpolate `${VAR}` and `$VAR` references in a string with environment variables.
///
/// - `${VAR}` is replaced with the value of `VAR`, or empty string if unset.
/// - `${VAR:-default}` uses "default" when `VAR` is unset or empty.
/// - Unresolvable references are replaced with empty string (open-source friendly).
fn interpolate_env(input: &str) -> String {
    let re = Regex::new(r"\$\{([^}]+)\}").unwrap();
    re.replace_all(input, |caps: &regex::Captures| {
        let expr = &caps[1];
        if let Some((var, default)) = expr.split_once(":-") {
            std::env::var(var).unwrap_or_else(|_| default.to_string())
        } else {
            std::env::var(expr).unwrap_or_default()
        }
    })
    .to_string()
}

/// Recursively interpolate environment variables in all string values of a TOML table.
fn interpolate_toml_value(value: &mut toml::Value) {
    match value {
        toml::Value::String(s) => {
            if s.contains('$') {
                *s = interpolate_env(s);
            }
        }
        toml::Value::Table(table) => {
            for (_, v) in table.iter_mut() {
                interpolate_toml_value(v);
            }
        }
        toml::Value::Array(arr) => {
            for v in arr.iter_mut() {
                interpolate_toml_value(v);
            }
        }
        _ => {}
    }
}

/// Deep-merge `overlay` into `base`. Overlay values win for scalars;
/// tables are merged recursively; arrays from overlay replace base.
fn deep_merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (key, overlay_val) in overlay_table {
                let entry = base_table.entry(key).or_insert(toml::Value::Boolean(false));
                deep_merge(entry, overlay_val);
            }
        }
        (base, overlay) => {
            *base = overlay;
        }
    }
}

/// Load configuration from a TOML file, with env interpolation and local overrides.
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
    let mut value: toml::Value = toml::from_str(&content).map_err(ConfigError::TomlError)?;

    // Merge canaveral.local.toml if it exists alongside the main config
    if let Some(dir) = path.parent() {
        let local_path = dir.join("canaveral.local.toml");
        if local_path.exists() {
            info!(path = %local_path.display(), "loading local config overlay");
            let local_content = std::fs::read_to_string(&local_path).map_err(ConfigError::Io)?;
            let local_value: toml::Value =
                toml::from_str(&local_content).map_err(ConfigError::TomlError)?;
            deep_merge(&mut value, local_value);
        }
    }

    // Interpolate environment variables in all string values
    interpolate_toml_value(&mut value);

    let config: Config = value
        .try_into()
        .map_err(|e: toml::de::Error| ConfigError::TomlError(e))?;

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

    #[test]
    fn test_interpolate_env_basic() {
        std::env::set_var("CANAVERAL_TEST_VAR", "hello");
        assert_eq!(interpolate_env("${CANAVERAL_TEST_VAR}"), "hello");
        assert_eq!(
            interpolate_env("pre-${CANAVERAL_TEST_VAR}-post"),
            "pre-hello-post"
        );
        std::env::remove_var("CANAVERAL_TEST_VAR");
    }

    #[test]
    fn test_interpolate_env_unset_returns_empty() {
        std::env::remove_var("CANAVERAL_NONEXISTENT_VAR");
        assert_eq!(interpolate_env("${CANAVERAL_NONEXISTENT_VAR}"), "");
    }

    #[test]
    fn test_interpolate_env_default_value() {
        std::env::remove_var("CANAVERAL_MISSING");
        assert_eq!(
            interpolate_env("${CANAVERAL_MISSING:-fallback}"),
            "fallback"
        );

        // When var is set, default is not used
        std::env::set_var("CANAVERAL_PRESENT", "real");
        assert_eq!(interpolate_env("${CANAVERAL_PRESENT:-fallback}"), "real");
        std::env::remove_var("CANAVERAL_PRESENT");
    }

    #[test]
    fn test_interpolate_env_no_vars_passthrough() {
        assert_eq!(interpolate_env("no variables here"), "no variables here");
    }

    #[test]
    fn test_deep_merge() {
        let mut base: toml::Value = toml::from_str(
            r#"
            [versioning]
            strategy = "semver"

            [git]
            remote = "origin"
            branch = "main"
            "#,
        )
        .unwrap();

        let overlay: toml::Value = toml::from_str(
            r#"
            [ios]
            team_id = "SECRET123"

            [git]
            branch = "develop"
            "#,
        )
        .unwrap();

        deep_merge(&mut base, overlay);

        let table = base.as_table().unwrap();
        // Original values preserved
        assert_eq!(table["versioning"]["strategy"].as_str().unwrap(), "semver");
        // Git remote preserved, branch overridden
        assert_eq!(table["git"]["remote"].as_str().unwrap(), "origin");
        assert_eq!(table["git"]["branch"].as_str().unwrap(), "develop");
        // New section merged in
        assert_eq!(table["ios"]["team_id"].as_str().unwrap(), "SECRET123");
    }

    #[test]
    fn test_local_config_overlay() {
        let temp = TempDir::new().unwrap();

        // Main config (committed to repo)
        std::fs::write(
            temp.path().join("canaveral.toml"),
            r#"
            [versioning]
            strategy = "semver"

            [ios]
            scheme = "MyApp"
            "#,
        )
        .unwrap();

        // Local config (gitignored, has secrets)
        std::fs::write(
            temp.path().join("canaveral.local.toml"),
            r#"
            [ios]
            team_id = "SECRET_TEAM_ID"
            "#,
        )
        .unwrap();

        let config = load_config(&temp.path().join("canaveral.toml")).unwrap();
        assert_eq!(config.versioning.strategy, "semver");
        assert_eq!(config.ios.scheme, Some("MyApp".to_string()));
        assert_eq!(config.ios.team_id, Some("SECRET_TEAM_ID".to_string()));
    }

    #[test]
    fn test_env_interpolation_in_config() {
        let temp = TempDir::new().unwrap();

        std::env::set_var("CANAVERAL_TEST_TEAM", "TEAM_FROM_ENV");
        std::fs::write(
            temp.path().join("canaveral.toml"),
            r#"
            [ios]
            team_id = "${CANAVERAL_TEST_TEAM}"
            scheme = "MyApp"
            "#,
        )
        .unwrap();

        let config = load_config(&temp.path().join("canaveral.toml")).unwrap();
        assert_eq!(config.ios.team_id, Some("TEAM_FROM_ENV".to_string()));
        assert_eq!(config.ios.scheme, Some("MyApp".to_string()));
        std::env::remove_var("CANAVERAL_TEST_TEAM");
    }
}
