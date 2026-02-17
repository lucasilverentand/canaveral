//! Test report generation
//!
//! Generates test reports in various formats: JUnit XML, JSON, plain text.

use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::context::TestReporter;
use crate::error::{FrameworkError, Result};
use crate::traits::{CoverageReport, TestCase, TestReport, TestStatus, TestSuite};

/// Output format for test reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestReportOutput {
    /// Total tests run
    pub total: usize,
    /// Tests passed
    pub passed: usize,
    /// Tests failed
    pub failed: usize,
    /// Tests skipped
    pub skipped: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Test suites
    pub suites: Vec<TestSuiteOutput>,
    /// Coverage info (if collected)
    pub coverage: Option<CoverageOutput>,
    /// Whether all tests passed
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteOutput {
    pub name: String,
    pub tests: usize,
    pub failures: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub cases: Vec<TestCaseOutput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestCaseOutput {
    pub name: String,
    pub status: String,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoverageOutput {
    pub line_coverage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_coverage: Option<f64>,
    pub files: usize,
    pub lines_covered: usize,
    pub lines_total: usize,
}

impl From<&TestReport> for TestReportOutput {
    fn from(report: &TestReport) -> Self {
        Self {
            total: report.total(),
            passed: report.passed,
            failed: report.failed,
            skipped: report.skipped,
            duration_ms: report.duration_ms,
            suites: report.suites.iter().map(|s| s.into()).collect(),
            coverage: report.coverage.as_ref().map(|c| c.into()),
            success: report.success(),
        }
    }
}

impl From<&TestSuite> for TestSuiteOutput {
    fn from(suite: &TestSuite) -> Self {
        let failures = suite
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Failed)
            .count();
        let skipped = suite
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Skipped)
            .count();

        Self {
            name: suite.name.clone(),
            tests: suite.tests.len(),
            failures,
            skipped,
            duration_ms: suite.duration_ms,
            cases: suite.tests.iter().map(|t| t.into()).collect(),
        }
    }
}

impl From<&TestCase> for TestCaseOutput {
    fn from(test: &TestCase) -> Self {
        Self {
            name: test.name.clone(),
            status: match test.status {
                TestStatus::Passed => "passed".to_string(),
                TestStatus::Failed => "failed".to_string(),
                TestStatus::Skipped => "skipped".to_string(),
            },
            duration_ms: test.duration_ms,
            error: test.error.clone(),
        }
    }
}

impl From<&CoverageReport> for CoverageOutput {
    fn from(coverage: &CoverageReport) -> Self {
        let lines_covered: usize = coverage.files.iter().map(|f| f.lines_covered).sum();
        let lines_total: usize = coverage.files.iter().map(|f| f.lines_total).sum();

        Self {
            line_coverage: coverage.line_coverage,
            branch_coverage: coverage.branch_coverage,
            files: coverage.files.len(),
            lines_covered,
            lines_total,
        }
    }
}

/// Report generator for various formats
pub struct ReportGenerator;

impl ReportGenerator {
    /// Generate report in the specified format
    pub fn generate(report: &TestReport, format: TestReporter) -> String {
        match format {
            TestReporter::Pretty => Self::generate_pretty(report),
            TestReporter::Json => Self::generate_json(report),
            TestReporter::Junit => Self::generate_junit(report),
            TestReporter::GithubActions => Self::generate_github_actions(report),
        }
    }

    /// Generate human-readable output
    pub fn generate_pretty(report: &TestReport) -> String {
        let mut output = String::new();

        output.push('\n');
        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str("                         TEST RESULTS\n");
        output.push_str("═══════════════════════════════════════════════════════════════\n\n");

        for suite in &report.suites {
            output.push_str(&format!("  {} ({} tests)\n", suite.name, suite.tests.len()));
            output.push_str("  ─────────────────────────────────────────────────────────────\n");

            for test in &suite.tests {
                let status_icon = match test.status {
                    TestStatus::Passed => "✓",
                    TestStatus::Failed => "✗",
                    TestStatus::Skipped => "○",
                };

                output.push_str(&format!(
                    "    {} {} ({}ms)\n",
                    status_icon, test.name, test.duration_ms
                ));

                if let Some(ref error) = test.error {
                    for line in error.lines() {
                        output.push_str(&format!("        {}\n", line));
                    }
                }
            }
            output.push('\n');
        }

        output.push_str("═══════════════════════════════════════════════════════════════\n");
        output.push_str(&format!(
            "  SUMMARY: {} passed, {} failed, {} skipped ({}ms)\n",
            report.passed, report.failed, report.skipped, report.duration_ms
        ));

        if let Some(ref coverage) = report.coverage {
            output.push_str(&format!(
                "  COVERAGE: {:.1}% lines",
                coverage.line_coverage * 100.0
            ));
            if let Some(branch) = coverage.branch_coverage {
                output.push_str(&format!(", {:.1}% branches", branch * 100.0));
            }
            output.push('\n');
        }

        output.push_str("═══════════════════════════════════════════════════════════════\n");

        if report.success() {
            output.push_str("\n  ✓ All tests passed!\n\n");
        } else {
            output.push_str("\n  ✗ Some tests failed.\n\n");
        }

        output
    }

