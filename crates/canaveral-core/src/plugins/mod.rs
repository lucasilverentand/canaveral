//! Plugin System - Dynamic loading of external adapters and strategies
//!
//! Canaveral supports loading external plugins that provide:
//! - Custom package adapters (new package managers)
//! - Custom version strategies
//! - Custom changelog formatters
//! - Custom hooks
//!
//! Plugins can be loaded from:
//! - Local paths (shared libraries)
//! - Configuration-defined commands (external executables)

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use crate::error::{CanaveralError, Result};

/// Plugin types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PluginType {
    /// Package adapter plugin
    Adapter,
    /// Version strategy plugin
    Strategy,
    /// Changelog formatter plugin
    Formatter,
}

impl PluginType {
    /// Get type as string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Adapter => "adapter",
            Self::Strategy => "strategy",
            Self::Formatter => "formatter",
        }
    }
}

/// Plugin metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInfo {
    /// Plugin name
    pub name: String,
    /// Plugin version
    pub version: String,
    /// Plugin type
    pub plugin_type: PluginType,
    /// Description
    pub description: Option<String>,
    /// Author
    pub author: Option<String>,
    /// Plugin capabilities
    pub capabilities: Vec<String>,
}

/// Plugin configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Plugin name
    pub name: String,
    /// Plugin type
    pub plugin_type: PluginType,
    /// Path to plugin executable or library
    pub path: Option<PathBuf>,
    /// Command to execute (for executable plugins)
    pub command: Option<String>,
    /// Plugin-specific configuration
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
    /// Whether the plugin is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

/// External plugin that runs as a subprocess
#[derive(Debug, Clone)]
pub struct ExternalPlugin {
    /// Plugin info
    info: PluginInfo,
    /// Command to run
    command: String,
    /// Working directory
    cwd: Option<PathBuf>,
    /// Plugin configuration
    config: HashMap<String, serde_json::Value>,
}

impl ExternalPlugin {
    /// Create a new external plugin
    pub fn new(info: PluginInfo, command: impl Into<String>) -> Self {
        Self {
            info,
            command: command.into(),
            cwd: None,
            config: HashMap::new(),
        }
    }

    /// Set working directory
    pub fn with_cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Set configuration
    pub fn with_config(mut self, config: HashMap<String, serde_json::Value>) -> Self {
        self.config = config;
        self
    }

    /// Get plugin info
    pub fn info(&self) -> &PluginInfo {
        &self.info
    }

    /// Execute a plugin action
    pub fn execute(&self, action: &str, input: &serde_json::Value) -> Result<serde_json::Value> {
        let request = PluginRequest {
            action: action.to_string(),
            input: input.clone(),
            config: self.config.clone(),
        };

        let request_json =
            serde_json::to_string(&request).map_err(|e| CanaveralError::other(e.to_string()))?;

        let mut cmd = Command::new(&self.command);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        if let Some(ref cwd) = self.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| CanaveralError::other(format!("Failed to spawn plugin: {}", e)))?;

        // Write request to stdin
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(request_json.as_bytes())
                .map_err(|e| CanaveralError::other(format!("Failed to write to plugin: {}", e)))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| CanaveralError::other(format!("Plugin execution failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CanaveralError::other(format!(
                "Plugin '{}' failed: {}",
                self.info.name, stderr
            )));
        }

        let response: PluginResponse = serde_json::from_slice(&output.stdout)
            .map_err(|e| CanaveralError::other(format!("Invalid plugin response: {}", e)))?;

        if let Some(error) = response.error {
            return Err(CanaveralError::other(format!(
                "Plugin '{}' error: {}",
                self.info.name, error
            )));
        }

        Ok(response.output.unwrap_or(serde_json::Value::Null))
    }
}

/// Plugin request format (sent to plugin via stdin)
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginRequest {
    /// Action to perform
    pub action: String,
    /// Input data
    pub input: serde_json::Value,
    /// Plugin configuration
    pub config: HashMap<String, serde_json::Value>,
}

/// Plugin response format (received from plugin via stdout)
#[derive(Debug, Serialize, Deserialize)]
pub struct PluginResponse {
    /// Output data (on success)
    pub output: Option<serde_json::Value>,
    /// Error message (on failure)
    pub error: Option<String>,
}

/// Plugin registry
#[derive(Debug, Default)]
pub struct PluginRegistry {
    /// Loaded plugins by type and name
    plugins: HashMap<PluginType, HashMap<String, ExternalPlugin>>,
    /// Plugin search paths
    search_paths: Vec<PathBuf>,
}

