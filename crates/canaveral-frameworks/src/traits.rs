//! Core traits for framework adapters
//!
//! All adapters implement these traits to provide a unified interface regardless
//! of the underlying framework (Flutter, Expo, React Native, native, etc.).

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::artifacts::Artifact;
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, ScreenshotContext, TestContext};
use crate::detection::Detection;
use crate::error::Result;

/// Platform targets for builds
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    /// iOS (iPhone, iPad)
    Ios,
    /// Android
    Android,
    /// macOS desktop
    MacOs,
    /// Windows desktop
    Windows,
    /// Linux desktop
    Linux,
    /// Web (browser)
    Web,
}

impl Platform {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ios => "ios",
            Self::Android => "android",
            Self::MacOs => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
            Self::Web => "web",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "ios" => Some(Self::Ios),
            "android" => Some(Self::Android),
            "macos" | "mac" => Some(Self::MacOs),
            "windows" | "win" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            "web" => Some(Self::Web),
            _ => None,
        }
    }
}

/// Build adapter trait - handles compiling/bundling for any framework
#[async_trait]
pub trait BuildAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g., "flutter", "expo", "native-ios")
    fn id(&self) -> &'static str;

    /// Human-readable name (e.g., "Flutter", "Expo (React Native)", "Native iOS")
    fn name(&self) -> &'static str;

    /// Detect if this adapter applies to the project at the given path
    /// Returns a confidence score (0-100) or None if not applicable
    fn detect(&self, path: &Path) -> Detection;

    /// Get capabilities of this adapter
    fn capabilities(&self) -> Capabilities;

    /// Get supported platforms
    fn supported_platforms(&self) -> &[Platform];

    /// Check if required tools are installed
    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus>;

    /// Build the project for the specified platform
    /// Returns the path(s) to the built artifact(s)
    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>>;

    /// Clean build artifacts
    async fn clean(&self, path: &Path) -> Result<()>;

    /// Get the version from project files
    fn get_version(&self, path: &Path) -> Result<VersionInfo>;

    /// Set the version in project files
    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()>;
}

/// Test adapter trait - handles running tests for any framework
#[async_trait]
pub trait TestAdapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Detect if this adapter applies to the project
    fn detect(&self, path: &Path) -> Detection;

    /// Get capabilities
    fn capabilities(&self) -> Capabilities;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus>;

    /// Run tests
    async fn test(&self, ctx: &TestContext) -> Result<TestReport>;
}

/// Screenshot adapter trait - handles capturing screenshots for any framework
#[async_trait]
pub trait ScreenshotAdapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Detect if this adapter applies to the project
    fn detect(&self, path: &Path) -> Detection;

    /// Get capabilities
    fn capabilities(&self) -> Capabilities;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus>;

    /// Capture screenshots
    async fn capture(&self, ctx: &ScreenshotContext) -> Result<Vec<Screenshot>>;
}

/// Distribution adapter trait - handles beta distribution
#[async_trait]
pub trait DistributeAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g., "testflight", "firebase", "appcenter")
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus>;

    /// Upload and distribute an artifact
    async fn distribute(&self, ctx: &DistributeContext) -> Result<DistributeResult>;

    /// Get distribution status
    async fn status(&self, ctx: &DistributeContext) -> Result<DistributeStatus>;
}

/// OTA (Over-the-Air) update adapter trait
#[async_trait]
pub trait OtaAdapter: Send + Sync {
    /// Unique identifier for this adapter (e.g., "expo-updates", "codepush", "shorebird")
    fn id(&self) -> &'static str;

    /// Human-readable name
    fn name(&self) -> &'static str;

    /// Detect if this adapter applies to the project
    fn detect(&self, path: &Path) -> Detection;

    /// Check prerequisites
    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus>;

    /// Publish an OTA update
    async fn publish(&self, ctx: &OtaContext) -> Result<OtaResult>;

    /// Rollback to a previous version
    async fn rollback(&self, ctx: &OtaContext, target: &str) -> Result<OtaResult>;
}

/// Version adapter trait - handles version management for any project type
pub trait VersionAdapter: Send + Sync {
    /// Unique identifier for this adapter
    fn id(&self) -> &'static str;

    /// Detect if this adapter applies to the project
    fn detect(&self, path: &Path) -> Detection;

    /// Files that this adapter manages
    fn managed_files(&self) -> &[&str];

    /// Get the current version
    fn get_version(&self, path: &Path) -> Result<VersionInfo>;

    /// Set the version
    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()>;
}

// -----------------------------------------------------------------------------
// Supporting types
// -----------------------------------------------------------------------------

/// Status of prerequisites check
#[derive(Debug, Clone)]
pub struct PrerequisiteStatus {
    pub satisfied: bool,
    pub tools: Vec<ToolStatus>,
    pub warnings: Vec<String>,
}

