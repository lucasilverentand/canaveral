//! Orchestrator configuration
//!
//! Configuration for the orchestrator, including CI/CD-specific settings,
//! retry behavior, and output formatting.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::output::OutputFormat;

/// Configuration for the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorConfig {
    /// Whether running in CI mode (auto-detected from CI env var)
    pub ci: bool,

    /// Output format
    pub output_format: OutputFormat,

    /// Whether to output JSON (shorthand for output_format = Json)
    pub json_output: bool,

    /// Quiet mode - suppress non-essential output
    pub quiet: bool,

    /// Verbose mode - extra debug output
    pub verbose: bool,

    /// Maximum retries for retryable operations
    pub max_retries: u32,

    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,

    /// Whether to check prerequisites before operations
    pub check_prerequisites: bool,

    /// Timeout for operations in seconds (0 = no timeout)
    pub timeout_secs: u64,

    /// Working directory (defaults to current dir)
    pub working_dir: Option<PathBuf>,

    /// Environment variables to pass to subprocesses
    pub env: HashMap<String, String>,

    /// Framework-specific configuration
    pub framework_config: HashMap<String, serde_json::Value>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        let ci = std::env::var("CI").is_ok();

        Self {
            ci,
            output_format: if ci {
                OutputFormat::from_env()
            } else {
                OutputFormat::Text
            },
            json_output: false,
            quiet: false,
            verbose: false,
            max_retries: if ci { 2 } else { 0 },
            retry_delay_ms: 1000,
            check_prerequisites: true,
            timeout_secs: 0,
            working_dir: None,
            env: HashMap::new(),
            framework_config: HashMap::new(),
        }
    }
}

impl OrchestratorConfig {
    /// Create a new config
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a config optimized for CI
    pub fn for_ci() -> Self {
        Self {
            ci: true,
            output_format: OutputFormat::from_env(),
            json_output: true,
            quiet: false,
            verbose: false,
            max_retries: 2,
            retry_delay_ms: 2000,
            check_prerequisites: true,
            timeout_secs: 1800, // 30 minutes default for CI
            ..Default::default()
        }
    }

    /// Create a config optimized for local development
    pub fn for_local() -> Self {
        Self {
            ci: false,
            output_format: OutputFormat::Text,
            json_output: false,
            quiet: false,
            verbose: false,
            max_retries: 0,
            retry_delay_ms: 1000,
            check_prerequisites: true,
            timeout_secs: 0,
            ..Default::default()
        }
    }

    /// Load config from environment variables
    pub fn from_env() -> Self {
        let mut config = if std::env::var("CI").is_ok() {
            Self::for_ci()
        } else {
            Self::for_local()
        };

        // Override from env vars
        if let Ok(v) = std::env::var("CANAVERAL_OUTPUT_FORMAT") {
            if let Some(fmt) = OutputFormat::parse(&v) {
                config.output_format = fmt;
            }
        }

        if std::env::var("CANAVERAL_JSON").is_ok() {
            config.json_output = true;
            config.output_format = OutputFormat::Json;
        }

        if std::env::var("CANAVERAL_QUIET").is_ok() || std::env::var("CANAVERAL_SILENT").is_ok() {
            config.quiet = true;
        }

        if std::env::var("CANAVERAL_VERBOSE").is_ok() {
            config.verbose = true;
        }

        if let Ok(v) = std::env::var("CANAVERAL_MAX_RETRIES") {
            if let Ok(n) = v.parse() {
                config.max_retries = n;
            }
        }

        if let Ok(v) = std::env::var("CANAVERAL_TIMEOUT") {
            if let Ok(n) = v.parse() {
                config.timeout_secs = n;
            }
        }

        if let Ok(v) = std::env::var("CANAVERAL_WORKING_DIR") {
            config.working_dir = Some(PathBuf::from(v));
        }

        config
    }

    // Builder methods

    pub fn with_ci(mut self, ci: bool) -> Self {
        self.ci = ci;
        self
    }

    pub fn with_output_format(mut self, format: OutputFormat) -> Self {
        self.output_format = format;
        self
    }

    pub fn with_json(mut self, json: bool) -> Self {
        self.json_output = json;
        if json {
            self.output_format = OutputFormat::Json;
        }
        self
    }

    pub fn with_quiet(mut self, quiet: bool) -> Self {
        self.quiet = quiet;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    pub fn with_working_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_framework_config(
        mut self,
        framework: impl Into<String>,
        config: serde_json::Value,
    ) -> Self {
        self.framework_config.insert(framework.into(), config);
        self
    }

    /// Get effective output format
    pub fn effective_output_format(&self) -> OutputFormat {
        if self.json_output {
            OutputFormat::Json
        } else {
            self.output_format
        }
    }

    /// Get working directory (current dir if not set)
    pub fn effective_working_dir(&self) -> PathBuf {
        self.working_dir
            .clone()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = OrchestratorConfig::default();
        assert!(config.check_prerequisites);
    }

    #[test]
    fn test_ci_config() {
        let config = OrchestratorConfig::for_ci();
        assert!(config.ci);
        assert!(config.json_output);
        assert_eq!(config.max_retries, 2);
        assert!(config.timeout_secs > 0);
    }

    #[test]
    fn test_local_config() {
        let config = OrchestratorConfig::for_local();
        assert!(!config.ci);
        assert!(!config.json_output);
        assert_eq!(config.max_retries, 0);
    }

    #[test]
    fn test_builder() {
        let config = OrchestratorConfig::new()
            .with_ci(true)
            .with_json(true)
            .with_max_retries(5)
            .with_timeout(3600)
            .with_env("MY_VAR", "my_value");

        assert!(config.ci);
        assert!(config.json_output);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.timeout_secs, 3600);
        assert_eq!(config.env.get("MY_VAR"), Some(&"my_value".to_string()));
    }
}
