//! Task types and definitions

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

/// Unique identifier for a task within the workspace
#[derive(Debug, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskId {
    /// Package name
    pub package: String,
    /// Task name (e.g., "build", "test", "lint")
    pub task_name: String,
}

impl TaskId {
    /// Create a new task ID
    pub fn new(package: impl Into<String>, task_name: impl Into<String>) -> Self {
        Self {
            package: package.into(),
            task_name: task_name.into(),
        }
    }

    /// Parse a task ID from "package:task" format
    pub fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(Self::new(parts[0], parts[1]))
        } else {
            None
        }
    }
}

impl fmt::Display for TaskId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.package, self.task_name)
    }
}

/// How a task should be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TaskCommand {
    /// Shell command to execute
    Shell(String),
    /// Use framework adapter (auto-detected)
    FrameworkAdapter,
}

impl Default for TaskCommand {
    fn default() -> Self {
        Self::FrameworkAdapter
    }
}

/// Definition of a task in the pipeline
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskDefinition {
    /// Task name (e.g., "build", "test", "lint")
    pub name: String,

    /// Command to execute
    #[serde(default)]
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

    /// Environment variables to pass
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether this is a persistent/long-running task (e.g., dev server)
    #[serde(default)]
    pub persistent: bool,
}

impl TaskDefinition {
    /// Create a new task definition
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            command: None,
            depends_on: Vec::new(),
            depends_on_packages: false,
            outputs: Vec::new(),
            inputs: Vec::new(),
            env: HashMap::new(),
            persistent: false,
        }
    }

    /// Set the command
    pub fn with_command(mut self, command: impl Into<String>) -> Self {
        self.command = Some(command.into());
        self
    }

    /// Add a same-package dependency
    pub fn with_depends_on(mut self, dep: impl Into<String>) -> Self {
        self.depends_on.push(dep.into());
        self
    }

    /// Set whether this task depends on package dependencies
    pub fn with_depends_on_packages(mut self, depends: bool) -> Self {
        self.depends_on_packages = depends;
        self
    }

    /// Add output globs
    pub fn with_outputs(mut self, outputs: Vec<String>) -> Self {
        self.outputs = outputs;
        self
    }

    /// Get the effective command for this task
    pub fn effective_command(&self) -> TaskCommand {
        match &self.command {
            Some(cmd) => TaskCommand::Shell(cmd.clone()),
            None => TaskCommand::FrameworkAdapter,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_display() {
        let id = TaskId::new("core", "build");
        assert_eq!(id.to_string(), "core:build");
    }

    #[test]
    fn test_task_id_parse() {
        let id = TaskId::parse("core:build").unwrap();
        assert_eq!(id.package, "core");
        assert_eq!(id.task_name, "build");
    }

    #[test]
    fn test_task_id_parse_invalid() {
        assert!(TaskId::parse("nobuild").is_none());
    }

    #[test]
    fn test_task_definition_builder() {
        let def = TaskDefinition::new("build")
            .with_command("cargo build")
            .with_depends_on_packages(true)
            .with_outputs(vec!["target/**".to_string()]);

        assert_eq!(def.name, "build");
        assert_eq!(def.command, Some("cargo build".to_string()));
        assert!(def.depends_on_packages);
        assert_eq!(def.outputs, vec!["target/**"]);
    }

    #[test]
    fn test_effective_command_shell() {
        let def = TaskDefinition::new("lint").with_command("npm run lint");
        match def.effective_command() {
            TaskCommand::Shell(cmd) => assert_eq!(cmd, "npm run lint"),
            _ => panic!("expected shell command"),
        }
    }

    #[test]
    fn test_effective_command_framework() {
        let def = TaskDefinition::new("build");
        assert!(matches!(
            def.effective_command(),
            TaskCommand::FrameworkAdapter
        ));
    }
}
