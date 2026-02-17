//! Configuration validation

use tracing::debug;

use crate::error::{ConfigError, Result};

use super::types::Config;

/// Validate configuration
pub fn validate_config(config: &Config) -> Result<()> {
    debug!("validating configuration");
    validate_versioning(config)?;
    validate_git(config)?;
    validate_changelog(config)?;
    validate_packages(config)?;
    debug!("configuration validation passed");
    Ok(())
}

fn validate_versioning(config: &Config) -> Result<()> {
    let valid_strategies = ["semver", "calver", "build"];
    if !valid_strategies.contains(&config.versioning.strategy.as_str()) {
        return Err(ConfigError::InvalidValue {
            field: "versioning.strategy".to_string(),
            message: format!("must be one of: {}", valid_strategies.join(", ")),
        }
        .into());
    }

    if !config.versioning.tag_format.contains("{version}") {
        return Err(ConfigError::InvalidValue {
            field: "versioning.tag_format".to_string(),
            message: "must contain {version} placeholder".to_string(),
        }
        .into());
    }

    Ok(())
}

fn validate_git(config: &Config) -> Result<()> {
    if config.git.remote.is_empty() {
        return Err(ConfigError::InvalidValue {
            field: "git.remote".to_string(),
            message: "remote cannot be empty".to_string(),
        }
        .into());
    }

    if config.git.branch.is_empty() {
        return Err(ConfigError::InvalidValue {
            field: "git.branch".to_string(),
            message: "branch cannot be empty".to_string(),
        }
        .into());
    }

    if !config.git.commit_message.contains("{version}") {
        return Err(ConfigError::InvalidValue {
            field: "git.commit_message".to_string(),
            message: "must contain {version} placeholder".to_string(),
        }
        .into());
    }

    Ok(())
}

fn validate_changelog(config: &Config) -> Result<()> {
    if config.changelog.enabled {
        let valid_formats = ["markdown", "md", "json", "plain"];
        if !valid_formats.contains(&config.changelog.format.as_str()) {
            return Err(ConfigError::InvalidValue {
                field: "changelog.format".to_string(),
                message: format!("must be one of: {}", valid_formats.join(", ")),
            }
            .into());
        }
    }

    Ok(())
}

fn validate_packages(config: &Config) -> Result<()> {
    if !config.packages.is_empty() {
        debug!(count = config.packages.len(), "validating packages");
    }
    for (i, package) in config.packages.iter().enumerate() {
        if package.name.is_empty() {
            return Err(ConfigError::InvalidValue {
                field: format!("packages[{}].name", i),
                message: "package name cannot be empty".to_string(),
            }
            .into());
        }

        let valid_types = ["npm", "cargo", "python", "go", "maven", "docker"];
        if !valid_types.contains(&package.package_type.as_str()) {
            return Err(ConfigError::InvalidValue {
                field: format!("packages[{}].type", i),
                message: format!("must be one of: {}", valid_types.join(", ")),
            }
            .into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_default_config() {
        let config = Config::default();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_invalid_strategy() {
        let mut config = Config::default();
        config.versioning.strategy = "invalid".to_string();
        assert!(validate_config(&config).is_err());
    }

    #[test]
    fn test_validate_invalid_tag_format() {
        let mut config = Config::default();
        config.versioning.tag_format = "no-placeholder".to_string();
        assert!(validate_config(&config).is_err());
    }
}
