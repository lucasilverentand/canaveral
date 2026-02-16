//! Run command — execute tasks across the workspace

use std::collections::HashMap;
use std::sync::Arc;

use clap::Args;
use console::style;
use anyhow::Context;

use canaveral_core::config::load_config_or_default;
use canaveral_core::monorepo::{
    ChangeDetector, DependencyGraph, PackageDiscovery, Workspace,
};
use canaveral_tasks::{
    TaskCache, TaskDag, TaskDefinition, TaskEvent, TaskReporter, TaskScheduler,
};
use canaveral_tasks::scheduler::SchedulerOptions;

use crate::cli::{Cli, OutputFormat};

/// Run tasks across the workspace
#[derive(Debug, Args)]
pub struct RunCommand {
    /// Tasks to run (e.g., build test lint)
    #[arg(required = true)]
    pub tasks: Vec<String>,

    /// Only run on packages affected by changes
    #[arg(long)]
    pub affected: bool,

    /// Base ref for affected detection (default: latest tag or main)
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Filter to specific packages (can be repeated)
    #[arg(long)]
    pub filter: Vec<String>,

    /// Maximum concurrent tasks
    #[arg(long)]
    pub concurrency: Option<usize>,

    /// Show execution plan without running
    #[arg(long)]
    pub dry_run: bool,

    /// Continue running other tasks when one fails
    #[arg(long)]
    pub continue_on_error: bool,

    /// Disable task cache
    #[arg(long)]
    pub no_cache: bool,
}

impl RunCommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(self.execute_async(cli))
    }

    async fn execute_async(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        // Discover workspace
        let workspace = Workspace::detect(&cwd)?
            .context("No workspace found in current directory")?;
        let discovery = PackageDiscovery::new(workspace);
        let discovered = discovery.discover()?;

        if discovered.is_empty() {
            anyhow::bail!("No packages found in workspace");
        }

        // Build dependency graph
        let graph = DependencyGraph::build(&discovered)?;
        graph.validate()?;

        // Determine which packages to include
        let mut packages: Vec<String> = if !self.filter.is_empty() {
            self.filter.clone()
        } else if self.affected {
            let detector = ChangeDetector::new(cwd.clone());
            let changed_files = detector.get_changed_files_git(Some(&self.base), "HEAD")?;
            let changed = detector.detect_changes(&discovered, &changed_files, Some(&graph))?;

            if changed.is_empty() {
                if !cli.quiet {
                    println!(
                        "{} No affected packages found since {}",
                        style("✓").green(),
                        style(&self.base).cyan()
                    );
                }
                return Ok(());
            }

            changed.iter().map(|c| c.name.clone()).collect()
        } else {
            graph.sorted().to_vec()
        };

        // Sort packages in topological order
        let sorted = graph.sorted();
        packages.sort_by_key(|p| sorted.iter().position(|s| s == p).unwrap_or(usize::MAX));

        // Build pipeline from config or defaults
        let pipeline = build_pipeline(&config.tasks.pipeline, &self.tasks);

        // Build the task DAG
        let dag = TaskDag::build(&graph, &pipeline, &self.tasks, &packages)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        if dag.is_empty() {
            if !cli.quiet {
                println!("{} No tasks to run.", style("✓").green());
            }
            return Ok(());
        }

        // Show plan
        if !cli.quiet && cli.format == OutputFormat::Text {
            println!();
            println!(
                "{} {} task{} across {} package{}",
                style("→").blue(),
                dag.len(),
                if dag.len() == 1 { "" } else { "s" },
                packages.len(),
                if packages.len() == 1 { "" } else { "s" },
            );

            if cli.verbose || self.dry_run {
                println!();
                println!("{}", dag.execution_plan());
            }

            if self.dry_run {
                println!("{}", style("[DRY RUN - no tasks will be executed]").yellow().bold());
                return Ok(());
            }

            println!();
        }

        if self.dry_run && cli.format == OutputFormat::Json {
            let plan: Vec<serde_json::Value> = dag
                .waves()
                .iter()
                .enumerate()
                .map(|(i, wave)| {
                    serde_json::json!({
                        "wave": i,
                        "tasks": wave.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                    })
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&plan)?);
            return Ok(());
        }

        // Set up cache
        let cache = if !self.no_cache && config.tasks.cache.enabled {
            Some(TaskCache::default_dir(&cwd))
        } else {
            None
        };

        // Set up reporter
        let reporter: Arc<dyn TaskReporter> = if cli.quiet {
            Arc::new(canaveral_tasks::reporter::TracingReporter)
        } else {
            Arc::new(ConsoleReporter::new(cli.verbose))
        };

        // Configure scheduler
        let concurrency = self.concurrency.unwrap_or(config.tasks.concurrency);
        let options = SchedulerOptions {
            concurrency,
            continue_on_error: self.continue_on_error,
            use_cache: !self.no_cache && config.tasks.cache.enabled,
            dry_run: false,
            root_dir: cwd,
        };

        let scheduler = TaskScheduler::new(options, cache, reporter);
        let results = scheduler.execute(&dag).await;

        // Report results
        let succeeded = results.iter().filter(|r| r.status.is_success()).count();
        let failed: Vec<_> = results
            .iter()
            .filter(|r| matches!(r.status, canaveral_tasks::TaskStatus::Failed(_)))
            .collect();
        let cached = results
            .iter()
            .filter(|r| matches!(r.status, canaveral_tasks::TaskStatus::CacheHit))
            .count();

        if cli.format == OutputFormat::Json {
            let summary = serde_json::json!({
                "total": results.len(),
                "succeeded": succeeded,
                "failed": failed.len(),
                "cached": cached,
                "tasks": results.iter().map(|r| {
                    serde_json::json!({
                        "id": r.id.to_string(),
                        "status": format!("{:?}", r.status),
                        "duration_ms": r.duration.as_millis(),
                    })
                }).collect::<Vec<_>>(),
            });
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }

        if !failed.is_empty() {
            if !cli.quiet && cli.format == OutputFormat::Text {
                println!();
                println!(
                    "  {} {}/{} tasks failed:",
                    style("✗").red().bold(),
                    failed.len(),
                    results.len()
                );
                for r in &failed {
                    if let canaveral_tasks::TaskStatus::Failed(ref err) = r.status {
                        println!("    {} {}: {}", style("✗").red(), r.id, err);
                    }
                }
            }
            anyhow::bail!(
                "{} task{} failed",
                failed.len(),
                if failed.len() == 1 { "" } else { "s" }
            );
        }

        Ok(())
    }
}

