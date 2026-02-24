//! Cache management command

use std::time::Duration;

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_tasks::TaskCache;

use crate::cli::output::Ui;
use crate::cli::Cli;

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
        let action_name = match &self.action {
            CacheAction::Prune(_) => "prune",
            CacheAction::Status(_) => "status",
            CacheAction::Clean(_) => "clean",
        };
        info!(action = action_name, "executing cache command");
        match &self.action {
            CacheAction::Prune(cmd) => cmd.execute(cli),
            CacheAction::Status(cmd) => cmd.execute(cli),
            CacheAction::Clean(cmd) => cmd.execute(cli),
        }
    }
}

impl CachePruneCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);
        let max_age = Duration::from_secs(self.max_age_days * 24 * 60 * 60);

        ui.info(&format!(
            "Pruning cache entries older than {} days...",
            self.max_age_days
        ));

        let stats = cache.prune(max_age)?;

        if ui.is_json() {
            let result = serde_json::json!({
                "total": stats.total,
                "removed": stats.removed,
                "kept": stats.kept,
            });
            ui.json(&result)?;
        } else {
            ui.success(&format!(
                "Removed {} of {} entries ({} kept)",
                stats.removed, stats.total, stats.kept
            ));
        }

        Ok(())
    }
}

impl CacheStatusCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);

        let stats = cache.status()?;

        if ui.is_json() {
            let result = serde_json::json!({
                "entries": stats.entries,
                "total_size": stats.total_size,
                "total_size_formatted": stats.formatted_size(),
                "cache_dir": cache.cache_dir().display().to_string(),
            });
            ui.json(&result)?;
        } else {
            ui.header("Task Cache Status");
            ui.blank();
            ui.key_value(
                "Location",
                &style(cache.cache_dir().display()).cyan().to_string(),
            );
            ui.key_value("Entries", &stats.entries.to_string());
            ui.key_value("Size", &style(stats.formatted_size()).yellow().to_string());
        }

        Ok(())
    }
}

impl CacheCleanCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let ui = Ui::new(cli);
        let cwd = std::env::current_dir()?;
        let cache = TaskCache::default_dir(&cwd);
        let cache_dir = cache.cache_dir().to_path_buf();

        if !cache_dir.exists() {
            ui.success("Cache directory does not exist.");
            return Ok(());
        }

        if !self.yes {
            let confirmed = ui.confirm(
                &format!("Remove all cached entries at {}?", cache_dir.display()),
                false,
            )?;

            if !confirmed {
                ui.warning("Aborted.");
                return Ok(());
            }
        }

        std::fs::remove_dir_all(&cache_dir)?;

        ui.success(&format!(
            "Cache cleared at {}",
            style(cache_dir.display()).cyan()
        ));

        Ok(())
    }
}
