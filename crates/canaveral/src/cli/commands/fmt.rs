//! Fmt command - Format source code using package adapters

use std::path::PathBuf;

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::AdapterRegistry;

use crate::cli::output::Ui;
use crate::cli::Cli;

/// Format source code
#[derive(Debug, Args)]
pub struct FmtCommand {
    /// Path to the project (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Verify formatting without applying changes (for CI / hooks)
    #[arg(long)]
    pub check: bool,

    /// Only format affected packages in monorepo
    #[arg(long)]
    pub affected: bool,

    /// Base ref for affected detection (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,
}

impl FmtCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(
            check = self.check,
            affected = self.affected,
            "executing fmt command"
        );
        let ui = Ui::new(cli);

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

        let mode = if self.check { "Checking" } else { "Formatting" };
        ui.info(&format!(
            "{} code ({})...",
            mode,
            style(adapter.name()).bold()
        ));

        adapter.fmt(&path, self.check)?;

        if self.check {
            ui.success("Formatting check passed!");
        } else {
            ui.success("Code formatted successfully!");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fmt_command_defaults() {
        let cmd = FmtCommand {
            path: PathBuf::from("."),
            check: false,
            affected: false,
            base: "main".to_string(),
        };
        assert!(!cmd.check);
        assert!(!cmd.affected);
    }
}