impl PluginRegistry {
    /// Create a new plugin registry
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a search path for plugins
    pub fn add_search_path(&mut self, path: impl Into<PathBuf>) {
        self.search_paths.push(path.into());
    }

    /// Register a plugin
    pub fn register(&mut self, plugin: ExternalPlugin) {
        let plugins = self.plugins.entry(plugin.info.plugin_type).or_default();
        plugins.insert(plugin.info.name.clone(), plugin);
    }

    /// Get a plugin by type and name
    pub fn get(&self, plugin_type: PluginType, name: &str) -> Option<&ExternalPlugin> {
        self.plugins
            .get(&plugin_type)
            .and_then(|m| m.get(name))
    }

    /// List plugins of a type
    pub fn list(&self, plugin_type: PluginType) -> Vec<&ExternalPlugin> {
        self.plugins
            .get(&plugin_type)
            .map(|m| m.values().collect())
            .unwrap_or_default()
    }

    /// List all plugins
    pub fn list_all(&self) -> Vec<&ExternalPlugin> {
        self.plugins.values().flat_map(|m| m.values()).collect()
    }

    /// Load plugins from configuration
    pub fn load_from_configs(&mut self, configs: &[PluginConfig]) -> Result<()> {
        for config in configs {
            if !config.enabled {
                continue;
            }

            let command = if let Some(ref cmd) = config.command {
                cmd.clone()
            } else if let Some(ref path) = config.path {
                path.to_string_lossy().to_string()
            } else {
                continue;
            };

            // Query plugin for its info
            let info = self.query_plugin_info(&command, &config.name, config.plugin_type)?;

            let plugin = ExternalPlugin::new(info, command).with_config(config.config.clone());

            self.register(plugin);
        }

        Ok(())
    }

    /// Query a plugin for its info
    fn query_plugin_info(
        &self,
        command: &str,
        fallback_name: &str,
        plugin_type: PluginType,
    ) -> Result<PluginInfo> {
        let request = PluginRequest {
            action: "info".to_string(),
            input: serde_json::Value::Null,
            config: HashMap::new(),
        };

        let request_json =
            serde_json::to_string(&request).map_err(|e| CanaveralError::other(e.to_string()))?;

        let output = Command::new(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                if let Some(mut stdin) = child.stdin.take() {
                    stdin.write_all(request_json.as_bytes())?;
                }
                child.wait_with_output()
            });

        match output {
            Ok(out) if out.status.success() => {
                let response: PluginResponse = serde_json::from_slice(&out.stdout)
                    .map_err(|e| CanaveralError::other(e.to_string()))?;

                if let Some(output) = response.output {
                    serde_json::from_value(output)
                        .map_err(|e| CanaveralError::other(e.to_string()))
                } else {
                    // Use fallback info
                    Ok(PluginInfo {
                        name: fallback_name.to_string(),
                        version: "unknown".to_string(),
                        plugin_type,
                        description: None,
                        author: None,
                        capabilities: Vec::new(),
                    })
                }
            }
            _ => {
                // Use fallback info if plugin doesn't support info action
                Ok(PluginInfo {
                    name: fallback_name.to_string(),
                    version: "unknown".to_string(),
                    plugin_type,
                    description: None,
                    author: None,
                    capabilities: Vec::new(),
                })
            }
        }
    }

    /// Discover plugins in search paths
    pub fn discover(&mut self) -> Result<Vec<PluginInfo>> {
        let mut discovered = Vec::new();

        for search_path in &self.search_paths.clone() {
            if !search_path.exists() {
                continue;
            }

            if search_path.is_dir() {
                for entry in std::fs::read_dir(search_path)
                    .map_err(|e| CanaveralError::other(e.to_string()))?
                {
                    let entry = entry.map_err(|e| CanaveralError::other(e.to_string()))?;
                    let path = entry.path();

                    if self.is_plugin_file(&path) {
                        if let Ok(info) = self.query_plugin_info(
                            &path.to_string_lossy(),
                            path.file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default()
                                .as_str(),
                            PluginType::Adapter, // Default type
                        ) {
                            discovered.push(info);
                        }
                    }
                }
            }
        }

        Ok(discovered)
    }

    /// Check if a path looks like a plugin file
    fn is_plugin_file(&self, path: &Path) -> bool {
        if !path.is_file() {
            return false;
        }

        let extension = path.extension().and_then(|e| e.to_str());

        // Check for executable plugins
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = path.metadata() {
                if metadata.permissions().mode() & 0o111 != 0 {
                    return true;
                }
            }
        }

        // Check for known extensions
        matches!(extension, Some("exe") | Some("so") | Some("dylib") | Some("dll"))
    }
}

