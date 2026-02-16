//! Credential management for package registries

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use canaveral_core::error::{AdapterError, Result};

/// Credential provider for registry authentication
pub struct CredentialProvider {
    /// Environment variable prefix for credentials
    env_prefix: String,
    /// Cached credentials
    cache: HashMap<String, Credential>,
}

impl CredentialProvider {
    /// Create a new credential provider
    pub fn new() -> Self {
        Self {
            env_prefix: "CANAVERAL".to_string(),
            cache: HashMap::new(),
        }
    }

    /// Set the environment variable prefix
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }

    /// Get credentials for a registry
    #[instrument(skip(self), fields(registry))]
    pub fn get(&mut self, registry: &str) -> Result<Option<Credential>> {
        // Check cache first
        if let Some(cred) = self.cache.get(registry) {
            debug!(registry, source = "cache", "credentials found");
            return Ok(Some(cred.clone()));
        }

        // Try environment variables
        if let Some(cred) = self.from_env(registry)? {
            debug!(registry, source = "environment", "credentials found");
            self.cache.insert(registry.to_string(), cred.clone());
            return Ok(Some(cred));
        }

        // Try registry-specific config files
        if let Some(cred) = self.from_registry_config(registry)? {
            debug!(registry, source = "config_file", "credentials found");
            self.cache.insert(registry.to_string(), cred.clone());
            return Ok(Some(cred));
        }

        debug!(registry, "no credentials found");
        Ok(None)
    }

    /// Get credentials from environment variables
    fn from_env(&self, registry: &str) -> Result<Option<Credential>> {
        let registry_upper = registry.to_uppercase().replace(['.', '-', '/'], "_");

        // Try TOKEN first (most common)
        let token_var = format!("{}_{}_TOKEN", self.env_prefix, registry_upper);
        if let Ok(token) = env::var(&token_var) {
            return Ok(Some(Credential::Token(token)));
        }

        // Try generic registry token patterns
        match registry {
            "npm" | "npmjs" => {
                if let Ok(token) = env::var("NPM_TOKEN") {
                    return Ok(Some(Credential::Token(token)));
                }
                if let Ok(token) = env::var("NODE_AUTH_TOKEN") {
                    return Ok(Some(Credential::Token(token)));
                }
            }
            "cargo" | "crates.io" => {
                if let Ok(token) = env::var("CARGO_REGISTRY_TOKEN") {
                    return Ok(Some(Credential::Token(token)));
                }
                if let Ok(token) = env::var("CRATES_IO_TOKEN") {
                    return Ok(Some(Credential::Token(token)));
                }
            }
            "pypi" | "python" => {
                if let Ok(token) = env::var("TWINE_PASSWORD") {
                    let username = env::var("TWINE_USERNAME").unwrap_or_else(|_| "__token__".to_string());
                    return Ok(Some(Credential::UsernamePassword { username, password: token }));
                }
                if let Ok(token) = env::var("PYPI_TOKEN") {
                    return Ok(Some(Credential::Token(token)));
                }
            }
            _ => {}
        }

        // Try username/password combo
        let user_var = format!("{}_{}_USERNAME", self.env_prefix, registry_upper);
        let pass_var = format!("{}_{}_PASSWORD", self.env_prefix, registry_upper);

        if let (Ok(username), Ok(password)) = (env::var(&user_var), env::var(&pass_var)) {
            return Ok(Some(Credential::UsernamePassword { username, password }));
        }

        Ok(None)
    }

    /// Get credentials from registry-specific config files
    fn from_registry_config(&self, registry: &str) -> Result<Option<Credential>> {
        match registry {
            "npm" | "npmjs" => self.from_npmrc(),
            "cargo" | "crates.io" => self.from_cargo_credentials(),
            "pypi" | "python" => self.from_pypirc(),
            _ => Ok(None),
        }
    }

    /// Read npm credentials from .npmrc
    fn from_npmrc(&self) -> Result<Option<Credential>> {
        let home = dirs::home_dir().ok_or_else(|| {
            AdapterError::AuthenticationFailed {
                registry: "npm".to_string(),
                reason: "Could not determine home directory".to_string(),
            }
        })?;

        let npmrc_path = home.join(".npmrc");
        if !npmrc_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&npmrc_path).map_err(|e| {
            AdapterError::AuthenticationFailed {
                registry: "npm".to_string(),
                reason: format!("Failed to read .npmrc: {}", e),
            }
        })?;

        // Look for //registry.npmjs.org/:_authToken=TOKEN pattern
        for line in content.lines() {
            let line = line.trim();
            if line.contains(":_authToken=") {
                if let Some(token) = line.split(":_authToken=").nth(1) {
                    let token = token.trim();
                    // Handle ${ENV_VAR} syntax
                    if token.starts_with("${") && token.ends_with('}') {
                        let var_name = &token[2..token.len() - 1];
                        if let Ok(value) = env::var(var_name) {
                            return Ok(Some(Credential::Token(value)));
                        }
                    } else {
                        return Ok(Some(Credential::Token(token.to_string())));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Read Cargo credentials from credentials.toml
    fn from_cargo_credentials(&self) -> Result<Option<Credential>> {
        let cargo_home = env::var("CARGO_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs::home_dir().map(|h| h.join(".cargo")))
            .ok_or_else(|| AdapterError::AuthenticationFailed {
                registry: "cargo".to_string(),
                reason: "Could not determine CARGO_HOME".to_string(),
            })?;

        let creds_path = cargo_home.join("credentials.toml");
        if !creds_path.exists() {
            // Also try credentials (without .toml)
            let alt_path = cargo_home.join("credentials");
            if !alt_path.exists() {
                return Ok(None);
            }
            return self.parse_cargo_credentials(&alt_path);
        }

        self.parse_cargo_credentials(&creds_path)
    }

    fn parse_cargo_credentials(&self, path: &PathBuf) -> Result<Option<Credential>> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AdapterError::AuthenticationFailed {
                registry: "cargo".to_string(),
                reason: format!("Failed to read credentials: {}", e),
            }
        })?;

        #[derive(Deserialize)]
        struct CargoCredentials {
            registry: Option<RegistryCredentials>,
        }

        #[derive(Deserialize)]
        struct RegistryCredentials {
            token: Option<String>,
        }

        let creds: CargoCredentials = toml::from_str(&content).map_err(|e| {
            AdapterError::AuthenticationFailed {
                registry: "cargo".to_string(),
                reason: format!("Failed to parse credentials: {}", e),
            }
        })?;

        if let Some(registry) = creds.registry {
            if let Some(token) = registry.token {
                return Ok(Some(Credential::Token(token)));
            }
        }

        Ok(None)
    }

    /// Read PyPI credentials from .pypirc
    fn from_pypirc(&self) -> Result<Option<Credential>> {
        let home = dirs::home_dir().ok_or_else(|| {
            AdapterError::AuthenticationFailed {
                registry: "pypi".to_string(),
                reason: "Could not determine home directory".to_string(),
            }
        })?;

        let pypirc_path = home.join(".pypirc");
        if !pypirc_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&pypirc_path).map_err(|e| {
            AdapterError::AuthenticationFailed {
                registry: "pypi".to_string(),
                reason: format!("Failed to read .pypirc: {}", e),
            }
        })?;

        // Simple INI-style parsing for [pypi] section
        let mut in_pypi_section = false;
        let mut username: Option<String> = None;
        let mut password: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                in_pypi_section = line == "[pypi]";
                continue;
            }

            if in_pypi_section {
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "username" => username = Some(value.to_string()),
                        "password" => password = Some(value.to_string()),
                        _ => {}
                    }
                }
            }
        }

        if let (Some(username), Some(password)) = (username, password) {
            return Ok(Some(Credential::UsernamePassword { username, password }));
        }

        Ok(None)
    }

    /// Check if credentials are available for a registry
    pub fn has_credentials(&mut self, registry: &str) -> bool {
        let has = self.get(registry).ok().flatten().is_some();
        debug!(registry, has_credentials = has, "credential check");
        has
    }

    /// Clear cached credentials
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

