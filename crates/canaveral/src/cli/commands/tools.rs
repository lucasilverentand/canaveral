//! Tools command — show and manage pinned tool versions

use clap::{Args, Subcommand};
use console::style;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_core::config::ToolVersionSpec;
use canaveral_tools::{ToolCache, ToolRegistry};

use crate::cli::{Cli, OutputFormat};

/// Manage tool versions (bun, node, etc.)
#[derive(Debug, Args)]
pub struct ToolsCommand {
    #[command(subcommand)]
    pub subcommand: Option<ToolsSubcommand>,
}

#[derive(Debug, Subcommand)]
pub enum ToolsSubcommand {
    /// Show status of all configured tools (default)
    Status,
    /// Install configured tools
    Install {
        /// Specific tool to install (installs all if omitted)
        name: Option<String>,
    },
    /// Output a deterministic SHA-256 hash of the [tools] config (for CI cache keys)
    Hash,
    /// Output colon-separated PATH additions for all cached tool binaries
    ExportPath,
    /// Remove cached tool versions not used within the configured max age
    Prune {
        /// Override the configured max age (days)
        #[arg(long)]
        max_age_days: Option<u64>,
    },
}

// ── JSON output types ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct ToolStatusJson {
    name: String,
    requested: String,
    installed: Option<String>,
    satisfied: bool,
}

#[derive(Debug, Serialize)]
struct ToolInstallJson {
    name: String,
    version: String,
    status: &'static str,
    message: Option<String>,
}

// ── impl ───────────────────────────────────────────────────────────────────────

