//! Lint command - Run linters using package adapters

use std::path::PathBuf;

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::AdapterRegistry;

use crate::cli::Cli;

/// Run linter on a project
#[derive(Debug, Args)]
pub struct LintCommand {
    /// Path to the project (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Only lint affected packages in monorepo
    #[arg(long)]
    pub affected: bool,

    /// Base ref for affected detection (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,
}

impl LintCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(affected = self.affected, "executing lint command");

        let path = if self.path.is_absolute() {
            self.path.clone()
        } else {
            std::env::current_dir()?.join(&self.path)
        };

        if !path.exists() {
            anyhow::bail!("Path not found: {}", path.display());
        }

        let registry = AdapterRegistry::new();

        let adapter = registry
            .detect(&path)
            .ok_or_else(|| anyhow::anyhow!("No package adapter detected for {}", path.display()))?;

        if !cli.quiet {
            println!(
                "{} Linting code ({})...",
                style("→").cyan(),
                style(adapter.name()).bold()
            );
        }

        adapter.lint(&path)?;

        if !cli.quiet {
            println!("{} Lint passed!", style("✓").green().bold());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lint_command_defaults() {
        let cmd = LintCommand {
            path: PathBuf::from("."),
            affected: false,
            base: "main".to_string(),
        };
        assert!(!cmd.affected);
    }
}