impl Default for CredentialProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Credential types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Credential {
    /// Bearer/API token
    Token(String),
    /// Username and password
    UsernamePassword {
        username: String,
        password: String,
    },
}

impl Credential {
    /// Get the credential as a token string (if applicable)
    pub fn as_token(&self) -> Option<&str> {
        match self {
            Self::Token(t) => Some(t),
            _ => None,
        }
    }

    /// Get username (if applicable)
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::UsernamePassword { username, .. } => Some(username),
            _ => None,
        }
    }

    /// Get password (if applicable)
    pub fn password(&self) -> Option<&str> {
        match self {
            Self::UsernamePassword { password, .. } => Some(password),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_provider_default() {
        let provider = CredentialProvider::new();
        assert_eq!(provider.env_prefix, "CANAVERAL");
    }

    #[test]
    fn test_credential_token() {
        let cred = Credential::Token("my-token".to_string());
        assert_eq!(cred.as_token(), Some("my-token"));
        assert_eq!(cred.username(), None);
    }

    #[test]
    fn test_credential_username_password() {
        let cred = Credential::UsernamePassword {
            username: "user".to_string(),
            password: "pass".to_string(),
        };
        assert_eq!(cred.username(), Some("user"));
        assert_eq!(cred.password(), Some("pass"));
        assert_eq!(cred.as_token(), None);
    }

    #[test]
    fn test_env_credential() {
        // Set environment variable
        env::set_var("NPM_TOKEN", "test-npm-token");

        let mut provider = CredentialProvider::new();
        let cred = provider.get("npm").unwrap();
        assert!(cred.is_some());
        assert_eq!(cred.unwrap().as_token(), Some("test-npm-token"));

        // Clean up
        env::remove_var("NPM_TOKEN");
    }
}