impl ToolsCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!("executing tools command");
        let rt = tokio::runtime::Runtime::new()?;
        match &self.subcommand {
            None | Some(ToolsSubcommand::Status) => rt.block_on(self.run_status(cli)),
            Some(ToolsSubcommand::Install { name }) => {
                rt.block_on(self.run_install(cli, name.as_deref()))
            }
            Some(ToolsSubcommand::Hash) => self.run_hash(),
            Some(ToolsSubcommand::ExportPath) => self.run_export_path(),
            Some(ToolsSubcommand::Prune { max_age_days }) => self.run_prune(cli, *max_age_days),
        }
    }

    async fn run_status(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let tools_map = flatten_tools(&config);

        if tools_map.is_empty() {
            if !cli.quiet {
                println!("No tools configured. Add a [tools] section to canaveral.toml.");
            }
            return Ok(());
        }

        let registry = ToolRegistry::with_builtins();
        let statuses = registry.check_all(&tools_map).await;

        match cli.format {
            OutputFormat::Json => {
                let json: Vec<ToolStatusJson> = statuses
                    .iter()
                    .map(|s| ToolStatusJson {
                        name: s.name.clone(),
                        requested: s.requested_version.clone().unwrap_or_default(),
                        installed: s.current_version.clone(),
                        satisfied: s.is_satisfied,
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                if !cli.quiet {
                    println!("{}", style("Tools:").bold());
                }
                for s in &statuses {
                    let requested = s.requested_version.as_deref().unwrap_or("?");
                    match (&s.current_version, s.is_satisfied) {
                        (Some(installed), true) => {
                            println!(
                                "  {} {:<10} {}  {}",
                                style("✓").green(),
                                style(&s.name).green(),
                                style(installed).dim(),
                                style(format!("(requested: {requested})")).dim(),
                            );
                        }
                        (Some(installed), false) => {
                            println!(
                                "  {} {:<10} {}  {}  — version mismatch",
                                style("✗").red(),
                                style(&s.name).red(),
                                style(installed).dim(),
                                style(format!("(requested: {requested})")).dim(),
                            );
                        }
                        (None, _) => {
                            println!(
                                "  {} {:<10} {}  {}  — not installed",
                                style("-").dim(),
                                style(&s.name).dim(),
                                style("—").dim(),
                                style(format!("(requested: {requested})")).dim(),
                            );
                        }
                    }
                }

                // Summary line
                if !cli.quiet {
                    let ok = statuses.iter().filter(|s| s.is_satisfied).count();
                    let total = statuses.len();
                    println!();
                    if ok == total {
                        println!(
                            "{} All {total} tool(s) satisfied",
                            style("✓").green().bold()
                        );
                    } else {
                        println!(
                            "{}/{} tool(s) satisfied",
                            style(ok).green(),
                            style(total).bold()
                        );
                    }
                }
            }
        }

        Ok(())
    }

    async fn run_install(&self, cli: &Cli, name: Option<&str>) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let mut tools_map = flatten_tools(&config);

        // Filter to a single tool if requested
        if let Some(n) = name {
            tools_map.retain(|k, _| k == n);
            if tools_map.is_empty() {
                anyhow::bail!("tool '{}' is not configured in [tools]", n);
            }
        }

        if tools_map.is_empty() {
            if !cli.quiet {
                println!("No tools configured. Add a [tools] section to canaveral.toml.");
            }
            return Ok(());
        }

        let registry = ToolRegistry::with_builtins();
        let statuses = registry.check_all(&tools_map).await;

        let mut install_results: Vec<ToolInstallJson> = Vec::new();
        let mut any_error = false;

        for status in &statuses {
            let requested = status.requested_version.as_deref().unwrap_or("?");

            // Skip already-satisfied tools
            if status.is_satisfied {
                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "  {} {} {} — already satisfied",
                        style("✓").green(),
                        style(&status.name).green(),
                        style(status.current_version.as_deref().unwrap_or("")).dim(),
                    );
                }
                install_results.push(ToolInstallJson {
                    name: status.name.clone(),
                    version: status.current_version.clone().unwrap_or_default(),
                    status: "satisfied",
                    message: None,
                });
                continue;
            }

            if let Some(provider) = registry.get(&status.name) {
                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "  {} Installing {} {}...",
                        style("→").cyan(),
                        style(&status.name).bold(),
                        style(requested).dim(),
                    );
                }

                match provider.install(requested).await {
                    Ok(result) => {
                        if !cli.quiet && cli.format == OutputFormat::Text {
                            println!(
                                "  {} Installed {} {}",
                                style("✓").green(),
                                style(&status.name).green(),
                                style(&result.version).dim(),
                            );
                        }
                        install_results.push(ToolInstallJson {
                            name: status.name.clone(),
                            version: result.version,
                            status: "installed",
                            message: None,
                        });
                    }
                    Err(e) => {
                        any_error = true;
                        let msg = e.to_string();
                        if !cli.quiet && cli.format == OutputFormat::Text {
                            println!(
                                "  {} Failed to install {}: {}",
                                style("✗").red(),
                                style(&status.name).red(),
                                msg,
                            );
                        }
                        install_results.push(ToolInstallJson {
                            name: status.name.clone(),
                            version: requested.to_string(),
                            status: "failed",
                            message: Some(msg),
                        });
                    }
                }
            } else {
                any_error = true;
                let msg = format!("no provider registered for '{}'", status.name);
                if !cli.quiet && cli.format == OutputFormat::Text {
                    println!(
                        "  {} {}: {}",
                        style("✗").red(),
                        style(&status.name).red(),
                        msg,
                    );
                }
                install_results.push(ToolInstallJson {
                    name: status.name.clone(),
                    version: requested.to_string(),
                    status: "failed",
                    message: Some(msg),
                });
            }
        }

        if cli.format == OutputFormat::Json {
            println!("{}", serde_json::to_string_pretty(&install_results)?);
        }

        if any_error {
            anyhow::bail!("one or more tools failed to install");
        }

        Ok(())
    }

    fn run_hash(&self) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let tools_map = flatten_tools(&config);

        // Build a canonical sorted string: "name=version\n" per tool
        let mut pairs: Vec<(&String, &String)> = tools_map.iter().collect();
        pairs.sort_by_key(|(name, _)| *name);
        let canonical: String = pairs
            .into_iter()
            .map(|(name, version)| format!("{name}={version}\n"))
            .collect();

        let hash = Sha256::digest(canonical.as_bytes());
        println!("{:x}", hash);
        Ok(())
    }

    fn run_export_path(&self) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let tools_map = flatten_tools(&config);
        let cache = ToolCache::new(&config.tools.cache);

        let mut paths: Vec<String> = Vec::new();
        let mut sorted_tools: Vec<(&String, &String)> = tools_map.iter().collect();
        sorted_tools.sort_by_key(|(name, _)| *name);

        for (name, version) in sorted_tools {
            let bin_dir = cache.version_dir(name, version).join("bin");
            paths.push(bin_dir.to_string_lossy().into_owned());
        }

        println!("{}", paths.join(":"));
        Ok(())
    }

    fn run_prune(&self, cli: &Cli, max_age_days: Option<u64>) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let mut cache_config = config.tools.cache.clone();
        if let Some(days) = max_age_days {
            cache_config.max_age_days = days;
        }

        let cache = ToolCache::new(&cache_config);
        let result = cache.prune()?;

        match cli.format {
            OutputFormat::Json => {
                #[derive(Serialize)]
                struct PruneJson {
                    removed: Vec<String>,
                    freed_bytes: u64,
                }
                let json = PruneJson {
                    removed: result
                        .removed
                        .iter()
                        .map(|(tool, ver)| format!("{tool}@{ver}"))
                        .collect(),
                    freed_bytes: result.freed_bytes,
                };
                println!("{}", serde_json::to_string_pretty(&json)?);
            }
            OutputFormat::Text => {
                if result.removed.is_empty() {
                    if !cli.quiet {
                        println!("Nothing to prune.");
                    }
                } else {
                    for (tool, version) in &result.removed {
                        println!("  {} removed {tool}@{version}", style("-").dim());
                    }
                    if !cli.quiet {
                        println!();
                        println!(
                            "{} Pruned {} version(s), freed {}",
                            style("✓").green().bold(),
                            result.removed.len(),
                            format_bytes(result.freed_bytes),
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

/// Format a byte count as a human-readable string.
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = KB * 1_024;
    const GB: u64 = MB * 1_024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Flatten ToolsConfig into a simple name → version string map.
/// For `Detailed` specs, extracts the version string.
fn flatten_tools(
    config: &canaveral_core::config::Config,
) -> std::collections::HashMap<String, String> {
    config
        .tools
        .tools
        .iter()
        .map(|(name, spec)| {
            let version = match spec {
                ToolVersionSpec::Version(v) => v.clone(),
                ToolVersionSpec::Detailed(d) => d.version.clone(),
            };
            (name.clone(), version)
        })
        .collect()
}
