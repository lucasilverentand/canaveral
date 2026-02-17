//! Test running framework for Canaveral
//!
//! Provides unified test execution across all supported frameworks.
//! Supports unit tests, integration tests, and widget tests (Flutter).

pub mod report;
pub mod runner;

pub use report::{JUnitReport, ReportGenerator, TestReportOutput};
pub use runner::{TestRunner, TestRunnerConfig};

use std::path::Path;

use crate::context::TestContext;
use crate::error::Result;
use crate::registry::FrameworkRegistry;
use crate::traits::{TestAdapter, TestReport};

/// Run tests for a project
///
/// Automatically detects the framework and runs tests accordingly.
pub async fn run_tests(path: &Path, ctx: &TestContext) -> Result<TestReport> {
    let runner = TestRunner::new();
    runner.run(path, ctx).await
}

/// Run tests with a specific adapter
pub async fn run_tests_with_adapter(
    adapter: &dyn TestAdapter,
    ctx: &TestContext,
) -> Result<TestReport> {
    adapter.test(ctx).await
}

/// Detect available test adapters for a project
pub fn detect_test_adapters(path: &Path) -> Vec<&'static str> {
    let registry = FrameworkRegistry::default();
    let mut adapters = Vec::new();

    for adapter in registry.test_adapters() {
        if adapter.detect(path).detected() {
            adapters.push(adapter.id());
        }
    }

    adapters
}
