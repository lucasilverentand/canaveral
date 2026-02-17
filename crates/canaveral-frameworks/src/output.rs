//! Structured output for CI/CD integration
//!
//! Provides consistent output formatting across all operations. In CI mode,
//! outputs machine-readable JSON. In interactive mode, outputs human-friendly text.
//! Also supports GitHub Actions, GitLab CI, and other CI-specific formats.

use std::collections::HashMap;
use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::artifacts::Artifact;
use crate::traits::{TestReport, VersionInfo};

/// Output format
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    /// Human-readable text (default for interactive)
    #[default]
    Text,
    /// JSON (default for CI)
    Json,
    /// GitHub Actions workflow commands
    GithubActions,
    /// GitLab CI variables
    GitlabCi,
}

impl OutputFormat {
    /// Detect format from environment
    pub fn from_env() -> Self {
        if std::env::var("GITHUB_ACTIONS").is_ok() {
            Self::GithubActions
        } else if std::env::var("GITLAB_CI").is_ok() {
            Self::GitlabCi
        } else if std::env::var("CI").is_ok() {
            Self::Json
        } else {
            Self::Text
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "text" | "pretty" => Some(Self::Text),
            "json" => Some(Self::Json),
            "github" | "github-actions" | "gha" => Some(Self::GithubActions),
            "gitlab" | "gitlab-ci" => Some(Self::GitlabCi),
            _ => None,
        }
    }
}

/// Structured output that can be rendered in multiple formats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Output {
    /// Whether the operation succeeded
    pub success: bool,

    /// Primary message
    pub message: String,

    /// Operation that was performed
    pub operation: String,

    /// Duration in milliseconds
    pub duration_ms: Option<u64>,

    /// Artifacts produced
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<ArtifactOutput>,

    /// Version information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<VersionOutput>,

    /// Test results
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tests: Option<TestOutput>,

    /// Warnings
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,

    /// Errors
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,

    /// CI output variables
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub outputs: HashMap<String, String>,

    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Output {
    /// Create a success output
    pub fn success(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: true,
            message: message.into(),
            operation: operation.into(),
            duration_ms: None,
            artifacts: Vec::new(),
            version: None,
            tests: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            outputs: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    /// Create a failure output
    pub fn failure(operation: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            success: false,
            message: message.into(),
            operation: operation.into(),
            duration_ms: None,
            artifacts: Vec::new(),
            version: None,
            tests: None,
            warnings: Vec::new(),
            errors: Vec::new(),
            outputs: HashMap::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn with_duration(mut self, ms: u64) -> Self {
        self.duration_ms = Some(ms);
        self
    }

    pub fn with_artifact(mut self, artifact: Artifact) -> Self {
        self.artifacts.push(ArtifactOutput::from(artifact));
        self
    }

    pub fn with_artifacts(mut self, artifacts: Vec<Artifact>) -> Self {
        self.artifacts
            .extend(artifacts.into_iter().map(ArtifactOutput::from));
        self
    }

    pub fn with_version(mut self, version: VersionInfo) -> Self {
        self.version = Some(VersionOutput::from(version));
        self
    }

    pub fn with_tests(mut self, report: TestReport) -> Self {
        self.tests = Some(TestOutput::from(report));
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }

    pub fn with_error(mut self, error: impl Into<String>) -> Self {
        self.errors.push(error.into());
        self
    }

    pub fn with_output(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.outputs.insert(key.into(), value.into());
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    /// Render output in the specified format
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self.render_text(),
            OutputFormat::Json => self.render_json(),
            OutputFormat::GithubActions => self.render_github_actions(),
            OutputFormat::GitlabCi => self.render_gitlab_ci(),
        }
    }

    /// Print output to stdout
    pub fn print(&self, format: OutputFormat) {
        print!("{}", self.render(format));
    }

    /// Print output to a writer
    pub fn write_to<W: Write>(&self, writer: &mut W, format: OutputFormat) -> std::io::Result<()> {
        write!(writer, "{}", self.render(format))
    }

    fn render_text(&self) -> String {
        let mut out = String::new();

        // Status line
        let status = if self.success { "✓" } else { "✗" };
        out.push_str(&format!("{} {}\n", status, self.message));

        // Duration
        if let Some(ms) = self.duration_ms {
            out.push_str(&format!("  Duration: {}ms\n", ms));
        }

        // Artifacts
        if !self.artifacts.is_empty() {
            out.push_str("\nArtifacts:\n");
            for artifact in &self.artifacts {
                out.push_str(&format!(
                    "  - {} ({}, {})\n",
                    artifact.path,
                    artifact.kind,
                    format_size(artifact.size)
                ));
            }
        }

        // Version
        if let Some(ref version) = self.version {
            out.push_str(&format!("\nVersion: {}", version.version));
            if let Some(bn) = version.build_number {
                out.push_str(&format!(" (build {})", bn));
            }
            out.push('\n');
        }

        // Tests
        if let Some(ref tests) = self.tests {
            out.push_str(&format!(
                "\nTests: {} passed, {} failed, {} skipped\n",
                tests.passed, tests.failed, tests.skipped
            ));
            if let Some(cov) = tests.coverage {
                out.push_str(&format!("Coverage: {:.1}%\n", cov));
            }
        }

        // Warnings
        for warning in &self.warnings {
            out.push_str(&format!("⚠ {}\n", warning));
        }

        // Errors
        for error in &self.errors {
            out.push_str(&format!("✗ {}\n", error));
        }

        out
    }

    fn render_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }

    fn render_github_actions(&self) -> String {
        let mut out = String::new();

        // Set outputs
        for (key, value) in &self.outputs {
            // Escape for multiline values
            let escaped = value
                .replace('%', "%25")
                .replace('\n', "%0A")
                .replace('\r', "%0D");
            out.push_str(&format!("::set-output name={}::{}\n", key, escaped));
        }

        // Also write to GITHUB_OUTPUT if available
        if let Ok(output_file) = std::env::var("GITHUB_OUTPUT") {
            if let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(&output_file) {
                for (key, value) in &self.outputs {
                    if value.contains('\n') {
                        let delimiter = "EOF";
                        let _ = writeln!(file, "{}<<{}", key, delimiter);
                        let _ = writeln!(file, "{}", value);
                        let _ = writeln!(file, "{}", delimiter);
                    } else {
                        let _ = writeln!(file, "{}={}", key, value);
                    }
                }
            }
        }

        // Warnings
        for warning in &self.warnings {
            out.push_str(&format!("::warning::{}\n", warning));
        }

        // Errors
        for error in &self.errors {
            out.push_str(&format!("::error::{}\n", error));
        }

        // Group for artifacts
        if !self.artifacts.is_empty() {
            out.push_str("::group::Artifacts\n");
            for artifact in &self.artifacts {
                out.push_str(&format!(
                    "{} ({}, {})\n",
                    artifact.path,
                    artifact.kind,
                    format_size(artifact.size)
                ));
            }
            out.push_str("::endgroup::\n");
        }

        // Summary
        out.push_str(&format!(
            "{} {}\n",
            if self.success { "✓" } else { "✗" },
            self.message
        ));

        out
    }

    fn render_gitlab_ci(&self) -> String {
        let mut out = String::new();

        // Write to dotenv file for passing variables between jobs
        let dotenv_content: String = self
            .outputs
            .iter()
            .map(|(k, v)| format!("{}={}", k.to_uppercase(), v))
            .collect::<Vec<_>>()
            .join("\n");

        if !dotenv_content.is_empty() {
            out.push_str(&format!("# GitLab CI Variables\n{}\n", dotenv_content));
        }

        // Collapsible section for artifacts
        if !self.artifacts.is_empty() {
            out.push_str(
                "\n\\e[0Ksection_start:`date +%s`:artifacts[collapsed=true]\\r\\e[0KArtifacts\n",
            );
            for artifact in &self.artifacts {
                out.push_str(&format!(
                    "{} ({}, {})\n",
                    artifact.path,
                    artifact.kind,
                    format_size(artifact.size)
                ));
            }
            out.push_str("\\e[0Ksection_end:`date +%s`:artifacts\\r\\e[0K\n");
        }

        // Status
        out.push_str(&format!(
            "\n{} {}\n",
            if self.success { "✓" } else { "✗" },
            self.message
        ));

        out
    }
}