    /// Generate JSON output
    pub fn generate_json(report: &TestReport) -> String {
        let output: TestReportOutput = report.into();
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
    }

    /// Generate JUnit XML output
    pub fn generate_junit(report: &TestReport) -> String {
        let junit = JUnitReport::from(report);
        junit.to_xml()
    }

    /// Generate GitHub Actions annotations
    pub fn generate_github_actions(report: &TestReport) -> String {
        let mut output = String::new();

        // Summary
        output.push_str(&format!(
            "::group::Test Results ({} passed, {} failed, {} skipped)\n",
            report.passed, report.failed, report.skipped
        ));

        for suite in &report.suites {
            for test in &suite.tests {
                match test.status {
                    TestStatus::Failed => {
                        if let Some(ref error) = test.error {
                            // Try to extract file:line from error
                            let message = error.lines().next().unwrap_or("Test failed");
                            output.push_str(&format!(
                                "::error title={}::{}::{}\n",
                                test.name, suite.name, message
                            ));
                        } else {
                            output.push_str(&format!(
                                "::error title={}::{} failed\n",
                                test.name, test.name
                            ));
                        }
                    }
                    TestStatus::Skipped => {
                        output.push_str(&format!(
                            "::warning title={}::{} skipped\n",
                            test.name, test.name
                        ));
                    }
                    TestStatus::Passed => {}
                }
            }
        }

        output.push_str("::endgroup::\n");

        // Output summary to workflow step summary if available
        if report.failed > 0 {
            output.push_str("::set-output name=test-result::failure\n");
        } else {
            output.push_str("::set-output name=test-result::success\n");
        }

        output
    }

