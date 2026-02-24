//! Task orchestration configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Task orchestration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TasksConfig {
    /// Maximum concurrent tasks
    pub concurrency: usize,

    /// Task pipeline definitions
    #[serde(default)]
    pub pipeline: HashMap<String, PipelineTask>,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            pipeline: HashMap::new(),
            cache: CacheConfig::default(),
        }
    }
}

/// A task in the pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct PipelineTask {
    /// Shell command to execute
    pub command: Option<String>,

    /// Tasks in the same package that must complete first
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Whether the same task must complete in dependency packages first
    #[serde(default)]
    pub depends_on_packages: bool,

    /// Output glob patterns (for caching)
    #[serde(default)]
    pub outputs: Vec<String>,

    /// Input glob patterns (for cache key computation)
    #[serde(default)]
    pub inputs: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether this is a persistent/long-running task
    #[serde(default)]
    pub persistent: bool,
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,

    /// Cache directory
    pub dir: PathBuf,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: PathBuf::from(".canaveral/cache"),
        }
    }
}