impl PrerequisiteStatus {
    pub fn ok() -> Self {
        Self {
            satisfied: true,
            tools: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn with_tool(mut self, tool: ToolStatus) -> Self {
        if !tool.available {
            self.satisfied = false;
        }
        self.tools.push(tool);
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

/// Status of a required tool
#[derive(Debug, Clone)]
pub struct ToolStatus {
    pub name: String,
    pub available: bool,
    pub version: Option<String>,
    pub install_hint: String,
}

impl ToolStatus {
    pub fn found(name: impl Into<String>, version: Option<String>) -> Self {
        Self {
            name: name.into(),
            available: true,
            version,
            install_hint: String::new(),
        }
    }

    pub fn missing(name: impl Into<String>, install_hint: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            available: false,
            version: None,
            install_hint: install_hint.into(),
        }
    }
}

/// Version information for a project
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VersionInfo {
    /// Semantic version (e.g., "1.2.3")
    pub version: String,
    /// Build number (e.g., 42) - used by iOS/Android and other platforms
    pub build_number: Option<u64>,
    /// Build name/code (e.g., "1.2.3+42")
    pub build_name: Option<String>,

    /// Platform-specific metadata (key-value pairs)
    /// Examples: npm dist-tags, cargo features, docker image tags
    #[serde(default)]
    pub platform_metadata: std::collections::HashMap<String, String>,
}

impl VersionInfo {
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            version: version.into(),
            ..Default::default()
        }
    }

    pub fn with_build_number(mut self, build_number: u64) -> Self {
        self.build_number = Some(build_number);
        self
    }
}

/// Test execution report
#[derive(Debug, Clone)]
pub struct TestReport {
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub duration_ms: u64,
    pub suites: Vec<TestSuite>,
    pub coverage: Option<CoverageReport>,
}

impl TestReport {
    pub fn success(&self) -> bool {
        self.failed == 0
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }
}

/// A test suite
#[derive(Debug, Clone)]
pub struct TestSuite {
    pub name: String,
    pub tests: Vec<TestCase>,
    pub duration_ms: u64,
}

/// A single test case
#[derive(Debug, Clone)]
pub struct TestCase {
    pub name: String,
    pub status: TestStatus,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Test case status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
}

/// Coverage report
#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub line_coverage: f64,
    pub branch_coverage: Option<f64>,
    pub files: Vec<FileCoverage>,
}

/// Per-file coverage
#[derive(Debug, Clone)]
pub struct FileCoverage {
    pub path: String,
    pub line_coverage: f64,
    pub lines_covered: usize,
    pub lines_total: usize,
}

/// A captured screenshot
#[derive(Debug, Clone)]
pub struct Screenshot {
    pub path: std::path::PathBuf,
    pub device: String,
    pub locale: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
}

/// Context for distribution
#[derive(Debug, Clone)]
pub struct DistributeContext {
    pub artifact: Artifact,
    pub groups: Vec<String>,
    pub notes: Option<String>,
    pub notify: bool,
}

/// Result of distribution
#[derive(Debug, Clone)]
pub struct DistributeResult {
    pub id: String,
    pub url: Option<String>,
    pub groups: Vec<String>,
}

/// Status of a distribution
#[derive(Debug, Clone)]
pub struct DistributeStatus {
    pub id: String,
    pub status: String,
    pub install_count: Option<u64>,
}

/// Context for OTA updates
#[derive(Debug, Clone)]
pub struct OtaContext {
    pub path: std::path::PathBuf,
    pub channel: String,
    pub message: Option<String>,
}

/// Result of OTA operation
#[derive(Debug, Clone)]
pub struct OtaResult {
    pub id: String,
    pub channel: String,
    pub url: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_roundtrip() {
        let platforms = [
            Platform::Ios,
            Platform::Android,
            Platform::MacOs,
            Platform::Windows,
            Platform::Linux,
            Platform::Web,
        ];

        for platform in platforms {
            let s = platform.as_str();
            let parsed = Platform::parse(s);
            assert_eq!(parsed, Some(platform));
        }
    }

    #[test]
    fn test_version_info_builder() {
        let info = VersionInfo::new("1.2.3").with_build_number(42);

        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.build_number, Some(42));
    }

    #[test]
    fn test_test_report_success() {
        let report = TestReport {
            passed: 10,
            failed: 0,
            skipped: 2,
            duration_ms: 1000,
            suites: vec![],
            coverage: None,
        };

        assert!(report.success());
        assert_eq!(report.total(), 12);
    }

    #[test]
    fn test_prerequisite_status() {
        let status = PrerequisiteStatus::ok()
            .with_tool(ToolStatus::found("flutter", Some("3.16.0".to_string())))
            .with_tool(ToolStatus::missing("xcode", "Install from App Store"));

        assert!(!status.satisfied);
        assert_eq!(status.tools.len(), 2);
    }
}
