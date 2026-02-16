//! Task scheduler — async executor using tokio

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::Semaphore;

use crate::cache::TaskCache;
use crate::dag::TaskDag;
use crate::reporter::{TaskEvent, TaskReporter};
use crate::task::{TaskCommand, TaskId};

/// Result of a single task execution
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// Task that was executed
    pub id: TaskId,
    /// Whether the task succeeded
    pub status: TaskStatus,
    /// How long the task took
    pub duration: Duration,
    /// Captured stdout
    pub stdout: String,
    /// Captured stderr
    pub stderr: String,
}

/// Task execution status
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task completed successfully
    Success,
    /// Task was a cache hit
    CacheHit,
    /// Task failed
    Failed(String),
    /// Task was skipped
    Skipped,
}

impl TaskStatus {
    /// Check if this status represents success
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success | Self::CacheHit)
    }
}

/// Options for the task scheduler
#[derive(Debug, Clone)]
pub struct SchedulerOptions {
    /// Maximum concurrent tasks
    pub concurrency: usize,
    /// Whether to continue on error
    pub continue_on_error: bool,
    /// Whether to use cache
    pub use_cache: bool,
    /// Whether this is a dry run
    pub dry_run: bool,
    /// Working directory root
    pub root_dir: std::path::PathBuf,
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self {
            concurrency: num_cpus(),
            continue_on_error: false,
            use_cache: true,
            dry_run: false,
            root_dir: std::env::current_dir().unwrap_or_default(),
        }
    }
}

fn num_cpus() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

/// Task scheduler — executes a DAG of tasks with parallelism
pub struct TaskScheduler {
    options: SchedulerOptions,
    cache: Option<TaskCache>,
    reporter: Arc<dyn TaskReporter>,
}

impl TaskScheduler {
    /// Create a new scheduler
    pub fn new(
        options: SchedulerOptions,
        cache: Option<TaskCache>,
        reporter: Arc<dyn TaskReporter>,
    ) -> Self {
        Self {
            options,
            cache,
            reporter,
        }
    }

    /// Execute all tasks in the DAG
    pub async fn execute(&self, dag: &TaskDag) -> Vec<TaskResult> {
        let start = Instant::now();
        let semaphore = Arc::new(Semaphore::new(self.options.concurrency));
        let mut all_results: HashMap<TaskId, TaskResult> = HashMap::new();
        let mut failed = false;

        for (wave_idx, wave) in dag.waves().iter().enumerate() {
            if failed && !self.options.continue_on_error {
                // Mark remaining tasks as skipped
                for id in wave {
                    all_results.insert(
                        id.clone(),
                        TaskResult {
                            id: id.clone(),
                            status: TaskStatus::Skipped,
                            duration: Duration::ZERO,
                            stdout: String::new(),
                            stderr: String::new(),
                        },
                    );
                }
                continue;
            }

            self.reporter.report(&TaskEvent::WaveStarted {
                wave: wave_idx,
                task_count: wave.len(),
            });

            let mut handles = Vec::new();

            for task_id in wave {
                let node = match dag.get(task_id) {
                    Some(n) => n,
                    None => continue,
                };

                let permit = semaphore.clone().acquire_owned().await.unwrap();
                let id = task_id.clone();
                let definition = node.definition.clone();
                let root_dir = self.options.root_dir.clone();
                let dry_run = self.options.dry_run;
                let reporter = self.reporter.clone();
                let use_cache = self.options.use_cache;
                let cache = self.cache.clone();

                let handle = tokio::spawn(async move {
                    let result = execute_task(
                        &id,
                        &definition,
                        &root_dir,
                        dry_run,
                        use_cache,
                        cache.as_ref(),
                        &*reporter,
                    )
                    .await;
                    drop(permit);
                    result
                });

                handles.push((task_id.clone(), handle));
            }

            // Collect results from this wave
            for (id, handle) in handles {
                match handle.await {
                    Ok(result) => {
                        if !result.status.is_success() {
                            failed = true;
                        }
                        all_results.insert(id, result);
                    }
                    Err(e) => {
                        failed = true;
                        all_results.insert(
                            id.clone(),
                            TaskResult {
                                id,
                                status: TaskStatus::Failed(format!("Task panicked: {}", e)),
                                duration: Duration::ZERO,
                                stdout: String::new(),
                                stderr: String::new(),
                            },
                        );
                    }
                }
            }
        }

        let total = all_results.len();
        let succeeded = all_results
            .values()
            .filter(|r| r.status.is_success())
            .count();
        let failed_count = all_results
            .values()
            .filter(|r| matches!(r.status, TaskStatus::Failed(_)))
            .count();
        let cached = all_results
            .values()
            .filter(|r| matches!(r.status, TaskStatus::CacheHit))
            .count();

        self.reporter.report(&TaskEvent::AllCompleted {
            total,
            succeeded,
            failed: failed_count,
            cached,
            duration: start.elapsed(),
        });

        // Return results in topological order
        dag.sorted()
            .iter()
            .filter_map(|id| all_results.remove(id))
            .collect()
    }
}