/// Adapter plugin interface
pub trait AdapterPlugin {
    /// Get adapter name
    fn name(&self) -> &str;

    /// Detect if this adapter applies to a path
    fn detect(&self, path: &Path) -> bool;

    /// Get package version
    fn get_version(&self, path: &Path) -> Result<String>;

    /// Set package version
    fn set_version(&self, path: &Path, version: &str) -> Result<()>;

    /// Publish package
    fn publish(&self, path: &Path, options: &serde_json::Value) -> Result<()>;
}

/// Strategy plugin interface
pub trait StrategyPlugin {
    /// Get strategy name
    fn name(&self) -> &str;

    /// Parse a version string
    fn parse(&self, version: &str) -> Result<serde_json::Value>;

    /// Format version components
    fn format(&self, components: &serde_json::Value) -> Result<String>;

    /// Calculate next version
    fn bump(&self, current: &str, bump_type: &str) -> Result<String>;
}

/// External adapter that wraps an ExternalPlugin
pub struct ExternalAdapter {
    plugin: ExternalPlugin,
}

impl ExternalAdapter {
    /// Create a new external adapter
    pub fn new(plugin: ExternalPlugin) -> Self {
        Self { plugin }
    }

    /// Get the plugin info
    pub fn info(&self) -> &PluginInfo {
        self.plugin.info()
    }
}

impl AdapterPlugin for ExternalAdapter {
    fn name(&self) -> &str {
        &self.plugin.info.name
    }

    fn detect(&self, path: &Path) -> bool {
        let input = serde_json::json!({
            "path": path.to_string_lossy()
        });

        self.plugin
            .execute("detect", &input)
            .ok()
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }

    fn get_version(&self, path: &Path) -> Result<String> {
        let input = serde_json::json!({
            "path": path.to_string_lossy()
        });

        let output = self.plugin.execute("get_version", &input)?;

        output
            .as_str()
            .map(String::from)
            .ok_or_else(|| CanaveralError::other("Invalid version response"))
    }

    fn set_version(&self, path: &Path, version: &str) -> Result<()> {
        let input = serde_json::json!({
            "path": path.to_string_lossy(),
            "version": version
        });

        self.plugin.execute("set_version", &input)?;
        Ok(())
    }

    fn publish(&self, path: &Path, options: &serde_json::Value) -> Result<()> {
        let input = serde_json::json!({
            "path": path.to_string_lossy(),
            "options": options
        });

        self.plugin.execute("publish", &input)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plugin_type() {
        assert_eq!(PluginType::Adapter.as_str(), "adapter");
        assert_eq!(PluginType::Strategy.as_str(), "strategy");
        assert_eq!(PluginType::Formatter.as_str(), "formatter");
    }

    #[test]
    fn test_plugin_info() {
        let info = PluginInfo {
            name: "test".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: PluginType::Adapter,
            description: Some("Test plugin".to_string()),
            author: Some("Test Author".to_string()),
            capabilities: vec!["publish".to_string()],
        };

        assert_eq!(info.name, "test");
        assert_eq!(info.plugin_type, PluginType::Adapter);
    }

    #[test]
    fn test_plugin_registry() {
        let mut registry = PluginRegistry::new();

        let info = PluginInfo {
            name: "test-adapter".to_string(),
            version: "1.0.0".to_string(),
            plugin_type: PluginType::Adapter,
            description: None,
            author: None,
            capabilities: Vec::new(),
        };

        let plugin = ExternalPlugin::new(info, "echo");
        registry.register(plugin);

        assert!(registry.get(PluginType::Adapter, "test-adapter").is_some());
        assert!(registry.get(PluginType::Adapter, "nonexistent").is_none());
    }

    #[test]
    fn test_plugin_request_serialization() {
        let request = PluginRequest {
            action: "detect".to_string(),
            input: serde_json::json!({"path": "/test"}),
            config: HashMap::new(),
        };

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("detect"));
        assert!(json.contains("/test"));
    }

    #[test]
    fn test_plugin_response_serialization() {
        let response = PluginResponse {
            output: Some(serde_json::json!({"version": "1.0.0"})),
            error: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("1.0.0"));
    }
}