    /// Write report to file
    pub fn write_to_file(report: &TestReport, format: TestReporter, path: &Path) -> Result<()> {
        let content = Self::generate(report, format);
        let mut file = std::fs::File::create(path).map_err(|e| FrameworkError::Context {
            context: "creating report file".to_string(),
            message: e.to_string(),
        })?;

        file.write_all(content.as_bytes())
            .map_err(|e| FrameworkError::Context {
                context: "writing report file".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }
}

/// JUnit XML report structure
#[derive(Debug, Clone)]
pub struct JUnitReport {
    pub name: String,
    pub tests: usize,
    pub failures: usize,
    pub errors: usize,
    pub skipped: usize,
    pub time: f64,
    pub testsuites: Vec<JUnitTestSuite>,
}

#[derive(Debug, Clone)]
pub struct JUnitTestSuite {
    pub name: String,
    pub tests: usize,
    pub failures: usize,
    pub errors: usize,
    pub skipped: usize,
    pub time: f64,
    pub testcases: Vec<JUnitTestCase>,
}

#[derive(Debug, Clone)]
pub struct JUnitTestCase {
    pub name: String,
    pub classname: String,
    pub time: f64,
    pub failure: Option<JUnitFailure>,
    pub skipped: bool,
}

#[derive(Debug, Clone)]
pub struct JUnitFailure {
    pub message: String,
    pub type_name: String,
    pub content: String,
}

impl From<&TestReport> for JUnitReport {
    fn from(report: &TestReport) -> Self {
        let testsuites: Vec<JUnitTestSuite> = report.suites.iter().map(|s| s.into()).collect();

        Self {
            name: "Test Results".to_string(),
            tests: report.total(),
            failures: report.failed,
            errors: 0,
            skipped: report.skipped,
            time: report.duration_ms as f64 / 1000.0,
            testsuites,
        }
    }
}

impl From<&TestSuite> for JUnitTestSuite {
    fn from(suite: &TestSuite) -> Self {
        let failures = suite
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Failed)
            .count();
        let skipped = suite
            .tests
            .iter()
            .filter(|t| t.status == TestStatus::Skipped)
            .count();

        Self {
            name: suite.name.clone(),
            tests: suite.tests.len(),
            failures,
            errors: 0,
            skipped,
            time: suite.duration_ms as f64 / 1000.0,
            testcases: suite
                .tests
                .iter()
                .map(|t| JUnitTestCase::from_test(t, &suite.name))
                .collect(),
        }
    }
}

impl JUnitTestCase {
    fn from_test(test: &TestCase, classname: &str) -> Self {
        Self {
            name: test.name.clone(),
            classname: classname.to_string(),
            time: test.duration_ms as f64 / 1000.0,
            failure: if test.status == TestStatus::Failed {
                Some(JUnitFailure {
                    message: test
                        .error
                        .clone()
                        .unwrap_or_else(|| "Test failed".to_string()),
                    type_name: "AssertionError".to_string(),
                    content: test.error.clone().unwrap_or_default(),
                })
            } else {
                None
            },
            skipped: test.status == TestStatus::Skipped,
        }
    }
}

impl JUnitReport {
    /// Generate XML string
    pub fn to_xml(&self) -> String {
        let mut xml = String::new();

        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str(&format!(
            "<testsuites name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\" time=\"{:.3}\">\n",
            escape_xml(&self.name),
            self.tests,
            self.failures,
            self.errors,
            self.skipped,
            self.time
        ));

        for suite in &self.testsuites {
            xml.push_str(&format!(
                "  <testsuite name=\"{}\" tests=\"{}\" failures=\"{}\" errors=\"{}\" skipped=\"{}\" time=\"{:.3}\">\n",
                escape_xml(&suite.name),
                suite.tests,
                suite.failures,
                suite.errors,
                suite.skipped,
                suite.time
            ));

            for testcase in &suite.testcases {
                if let Some(ref failure) = testcase.failure {
                    xml.push_str(&format!(
                        "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">\n",
                        escape_xml(&testcase.name),
                        escape_xml(&testcase.classname),
                        testcase.time
                    ));
                    xml.push_str(&format!(
                        "      <failure message=\"{}\" type=\"{}\">{}</failure>\n",
                        escape_xml(&failure.message),
                        escape_xml(&failure.type_name),
                        escape_xml(&failure.content)
                    ));
                    xml.push_str("    </testcase>\n");
                } else if testcase.skipped {
                    xml.push_str(&format!(
                        "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\">\n",
                        escape_xml(&testcase.name),
                        escape_xml(&testcase.classname),
                        testcase.time
                    ));
                    xml.push_str("      <skipped/>\n");
                    xml.push_str("    </testcase>\n");
                } else {
                    xml.push_str(&format!(
                        "    <testcase name=\"{}\" classname=\"{}\" time=\"{:.3}\"/>\n",
                        escape_xml(&testcase.name),
                        escape_xml(&testcase.classname),
                        testcase.time
                    ));
                }
            }

            xml.push_str("  </testsuite>\n");
        }

        xml.push_str("</testsuites>\n");

        xml
    }
}

/// Escape special XML characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_report() -> TestReport {
        TestReport {
            passed: 8,
            failed: 2,
            skipped: 1,
            duration_ms: 1234,
            suites: vec![TestSuite {
                name: "unit_tests".to_string(),
                duration_ms: 500,
                tests: vec![
                    TestCase {
                        name: "test_add".to_string(),
                        status: TestStatus::Passed,
                        duration_ms: 10,
                        error: None,
                    },
                    TestCase {
                        name: "test_subtract".to_string(),
                        status: TestStatus::Failed,
                        duration_ms: 15,
                        error: Some("Expected 5, got 3".to_string()),
                    },
                    TestCase {
                        name: "test_pending".to_string(),
                        status: TestStatus::Skipped,
                        duration_ms: 0,
                        error: None,
                    },
                ],
            }],
            coverage: None,
        }
    }

    #[test]
    fn test_json_output() {
        let report = sample_report();
        let json = ReportGenerator::generate_json(&report);

        assert!(json.contains("\"passed\": 8"));
        assert!(json.contains("\"failed\": 2"));
        assert!(json.contains("\"success\": false"));
    }

    #[test]
    fn test_junit_output() {
        let report = sample_report();
        let xml = ReportGenerator::generate_junit(&report);

        assert!(xml.contains("<?xml version=\"1.0\""));
        assert!(xml.contains("tests=\"11\""));
        assert!(xml.contains("failures=\"2\""));
        assert!(xml.contains("<failure"));
        assert!(xml.contains("<skipped/>"));
    }

    #[test]
    fn test_pretty_output() {
        let report = sample_report();
        let pretty = ReportGenerator::generate_pretty(&report);

        assert!(pretty.contains("TEST RESULTS"));
        assert!(pretty.contains("8 passed"));
        assert!(pretty.contains("2 failed"));
        assert!(pretty.contains("1 skipped"));
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("<test>"), "&lt;test&gt;");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("\"quoted\""), "&quot;quoted&quot;");
    }
}