/// Execute a single task
async fn execute_task(
    id: &TaskId,
    definition: &crate::task::TaskDefinition,
    root_dir: &std::path::Path,
    dry_run: bool,
    use_cache: bool,
    cache: Option<&TaskCache>,
    reporter: &dyn TaskReporter,
) -> TaskResult {
    let start = Instant::now();
    let command = definition.effective_command();
    let cmd_str = match &command {
        TaskCommand::Shell(s) => s.clone(),
        TaskCommand::FrameworkAdapter => format!("<framework:{}>", definition.name),
    };

    reporter.report(&TaskEvent::Started {
        id: id.clone(),
        command: cmd_str.clone(),
    });

    // Check cache
    if use_cache && !definition.outputs.is_empty() {
        if let Some(cache) = cache {
            if let Ok(Some(entry)) = cache.lookup(id, definition, root_dir) {
                reporter.report(&TaskEvent::Completed {
                    id: id.clone(),
                    duration: start.elapsed(),
                    cached: true,
                });
                return TaskResult {
                    id: id.clone(),
                    status: TaskStatus::CacheHit,
                    duration: start.elapsed(),
                    stdout: entry.stdout,
                    stderr: entry.stderr,
                };
            }
        }
    }

    // Dry run — don't actually execute
    if dry_run {
        reporter.report(&TaskEvent::Skipped {
            id: id.clone(),
            reason: "dry run".to_string(),
        });
        return TaskResult {
            id: id.clone(),
            status: TaskStatus::Skipped,
            duration: start.elapsed(),
            stdout: String::new(),
            stderr: String::new(),
        };
    }

    // Execute the command
    match command {
        TaskCommand::Shell(ref cmd) => {
            let result = run_shell_command(id, cmd, root_dir, reporter).await;
            let duration = start.elapsed();

            match result {
                Ok((stdout, stderr)) => {
                    // Store in cache
                    if use_cache && !definition.outputs.is_empty() {
                        if let Some(cache) = cache {
                            let _ = cache.store(id, definition, root_dir, &stdout, &stderr);
                        }
                    }
                    reporter.report(&TaskEvent::Completed {
                        id: id.clone(),
                        duration,
                        cached: false,
                    });
                    TaskResult {
                        id: id.clone(),
                        status: TaskStatus::Success,
                        duration,
                        stdout,
                        stderr,
                    }
                }
                Err(e) => {
                    reporter.report(&TaskEvent::Failed {
                        id: id.clone(),
                        duration,
                        error: e.clone(),
                    });
                    TaskResult {
                        id: id.clone(),
                        status: TaskStatus::Failed(e),
                        duration,
                        stdout: String::new(),
                        stderr: String::new(),
                    }
                }
            }
        }
        TaskCommand::FrameworkAdapter => {
            // Framework adapter tasks are not directly executable via scheduler;
            // they need to be resolved to shell commands by the CLI layer
            reporter.report(&TaskEvent::Skipped {
                id: id.clone(),
                reason: "framework adapter not resolved".to_string(),
            });
            TaskResult {
                id: id.clone(),
                status: TaskStatus::Skipped,
                duration: start.elapsed(),
                stdout: String::new(),
                stderr: String::new(),
            }
        }
    }
}

/// Run a shell command and capture output
async fn run_shell_command(
    id: &TaskId,
    cmd: &str,
    root_dir: &std::path::Path,
    reporter: &dyn TaskReporter,
) -> Result<(String, String), String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(root_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn: {}", e))?;

    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();

    let mut stdout_lines = Vec::new();
    let mut stderr_lines = Vec::new();

    // Read stdout
    if let Some(stdout) = stdout_handle {
        let reader = BufReader::new(stdout);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            reporter.report(&TaskEvent::Output {
                id: id.clone(),
                line: line.clone(),
                is_stderr: false,
            });
            stdout_lines.push(line);
        }
    }

    // Read stderr
    if let Some(stderr) = stderr_handle {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            reporter.report(&TaskEvent::Output {
                id: id.clone(),
                line: line.clone(),
                is_stderr: true,
            });
            stderr_lines.push(line);
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait: {}", e))?;

    if status.success() {
        Ok((stdout_lines.join("\n"), stderr_lines.join("\n")))
    } else {
        let code = status.code().unwrap_or(-1);
        Err(format!(
            "Command exited with code {}: {}",
            code,
            stderr_lines.join("\n")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_is_success() {
        assert!(TaskStatus::Success.is_success());
        assert!(TaskStatus::CacheHit.is_success());
        assert!(!TaskStatus::Failed("error".to_string()).is_success());
        assert!(!TaskStatus::Skipped.is_success());
    }

    #[test]
    fn test_scheduler_options_default() {
        let opts = SchedulerOptions::default();
        assert!(opts.concurrency > 0);
        assert!(!opts.continue_on_error);
        assert!(opts.use_cache);
        assert!(!opts.dry_run);
    }

    #[tokio::test]
    async fn test_execute_dry_run() {
        use crate::dag::TaskDag;
        use crate::reporter::CollectingReporter;
        use crate::task::TaskDefinition;
        use canaveral_core::monorepo::discovery::DiscoveredPackage;
        use canaveral_core::monorepo::graph::DependencyGraph;

        let packages = vec![DiscoveredPackage {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            path: "test-pkg".into(),
            manifest_path: "test-pkg/package.json".into(),
            package_type: "npm".to_string(),
            private: false,
            workspace_dependencies: vec![],
        }];

        let graph = DependencyGraph::build(&packages).unwrap();
        let mut pipeline = HashMap::new();
        pipeline.insert(
            "build".to_string(),
            TaskDefinition::new("build").with_command("echo hello"),
        );

        let dag = TaskDag::build(
            &graph,
            &pipeline,
            &["build".to_string()],
            &["test-pkg".to_string()],
        )
        .unwrap();

        let reporter = Arc::new(CollectingReporter::default());
        let opts = SchedulerOptions {
            dry_run: true,
            ..Default::default()
        };

        let scheduler = TaskScheduler::new(opts, None, reporter.clone());
        let results = scheduler.execute(&dag).await;

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, TaskStatus::Skipped);
    }
}