/// Build pipeline task definitions from config
fn build_pipeline(
    config_pipeline: &HashMap<String, canaveral_core::config::PipelineTask>,
    target_tasks: &[String],
) -> HashMap<String, TaskDefinition> {
    let mut pipeline = HashMap::new();

    for task_name in target_tasks {
        if let Some(config_task) = config_pipeline.get(task_name) {
            let mut def = TaskDefinition::new(task_name);
            def.command = config_task.command.clone();
            def.depends_on = config_task.depends_on.clone();
            def.depends_on_packages = config_task.depends_on_packages;
            def.outputs = config_task.outputs.clone();
            def.inputs = config_task.inputs.clone();
            def.env = config_task.env.clone();
            def.persistent = config_task.persistent;
            pipeline.insert(task_name.clone(), def);
        } else {
            // Create default definition for unconfigured tasks
            let def = match task_name.as_str() {
                "build" => TaskDefinition::new("build").with_depends_on_packages(true),
                "test" => TaskDefinition::new("test").with_depends_on("build"),
                "lint" => TaskDefinition::new("lint"),
                _ => TaskDefinition::new(task_name),
            };
            pipeline.insert(task_name.clone(), def);
        }
    }

    pipeline
}

/// Console reporter with live output
struct ConsoleReporter {
    verbose: bool,
}

impl ConsoleReporter {
    fn new(verbose: bool) -> Self {
        Self { verbose }
    }
}

impl TaskReporter for ConsoleReporter {
    fn report(&self, event: &TaskEvent) {
        match event {
            TaskEvent::Started { id, command } => {
                println!(
                    "  {} {} {}",
                    style("▸").dim(),
                    style(id).bold(),
                    if self.verbose {
                        style(format!("({})", command)).dim().to_string()
                    } else {
                        String::new()
                    }
                );
            }
            TaskEvent::Output { id, line, is_stderr } => {
                if self.verbose {
                    if *is_stderr {
                        println!("    {} {}", style(format!("[{}]", id)).red().dim(), line);
                    } else {
                        println!("    {} {}", style(format!("[{}]", id)).dim(), line);
                    }
                }
            }
            TaskEvent::Completed {
                id,
                duration,
                cached,
            } => {
                if *cached {
                    println!(
                        "  {} {} {} {}",
                        style("✓").green(),
                        style(id).green(),
                        style("(cached)").cyan(),
                        style(format!("{:.1}s", duration.as_secs_f64())).dim()
                    );
                } else {
                    println!(
                        "  {} {} {}",
                        style("✓").green(),
                        style(id).green(),
                        style(format!("{:.1}s", duration.as_secs_f64())).dim()
                    );
                }
            }
            TaskEvent::Failed {
                id,
                duration,
                error,
            } => {
                println!(
                    "  {} {} {} {}",
                    style("✗").red(),
                    style(id).red(),
                    style(format!("{:.1}s", duration.as_secs_f64())).dim(),
                    style(error).red().dim()
                );
            }
            TaskEvent::Skipped { id, reason } => {
                println!(
                    "  {} {} {}",
                    style("○").yellow(),
                    style(id).yellow(),
                    style(format!("({})", reason)).dim()
                );
            }
            TaskEvent::WaveStarted { wave, task_count } => {
                if self.verbose {
                    println!(
                        "  {} Wave {} ({} tasks)",
                        style("─").dim(),
                        wave,
                        task_count
                    );
                }
            }
            TaskEvent::AllCompleted {
                total,
                succeeded,
                failed,
                cached,
                duration,
            } => {
                println!();
                println!(
                    "  {} {}/{} succeeded, {} failed, {} cached ({:.1}s)",
                    if *failed == 0 {
                        style("✓").green().bold()
                    } else {
                        style("✗").red().bold()
                    },
                    succeeded,
                    total,
                    failed,
                    cached,
                    duration.as_secs_f64()
                );
            }
        }
    }
}
