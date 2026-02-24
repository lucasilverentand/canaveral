//! Tool version pinning configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Tool version specification — either a simple version string or detailed config
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolVersionSpec {
    /// Simple version string, e.g. `"1.2"`
    Version(String),
    /// Detailed config with optional install method
    Detailed(DetailedToolSpec),
}

/// Detailed tool specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetailedToolSpec {
    /// Tool version
    pub version: String,

    /// How to install the tool (e.g. "mise", "brew", "cargo")
    #[serde(default)]
    pub install_method: Option<String>,

    /// Explicit aqua registry source as "owner/repo" (e.g. "BurntSushi/ripgrep")
    #[serde(default)]
    pub source: Option<String>,
}

fn default_tools_cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".canaveral/tools")
}

fn default_max_age_days() -> u64 {
    30
}

/// Cache configuration for installed tools
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolsCacheConfig {
    /// Cache directory (default: ~/.canaveral/tools)
    #[serde(default = "default_tools_cache_dir")]
    pub dir: PathBuf,
    /// Auto-prune versions not used in this many days (default: 30)
    #[serde(default = "default_max_age_days")]
    pub max_age_days: u64,
    /// Optional max cache size (e.g. "10GB", "500MB")
    #[serde(default)]
    pub max_size: Option<String>,
}

impl Default for ToolsCacheConfig {
    fn default() -> Self {
        Self {
            dir: default_tools_cache_dir(),
            max_age_days: default_max_age_days(),
            max_size: None,
        }
    }
}

/// Tool version pinning configuration
///
/// ```toml
/// [tools]
/// bun = "1.2"
/// node = "22"
/// rust = { version = "1.75", install_method = "rustup" }
///
/// [tools.cache]
/// dir = "~/.canaveral/tools"
/// max_age_days = 30
/// max_size = "10GB"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ToolsConfig {
    #[serde(default)]
    pub cache: ToolsCacheConfig,
    #[serde(flatten)]
    pub tools: HashMap<String, ToolVersionSpec>,
}