/// Artifact output for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactOutput {
    pub path: String,
    pub kind: String,
    pub platform: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

impl From<Artifact> for ArtifactOutput {
    fn from(a: Artifact) -> Self {
        Self {
            path: a.path.to_string_lossy().to_string(),
            kind: format!("{:?}", a.kind).to_lowercase(),
            platform: a.platform.as_str().to_string(),
            size: a.size,
            sha256: a.sha256,
        }
    }
}

/// Version output for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionOutput {
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_number: Option<u64>,
}

impl From<VersionInfo> for VersionOutput {
    fn from(v: VersionInfo) -> Self {
        Self {
            version: v.version,
            build_number: v.build_number,
        }
    }
}

/// Test output for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestOutput {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub total: usize,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coverage: Option<f64>,
}

impl From<TestReport> for TestOutput {
    fn from(r: TestReport) -> Self {
        Self {
            passed: r.passed,
            failed: r.failed,
            skipped: r.skipped,
            total: r.total(),
            duration_ms: r.duration_ms,
            coverage: r.coverage.map(|c| c.line_coverage),
        }
    }
}

/// Format file size for display
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_success() {
        let output = Output::success("build", "Build completed successfully")
            .with_duration(1234)
            .with_output("artifact_path", "/path/to/app.ipa")
            .with_warning("Using deprecated API");

        assert!(output.success);
        assert_eq!(output.duration_ms, Some(1234));
        assert_eq!(
            output.outputs.get("artifact_path"),
            Some(&"/path/to/app.ipa".to_string())
        );
        assert_eq!(output.warnings.len(), 1);
    }

    #[test]
    fn test_output_failure() {
        let output =
            Output::failure("build", "Build failed").with_error("Missing provisioning profile");

        assert!(!output.success);
        assert_eq!(output.errors.len(), 1);
    }

    #[test]
    fn test_render_json() {
        let output = Output::success("test", "Tests passed").with_output("coverage", "85.5");

        let json = output.render(OutputFormat::Json);
        assert!(json.contains("\"success\": true"));
        assert!(json.contains("\"coverage\": \"85.5\""));
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1500), "1.5 KB");
        assert_eq!(format_size(1_500_000), "1.4 MB");
        assert_eq!(format_size(1_500_000_000), "1.4 GB");
    }

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::parse("json"), Some(OutputFormat::Json));
        assert_eq!(
            OutputFormat::parse("github"),
            Some(OutputFormat::GithubActions)
        );
        assert_eq!(OutputFormat::parse("invalid"), None);
    }
}
