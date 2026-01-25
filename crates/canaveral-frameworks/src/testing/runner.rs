//! Test runner orchestration
//!
//! Handles detecting the framework and running tests with the appropriate adapter.

use std::path::Path;
use std::time::Instant;

use tracing::{debug, info};

use crate::context::TestContext;
use crate::error::{FrameworkError, Result};
use crate::registry::FrameworkRegistry;
use crate::traits::{TestAdapter, TestReport, TestCase, TestStatus, TestSuite};

/// Configuration for the test runner
#[derive(Debug, Clone, Default)]
pub struct TestRunnerConfig {
    /// Specific adapter to use (bypasses auto-detection)
    pub adapter_id: Option<String>,
    /// Fail fast - stop on first failure
    pub fail_fast: bool,
    /// Verbose output
    pub verbose: bool,
    /// Retry failed tests
    pub retry_count: usize,
}

impl TestRunnerConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_adapter(mut self, adapter_id: impl Into<String>) -> Self {
        self.adapter_id = Some(adapter_id.into());
        self
    }

    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    pub fn with_retry(mut self, count: usize) -> Self {
        self.retry_count = count;
        self
    }
}

/// Test runner that orchestrates test execution
pub struct TestRunner {
    config: TestRunnerConfig,
    registry: FrameworkRegistry,
}

impl TestRunner {
    pub fn new() -> Self {
        Self {
            config: TestRunnerConfig::default(),
            registry: FrameworkRegistry::default(),
        }
    }

    pub fn with_config(config: TestRunnerConfig) -> Self {
        Self {
            config,
            registry: FrameworkRegistry::default(),
        }
    }

    /// Run tests for a project
    pub async fn run(&self, path: &Path, ctx: &TestContext) -> Result<TestReport> {
        let start = Instant::now();

        // Find adapter
        let adapter = self.find_adapter(path)?;

        info!("Running tests with {} adapter", adapter.name());
        debug!("Test context: {:?}", ctx);

        // Check prerequisites
        let prereqs = adapter.check_prerequisites().await?;
        if !prereqs.satisfied {
            let missing = prereqs.tools.iter()
                .find(|t| !t.available);

            if let Some(tool) = missing {
                return Err(FrameworkError::ToolNotFound {
                    tool: tool.name.clone(),
                    install_hint: tool.install_hint.clone(),
                });
            }
        }

        // Run tests with retry logic
        let mut report = None;
        let mut attempts = 0;
        let max_attempts = self.config.retry_count + 1;

        while attempts < max_attempts {
            attempts += 1;

            match adapter.test(ctx).await {
                Ok(r) => {
                    report = Some(r);

                    // If no failures, we're done
                    if report.as_ref().map(|r| r.failed == 0).unwrap_or(false) {
                        break;
                    }

                    // If fail fast and there are failures, don't retry
                    if self.config.fail_fast {
                        break;
                    }

                    // If this was the last attempt, break
                    if attempts >= max_attempts {
                        break;
                    }

                    info!("Test failures detected, retrying ({}/{})", attempts, max_attempts);
                }
                Err(e) => {
                    if attempts >= max_attempts {
                        return Err(e);
                    }
                    info!("Test run failed, retrying ({}/{}): {}", attempts, max_attempts, e);
                }
            }
        }

        let mut report = report.ok_or_else(|| FrameworkError::TestFailed {
            summary: "No test results produced".to_string(),
            failed_count: 0,
            total_count: 0,
        })?;

        // Update duration
        report.duration_ms = start.elapsed().as_millis() as u64;

        info!(
            "Tests completed: {} passed, {} failed, {} skipped in {}ms",
            report.passed, report.failed, report.skipped, report.duration_ms
        );

        Ok(report)
    }

    fn find_adapter(&self, path: &Path) -> Result<&dyn TestAdapter> {
        // If specific adapter requested, use that
        if let Some(ref adapter_id) = self.config.adapter_id {
            return self.registry.get_test_adapter(adapter_id)
                .ok_or_else(|| FrameworkError::Context {
                    context: "adapter resolution".to_string(),
                    message: format!("Unknown test adapter: {}", adapter_id),
                });
        }

        // Auto-detect
        let adapters = self.registry.test_adapters();
        let mut best_match: Option<(&dyn TestAdapter, u8)> = None;

        for adapter in adapters {
            let detection = adapter.detect(path);
            if detection.detected() {
                let confidence = detection.confidence();
                if best_match.map(|(_, c)| confidence > c).unwrap_or(true) {
                    best_match = Some((adapter, confidence));
                }
            }
        }

        best_match
            .map(|(adapter, _)| adapter)
            .ok_or_else(|| FrameworkError::NoFrameworkDetected {
                path: path.to_path_buf(),
                supported: self.registry.test_adapter_ids().join(", "),
            })
    }

    /// Run tests for multiple directories/packages (monorepo support)
    pub async fn run_all(&self, paths: &[&Path], ctx: &TestContext) -> Result<TestReport> {
        let start = Instant::now();
        let mut all_suites = Vec::new();
        let mut total_passed = 0;
        let mut total_failed = 0;
        let mut total_skipped = 0;

        for path in paths {
            match self.run(path, ctx).await {
                Ok(report) => {
                    total_passed += report.passed;
                    total_failed += report.failed;
                    total_skipped += report.skipped;
                    all_suites.extend(report.suites);
                }
                Err(e) => {
                    // Create a failed test case for this path
                    let suite = TestSuite {
                        name: path.display().to_string(),
                        tests: vec![TestCase {
                            name: "setup".to_string(),
                            status: TestStatus::Failed,
                            duration_ms: 0,
                            error: Some(e.to_string()),
                        }],
                        duration_ms: 0,
                    };
                    all_suites.push(suite);
                    total_failed += 1;
                }
            }
        }

        Ok(TestReport {
            passed: total_passed,
            failed: total_failed,
            skipped: total_skipped,
            duration_ms: start.elapsed().as_millis() as u64,
            suites: all_suites,
            coverage: None,
        })
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runner_config_builder() {
        let config = TestRunnerConfig::new()
            .with_adapter("flutter")
            .with_fail_fast(true)
            .with_retry(2);

        assert_eq!(config.adapter_id, Some("flutter".to_string()));
        assert!(config.fail_fast);
        assert_eq!(config.retry_count, 2);
    }
}
