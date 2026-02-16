//! Flutter test adapter
//!
//! Runs Flutter unit tests, widget tests, and integration tests.

use std::path::Path;
use std::process::Command;
use std::time::Instant;

use async_trait::async_trait;
use tracing::{info, instrument};

use crate::capabilities::Capabilities;
use crate::context::TestContext;
use crate::detection::{file_exists, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{
    CoverageReport, FileCoverage, PrerequisiteStatus, TestAdapter, TestCase, TestReport,
    TestStatus, TestSuite, ToolStatus,
};

/// Flutter test adapter
pub struct FlutterTestAdapter {
    /// Path to flutter executable
    flutter_path: Option<String>,
}

impl FlutterTestAdapter {
    pub fn new() -> Self {
        Self { flutter_path: None }
    }

    pub fn with_flutter_path(path: impl Into<String>) -> Self {
        Self {
            flutter_path: Some(path.into()),
        }
    }

    fn flutter_cmd(&self) -> String {
        self.flutter_path
            .clone()
            .unwrap_or_else(|| "flutter".to_string())
    }

    fn parse_test_output(&self, output: &str, duration_ms: u64) -> TestReport {
        // Flutter test output format:
        // 00:02 +5: test description
        // 00:03 +5 -1: test description (failure)
        // 00:03 +5 -1 ~1: test description (skipped)

        let mut suites: std::collections::HashMap<String, Vec<TestCase>> = std::collections::HashMap::new();
        let mut passed = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for line in output.lines() {
            // Parse test result lines
            if let Some(caps) = parse_flutter_test_line(line) {
                let test_case = TestCase {
                    name: caps.name,
                    status: caps.status,
                    duration_ms: 0, // Individual test durations not easily available
                    error: caps.error,
                };

                match test_case.status {
                    TestStatus::Passed => passed += 1,
                    TestStatus::Failed => failed += 1,
                    TestStatus::Skipped => skipped += 1,
                }

                let suite_name = caps.suite.unwrap_or_else(|| "default".to_string());
                suites.entry(suite_name).or_default().push(test_case);
            }
        }

        // If no tests were parsed from the output, try to get counts from summary line
        if suites.is_empty() {
            // Try to parse summary line like: "All tests passed!" or "Some tests failed."
            for line in output.lines() {
                if line.contains("All tests passed") {
                    // We don't have individual tests, create a placeholder
                    suites.insert("tests".to_string(), vec![TestCase {
                        name: "all_tests".to_string(),
                        status: TestStatus::Passed,
                        duration_ms,
                        error: None,
                    }]);
                    passed = 1;
                } else if line.contains("Some tests failed") || line.contains("FAILED") {
                    suites.insert("tests".to_string(), vec![TestCase {
                        name: "tests".to_string(),
                        status: TestStatus::Failed,
                        duration_ms,
                        error: Some(output.to_string()),
                    }]);
                    failed = 1;
                }
            }
        }

        let test_suites: Vec<TestSuite> = suites
            .into_iter()
            .map(|(name, tests)| {
                let suite_duration: u64 = tests.iter().map(|t| t.duration_ms).sum();
                TestSuite {
                    name,
                    tests,
                    duration_ms: suite_duration,
                }
            })
            .collect();

        TestReport {
            passed,
            failed,
            skipped,
            duration_ms,
            suites: test_suites,
            coverage: None,
        }
    }

    fn parse_coverage(&self, path: &Path) -> Option<CoverageReport> {
        // Coverage is generated in coverage/lcov.info
        let lcov_path = path.join("coverage/lcov.info");
        if !lcov_path.exists() {
            return None;
        }

        let content = std::fs::read_to_string(&lcov_path).ok()?;
        let mut files = Vec::new();
        let mut current_file: Option<String> = None;
        let mut lines_covered = 0usize;
        let mut lines_total = 0usize;

        for line in content.lines() {
            if line.starts_with("SF:") {
                current_file = Some(line[3..].to_string());
                lines_covered = 0;
                lines_total = 0;
            } else if line.starts_with("DA:") {
                lines_total += 1;
                // DA:line_number,hit_count
                if let Some(hit_count) = line.split(',').nth(1) {
                    if hit_count.trim() != "0" {
                        lines_covered += 1;
                    }
                }
            } else if line == "end_of_record" {
                if let Some(file_path) = current_file.take() {
                    let coverage = if lines_total > 0 {
                        lines_covered as f64 / lines_total as f64
                    } else {
                        0.0
                    };
                    files.push(FileCoverage {
                        path: file_path,
                        line_coverage: coverage,
                        lines_covered,
                        lines_total,
                    });
                }
            }
        }

        if files.is_empty() {
            return None;
        }

        let total_covered: usize = files.iter().map(|f| f.lines_covered).sum();
        let total_lines: usize = files.iter().map(|f| f.lines_total).sum();
        let overall_coverage = if total_lines > 0 {
            total_covered as f64 / total_lines as f64
        } else {
            0.0
        };

        Some(CoverageReport {
            line_coverage: overall_coverage,
            branch_coverage: None,
            files,
        })
    }
}

impl Default for FlutterTestAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TestAdapter for FlutterTestAdapter {
    fn id(&self) -> &'static str {
        "flutter"
    }

    fn name(&self) -> &'static str {
        "Flutter"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Must have pubspec.yaml with flutter dependency and test directory
        if !file_exists(path, "pubspec.yaml") {
            return Detection::No;
        }

        let pubspec = path.join("pubspec.yaml");
        if let Ok(content) = std::fs::read_to_string(pubspec) {
            if content.contains("sdk: flutter") || content.contains("flutter:") {
                let has_test_dir = path.join("test").is_dir();
                let has_integration_test = path.join("integration_test").is_dir();

                if has_test_dir || has_integration_test {
                    return Detection::Yes(95);
                } else {
                    return Detection::Maybe(60);
                }
            }
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::flutter()
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        match which::which("flutter") {
            Ok(_) => {
                let version = Command::new("flutter")
                    .args(["--version", "--machine"])
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout).ok()
                        } else {
                            None
                        }
                    })
                    .and_then(|s| {
                        serde_json::from_str::<serde_json::Value>(&s)
                            .ok()
                            .and_then(|v| v["frameworkVersion"].as_str().map(|s| s.to_string()))
                    });

                status = status.with_tool(ToolStatus::found("flutter", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "flutter",
                    "Install from https://flutter.dev/docs/get-started/install",
                ));
            }
        }

        Ok(status)
    }

    #[instrument(skip(self, ctx), fields(framework = "flutter", path = %ctx.path.display()))]
    async fn test(&self, ctx: &TestContext) -> Result<TestReport> {
        info!(coverage = ctx.coverage, "running Flutter tests");
        let start = Instant::now();

        let mut args = vec!["test"];

        // Machine-readable output for parsing
        args.push("--reporter");
        args.push("expanded");

        // Coverage
        if ctx.coverage {
            args.push("--coverage");
        }

        // Test filter
        let filter_string;
        if let Some(ref filter) = ctx.filter {
            args.push("--name");
            filter_string = filter.clone();
            args.push(&filter_string);
        }

        // Timeout
        let timeout_string;
        if let Some(timeout) = ctx.timeout {
            args.push("--timeout");
            timeout_string = format!("{}s", timeout);
            args.push(&timeout_string);
        }

        // Concurrency/jobs
        let jobs_string;
        if let Some(jobs) = ctx.jobs {
            args.push("--concurrency");
            jobs_string = jobs.to_string();
            args.push(&jobs_string);
        }

        if ctx.dry_run {
            return Ok(TestReport {
                passed: 0,
                failed: 0,
                skipped: 0,
                duration_ms: 0,
                suites: vec![],
                coverage: None,
            });
        }

        let output = Command::new(self.flutter_cmd())
            .args(&args)
            .current_dir(&ctx.path)
            .envs(&ctx.env)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("flutter {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let combined = format!("{}\n{}", stdout, stderr);

        let duration_ms = start.elapsed().as_millis() as u64;
        let mut report = self.parse_test_output(&combined, duration_ms);

        // Add coverage if requested
        if ctx.coverage {
            report.coverage = self.parse_coverage(&ctx.path);
        }

        // If tests failed and we don't have details, use stderr
        if !output.status.success() && report.failed == 0 {
            report.failed = 1;
            report.suites.push(TestSuite {
                name: "flutter_test".to_string(),
                tests: vec![TestCase {
                    name: "test_execution".to_string(),
                    status: TestStatus::Failed,
                    duration_ms,
                    error: Some(stderr.to_string()),
                }],
                duration_ms,
            });
        }

        Ok(report)
    }
}

