//! Cache management command

use std::time::Duration;

use clap::{Args, Subcommand};
use console::style;

use canaveral_tasks::TaskCache;

use crate::cli::{Cli, OutputFormat};

/// Task cache management
#[derive(Debug, Args)]
pub struct CacheCommand {
    #[command(subcommand)]
    pub action: CacheAction,
}

/// Cache subcommands
#[derive(Debug, Subcommand)]
pub enum CacheAction {
    /// Remove old cache entries
    Prune(CachePruneCommand),
    /// Show cache statistics
    Status(CacheStatusCommand),
    /// Clear all cached entries
    Clean(CacheCleanCommand),
}

/// Prune old cache entries
#[derive(Debug, Args)]
pub struct CachePruneCommand {
    /// Maximum age in days (default: 7)
    #[arg(long, default_value = "7")]
    pub max_age_days: u64,

    /// Dry run - show what would be pruned
    #[arg(long)]
    pub dry_run: bool,
}

/// Show cache statistics
#[derive(Debug, Args)]
pub struct CacheStatusCommand;

/// Clear all cached entries
#[derive(Debug, Args)]
pub struct CacheCleanCommand {
    /// Skip confirmation
    #[arg(short = 'y', long)]
    pub yes: bool,
}

impl CacheCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        match &self.action {
            CacheAction::Prune(cmd) => cmd.execute(cli),
            CacheAction::Status(cmd) => cmd.execute(cli),
            CacheAction::Clean(cmd) => cmd.execute(cli),
        }
    }
}

impl CachePruneCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);
        let max_age = Duration::from_secs(self.max_age_days * 24 * 60 * 60);

        if !cli.quiet {
            println!(
                "{} Pruning cache entries older than {} days...",
                style("→").blue(),
                self.max_age_days
            );
        }

        let stats = cache.prune(max_age)?;

        if cli.format == OutputFormat::Json {
            let result = serde_json::json!({
                "total": stats.total,
                "removed": stats.removed,
                "kept": stats.kept,
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else if !cli.quiet {
            println!(
                "  {} Removed {} of {} entries ({} kept)",
                style("✓").green(),
                stats.removed,
                stats.total,
                stats.kept
            );
        }

        Ok(())
    }
}

impl CacheStatusCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);

        let stats = cache.status()?;

        if cli.format == OutputFormat::Json {
            let result = serde_json::json!({
                "entries": stats.entries,
                "total_size": stats.total_size,
                "total_size_formatted": stats.formatted_size(),
                "cache_dir": cache.cache_dir().display().to_string(),
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        } else if !cli.quiet {
            println!("{}", style("Task Cache Status").bold());
            println!();
            println!("  Location: {}", style(cache.cache_dir().display()).cyan());
            println!("  Entries:  {}", stats.entries);
            println!("  Size:     {}", style(stats.formatted_size()).yellow());
        }

        Ok(())
    }
}

impl CacheCleanCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);
        let cache_dir = cache.cache_dir().to_path_buf();

        if !cache_dir.exists() {
            if !cli.quiet {
                println!("{} Cache directory does not exist.", style("✓").green());
            }
            return Ok(());
        }

        if !self.yes {
            let confirmed = dialoguer::Confirm::new()
                .with_prompt(format!(
                    "Remove all cached entries at {}?",
                    cache_dir.display()
                ))
                .default(false)
                .interact()?;

            if !confirmed {
                println!("{}", style("Aborted.").yellow());
                return Ok(());
            }
        }

        std::fs::remove_dir_all(&cache_dir)?;

        if !cli.quiet {
            println!(
                "{} Cache cleared at {}",
                style("✓").green(),
                style(cache_dir.display()).cyan()
            );
        }

        Ok(())
    }
}
