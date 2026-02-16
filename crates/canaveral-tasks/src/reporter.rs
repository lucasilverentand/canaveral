//! Task execution reporting

use std::sync::Arc;
use std::time::Duration;

use crate::task::TaskId;

/// Events emitted during task execution
#[derive(Debug, Clone)]
pub enum TaskEvent {
    /// A task is starting execution
    Started {
        id: TaskId,
        command: String,
    },
    /// A task produced output
    Output {
        id: TaskId,
        line: String,
        is_stderr: bool,
    },
    /// A task completed successfully
    Completed {
        id: TaskId,
        duration: Duration,
        cached: bool,
    },
    /// A task failed
    Failed {
        id: TaskId,
        duration: Duration,
        error: String,
    },
    /// A task was skipped (e.g., cache hit with replay)
    Skipped {
        id: TaskId,
        reason: String,
    },
    /// An execution wave is starting
    WaveStarted {
        wave: usize,
        task_count: usize,
    },
    /// All tasks completed
    AllCompleted {
        total: usize,
        succeeded: usize,
        failed: usize,
        cached: usize,
        duration: Duration,
    },
}

/// Trait for reporting task execution progress
pub trait TaskReporter: Send + Sync {
    /// Handle a task event
    fn report(&self, event: &TaskEvent);
}

/// Simple reporter that logs to tracing
#[derive(Debug, Default)]
pub struct TracingReporter;

impl TaskReporter for TracingReporter {
    fn report(&self, event: &TaskEvent) {
        match event {
            TaskEvent::Started { id, command } => {
                tracing::info!("Starting {}: {}", id, command);
            }
            TaskEvent::Output { id, line, is_stderr } => {
                if *is_stderr {
                    tracing::warn!("[{}] {}", id, line);
                } else {
                    tracing::debug!("[{}] {}", id, line);
                }
            }
            TaskEvent::Completed { id, duration, cached } => {
                if *cached {
                    tracing::info!("{} completed (cached) in {:.1}s", id, duration.as_secs_f64());
                } else {
                    tracing::info!("{} completed in {:.1}s", id, duration.as_secs_f64());
                }
            }
            TaskEvent::Failed { id, duration, error } => {
                tracing::error!("{} failed after {:.1}s: {}", id, duration.as_secs_f64(), error);
            }
            TaskEvent::Skipped { id, reason } => {
                tracing::info!("{} skipped: {}", id, reason);
            }
            TaskEvent::WaveStarted { wave, task_count } => {
                tracing::info!("Starting wave {} ({} tasks)", wave, task_count);
            }
            TaskEvent::AllCompleted {
                total,
                succeeded,
                failed,
                cached,
                duration,
            } => {
                tracing::info!(
                    "All tasks complete: {}/{} succeeded, {} failed, {} cached ({:.1}s)",
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

/// Reporter that collects events for later inspection (useful for testing)
#[derive(Debug, Default)]
pub struct CollectingReporter {
    events: std::sync::Mutex<Vec<TaskEvent>>,
}

impl CollectingReporter {
    /// Get all collected events
    pub fn events(&self) -> Vec<TaskEvent> {
        self.events.lock().unwrap().clone()
    }
}

impl TaskReporter for CollectingReporter {
    fn report(&self, event: &TaskEvent) {
        self.events.lock().unwrap().push(event.clone());
    }
}

/// Registry of task reporters
pub struct TaskReporterRegistry {
    reporters: Vec<Arc<dyn TaskReporter>>,
}

impl TaskReporterRegistry {
    pub fn new() -> Self {
        Self {
            reporters: vec![Arc::new(TracingReporter)],
        }
    }

    pub fn empty() -> Self {
        Self {
            reporters: Vec::new(),
        }
    }

    pub fn register<R: TaskReporter + 'static>(&mut self, reporter: R) {
        self.reporters.push(Arc::new(reporter));
    }

    pub fn all(&self) -> &[Arc<dyn TaskReporter>] {
        &self.reporters
    }

    /// Broadcast an event to all registered reporters
    pub fn broadcast(&self, event: &TaskEvent) {
        for reporter in &self.reporters {
            reporter.report(event);
        }
    }
}

impl Default for TaskReporterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_collecting_reporter() {
        let reporter = CollectingReporter::default();
        let id = TaskId::new("core", "build");

        reporter.report(&TaskEvent::Started {
            id: id.clone(),
            command: "cargo build".to_string(),
        });
        reporter.report(&TaskEvent::Completed {
            id,
            duration: Duration::from_secs(5),
            cached: false,
        });

        let events = reporter.events();
        assert_eq!(events.len(), 2);
    }

    #[test]
    fn test_tracing_reporter() {
        let reporter = TracingReporter;
        let id = TaskId::new("core", "build");

        // Just verify it doesn't panic
        reporter.report(&TaskEvent::Started {
            id: id.clone(),
            command: "cargo build".to_string(),
        });
        reporter.report(&TaskEvent::Completed {
            id,
            duration: Duration::from_secs(1),
            cached: true,
        });
    }

    #[test]
    fn test_empty_registry() {
        let registry = TaskReporterRegistry::empty();
        assert!(registry.all().is_empty());
    }

    #[test]
    fn test_broadcast() {
        let collecting = Arc::new(CollectingReporter::default());
        let mut registry = TaskReporterRegistry::empty();
        registry.reporters.push(collecting.clone());

        let id = TaskId::new("core", "build");
        registry.broadcast(&TaskEvent::Started {
            id,
            command: "cargo build".to_string(),
        });

        assert_eq!(collecting.events().len(), 1);
    }

    #[test]
    fn test_register() {
        let mut registry = TaskReporterRegistry::empty();
        assert!(registry.all().is_empty());

        registry.register(TracingReporter);
        assert_eq!(registry.all().len(), 1);

        registry.register(CollectingReporter::default());
        assert_eq!(registry.all().len(), 2);
    }
}