/// Parsed test line result
struct ParsedTestLine {
    name: String,
    status: TestStatus,
    suite: Option<String>,
    error: Option<String>,
}

/// Parse a Flutter test output line
fn parse_flutter_test_line(line: &str) -> Option<ParsedTestLine> {
    // Flutter test output format examples:
    // "00:02 +5: test description"
    // "00:02 +5 -1: test description"
    // "00:02 +5 ~1: test description"

    let line = line.trim();
    if line.is_empty() || !line.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }

    // Skip the timestamp (00:02)
    let after_time = line.split(':').skip(1).collect::<Vec<_>>().join(":");
    if after_time.is_empty() {
        return None;
    }

    // Parse counts and test name
    // Format: " +5: test name" or " +5 -1: test name" or " +5 -1 ~1: test name"
    let parts: Vec<&str> = after_time.splitn(2, ": ").collect();
    if parts.len() < 2 {
        return None;
    }

    let counts = parts[0].trim();
    let test_name = parts[1].trim().to_string();

    // Determine status based on counts
    let status = if counts.contains('-') {
        TestStatus::Failed
    } else if counts.contains('~') {
        TestStatus::Skipped
    } else {
        TestStatus::Passed
    };

    Some(ParsedTestLine {
        name: test_name,
        status,
        suite: None,
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_flutter_project(temp: &TempDir) {
        std::fs::write(
            temp.path().join("pubspec.yaml"),
            r#"
name: test_app
version: 1.0.0

dependencies:
  flutter:
    sdk: flutter

dev_dependencies:
  flutter_test:
    sdk: flutter
"#,
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("test")).unwrap();
    }

    #[test]
    fn test_detection() {
        let adapter = FlutterTestAdapter::new();
        let temp = TempDir::new().unwrap();

        // No detection without project
        assert!(!adapter.detect(temp.path()).detected());

        // Create Flutter project with test dir
        create_flutter_project(&temp);
        let detection = adapter.detect(temp.path());
        assert!(detection.detected());
        assert!(detection.confidence() >= 90);
    }

    #[test]
    fn test_parse_test_line() {
        assert!(parse_flutter_test_line("00:02 +5: my test").is_some());
        assert!(parse_flutter_test_line("00:02 +5 -1: failing test").is_some());

        let passed = parse_flutter_test_line("00:02 +5: my test").unwrap();
        assert_eq!(passed.status, TestStatus::Passed);
        assert_eq!(passed.name, "my test");

        let failed = parse_flutter_test_line("00:02 +5 -1: failing test").unwrap();
        assert_eq!(failed.status, TestStatus::Failed);

        let skipped = parse_flutter_test_line("00:02 +5 ~1: skipped test").unwrap();
        assert_eq!(skipped.status, TestStatus::Skipped);
    }

    #[test]
    fn test_parse_test_output() {
        let adapter = FlutterTestAdapter::new();
        let output = r#"
00:01 +1: widget_test my first test
00:02 +2: widget_test my second test
00:02 +2 -1: widget_test my failing test
"#;

        let report = adapter.parse_test_output(output, 2000);
        assert_eq!(report.passed, 2);
        assert_eq!(report.failed, 1);
    }
}
