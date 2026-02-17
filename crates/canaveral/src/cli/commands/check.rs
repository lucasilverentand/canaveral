//! Check command - Run fmt check + lint + test in sequence

use std::path::PathBuf;

use clap::Args;
use console::style;
use tracing::info;

use canaveral_adapters::AdapterRegistry;

use crate::cli::Cli;

/// Run all checks: format verification, linting, and tests
#[derive(Debug, Args)]
pub struct CheckCommand {
    /// Path to the project (defaults to current directory)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Only check affected packages in monorepo
    #[arg(long)]
    pub affected: bool,

    /// Base ref for affected detection (default: main)
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Steps to skip (fmt, lint, test)
    #[arg(long = "skip", value_delimiter = ',')]
    pub skip: Vec<String>,
}

impl CheckCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(affected = self.affected, skip = ?self.skip, "executing check command");

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
            println!();
            println!(
                "{} Running checks ({})...",
                style("→").cyan(),
                style(adapter.name()).bold()
            );
            println!();
        }

        let skip_fmt = self.skip.iter().any(|s| s == "fmt");
        let skip_lint = self.skip.iter().any(|s| s == "lint");
        let skip_test = self.skip.iter().any(|s| s == "test");

        // Step 1: Format check
        if !skip_fmt {
            if !cli.quiet {
                println!("  {} Checking formatting...", style("1/3").dim());
            }
            adapter.fmt(&path, true)?;
            if !cli.quiet {
                println!("       {} Formatting OK", style("✓").green());
            }
        }

        // Step 2: Lint
        if !skip_lint {
            if !cli.quiet {
                let step = if skip_fmt { "1" } else { "2" };
                println!("  {} Running linter...", style(format!("{}/3", step)).dim());
            }
            adapter.lint(&path)?;
            if !cli.quiet {
                println!("       {} Lint OK", style("✓").green());
            }
        }

        // Step 3: Test
        if !skip_test {
            if !cli.quiet {
                let step = if skip_fmt && skip_lint {
                    "1"
                } else if skip_fmt || skip_lint {
                    "2"
                } else {
                    "3"
                };
                println!("  {} Running tests...", style(format!("{}/3", step)).dim());
            }
            adapter.test(&path)?;
            if !cli.quiet {
                println!("       {} Tests OK", style("✓").green());
            }
        }

        if !cli.quiet {
            println!();
            println!("{} All checks passed!", style("✓").green().bold());
            println!();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_command_defaults() {
        let cmd = CheckCommand {
            path: PathBuf::from("."),
            affected: false,
            base: "main".to_string(),
            skip: vec![],
        };
        assert!(cmd.skip.is_empty());
    }

    #[test]
    fn test_skip_parsing() {
        let skip = ["fmt".to_string(), "test".to_string()];
        assert!(skip.iter().any(|s| s == "fmt"));
        assert!(!skip.iter().any(|s| s == "lint"));
        assert!(skip.iter().any(|s| s == "test"));
    }
}
