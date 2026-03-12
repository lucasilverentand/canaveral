//! Xcodebuild integration — async wrappers for xcodebuild commands
//!
//! This module provides structured types and async execution for xcodebuild
//! operations: build, test, archive, and export. Used by the `NativeIosAdapter`
//! to drive the full iOS build pipeline.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use plist::Value as PlistValue;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument};

use crate::error::{FrameworkError, Result};

// ---------------------------------------------------------------------------
// Configuration types
// ---------------------------------------------------------------------------

/// Options common to all xcodebuild invocations.
#[derive(Debug, Clone)]
pub struct XcodeBuildOptions {
    /// Path to `.xcworkspace` or `.xcodeproj`.
    pub project_path: PathBuf,
    /// Xcode scheme to build.
    pub scheme: String,
    /// Build configuration (Debug / Release / custom).
    pub configuration: BuildConfiguration,
    /// Build destination.
    pub destination: Destination,
    /// Custom derived-data path (uses Xcode default when `None`).
    pub derived_data_path: Option<PathBuf>,
    /// Additional raw arguments forwarded to xcodebuild.
    pub extra_args: Vec<String>,
    /// Extra build settings (`KEY=value` pairs).
    pub build_settings: HashMap<String, String>,
    /// Environment variables to set for the subprocess.
    pub env: HashMap<String, String>,
}

impl XcodeBuildOptions {
    /// Create options with required fields.
    pub fn new(project_path: impl Into<PathBuf>, scheme: impl Into<String>) -> Self {
        Self {
            project_path: project_path.into(),
            scheme: scheme.into(),
            configuration: BuildConfiguration::Debug,
            destination: Destination::GenericIos,
            derived_data_path: None,
            extra_args: Vec::new(),
            build_settings: HashMap::new(),
            env: HashMap::new(),
        }
    }

    pub fn with_configuration(mut self, cfg: BuildConfiguration) -> Self {
        self.configuration = cfg;
        self
    }

    pub fn with_destination(mut self, dest: Destination) -> Self {
        self.destination = dest;
        self
    }

    pub fn with_derived_data_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.derived_data_path = Some(path.into());
        self
    }

    pub fn with_build_setting(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.build_settings.insert(key.into(), value.into());
        self
    }

    pub fn with_extra_arg(mut self, arg: impl Into<String>) -> Self {
        self.extra_args.push(arg.into());
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    // -- internal helpers ---------------------------------------------------

    /// Build the project/workspace flag pair.
    fn project_flag(&self) -> (&str, String) {
        let path_str = self.project_path.to_string_lossy().to_string();
        if path_str.ends_with(".xcworkspace") {
            ("-workspace", path_str)
        } else {
            ("-project", path_str)
        }
    }

    /// Append the common args shared across build/test/archive.
    fn push_common_args(&self, args: &mut Vec<String>) {
        let (flag, path) = self.project_flag();
        args.push(flag.to_string());
        args.push(path);

        args.push("-scheme".to_string());
        args.push(self.scheme.clone());

        args.push("-configuration".to_string());
        args.push(self.configuration.as_str().to_string());

        args.push("-destination".to_string());
        args.push(self.destination.to_xcodebuild_string());

        if let Some(ref dd) = self.derived_data_path {
            args.push("-derivedDataPath".to_string());
            args.push(dd.to_string_lossy().to_string());
        }

        for (k, v) in &self.build_settings {
            args.push(format!("{}={}", k, v));
        }

        for extra in &self.extra_args {
            args.push(extra.clone());
        }
    }
}

/// Xcode build configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildConfiguration {
    Debug,
    Release,
    Custom(String),
}

impl BuildConfiguration {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Debug => "Debug",
            Self::Release => "Release",
            Self::Custom(s) => s.as_str(),
        }
    }
}

impl Default for BuildConfiguration {
    fn default() -> Self {
        Self::Debug
    }
}

/// Build destination for xcodebuild.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Destination {
    /// iOS Simulator (e.g. `platform=iOS Simulator,name=iPhone 16,OS=latest`).
    Simulator { name: String, os: Option<String> },
    /// Physical device (optionally by UDID).
    Device { id: Option<String> },
    /// Generic iOS destination for archive builds.
    GenericIos,
    /// Generic macOS destination.
    GenericMacos,
}

impl Destination {
    /// Format as the `-destination` argument value.
    pub fn to_xcodebuild_string(&self) -> String {
        match self {
            Self::Simulator { name, os } => {
                let os_part = os.as_deref().unwrap_or("latest");
                format!("platform=iOS Simulator,name={},OS={}", name, os_part)
            }
            Self::Device { id } => {
                if let Some(udid) = id {
                    format!("platform=iOS,id={}", udid)
                } else {
                    "generic/platform=iOS".to_string()
                }
            }
            Self::GenericIos => "generic/platform=iOS".to_string(),
            Self::GenericMacos => "platform=macOS".to_string(),
        }
    }
}

impl Default for Destination {
    fn default() -> Self {
        Self::GenericIos
    }
}

// ---------------------------------------------------------------------------
// Export types
// ---------------------------------------------------------------------------

/// Method used when exporting an archive to an IPA.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExportMethod {
    AppStore,
    AdHoc,
    Development,
    Enterprise,
}

impl ExportMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AppStore => "app-store",
            Self::AdHoc => "ad-hoc",
            Self::Development => "development",
            Self::Enterprise => "enterprise",
        }
    }
}

/// Signing style for export.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SigningStyle {
    Automatic,
    Manual,
}

impl SigningStyle {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Manual => "manual",
        }
    }
}

impl Default for SigningStyle {
    fn default() -> Self {
        Self::Automatic
    }
}

/// Options controlling how an `.xcarchive` is exported to an `.ipa`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportOptions {
    pub method: ExportMethod,
    pub team_id: String,
    pub signing_style: SigningStyle,
    pub upload_symbols: bool,
    pub compile_bitcode: bool,
    /// Map of bundle-id to provisioning profile name (only for manual signing).
    pub provisioning_profiles: HashMap<String, String>,
}

impl ExportOptions {
    pub fn new(method: ExportMethod, team_id: impl Into<String>) -> Self {
        Self {
            method,
            team_id: team_id.into(),
            signing_style: SigningStyle::default(),
            upload_symbols: true,
            compile_bitcode: false,
            provisioning_profiles: HashMap::new(),
        }
    }

    pub fn with_signing_style(mut self, style: SigningStyle) -> Self {
        self.signing_style = style;
        self
    }

    pub fn with_upload_symbols(mut self, upload: bool) -> Self {
        self.upload_symbols = upload;
        self
    }

    pub fn with_compile_bitcode(mut self, compile: bool) -> Self {
        self.compile_bitcode = compile;
        self
    }

    pub fn with_provisioning_profile(
        mut self,
        bundle_id: impl Into<String>,
        profile_name: impl Into<String>,
    ) -> Self {
        self.provisioning_profiles
            .insert(bundle_id.into(), profile_name.into());
        self
    }

    /// Generate the `ExportOptions.plist` XML content.
    pub fn to_plist_xml(&self) -> Result<String> {
        let mut dict = plist::Dictionary::new();

        dict.insert(
            "method".to_string(),
            PlistValue::String(self.method.as_str().to_string()),
        );
        dict.insert(
            "teamID".to_string(),
            PlistValue::String(self.team_id.clone()),
        );
        dict.insert(
            "signingStyle".to_string(),
            PlistValue::String(self.signing_style.as_str().to_string()),
        );
        dict.insert(
            "uploadSymbols".to_string(),
            PlistValue::Boolean(self.upload_symbols),
        );
        dict.insert(
            "compileBitcode".to_string(),
            PlistValue::Boolean(self.compile_bitcode),
        );

        if !self.provisioning_profiles.is_empty() {
            let mut profiles = plist::Dictionary::new();
            for (bundle_id, profile_name) in &self.provisioning_profiles {
                profiles.insert(bundle_id.clone(), PlistValue::String(profile_name.clone()));
            }
            dict.insert(
                "provisioningProfiles".to_string(),
                PlistValue::Dictionary(profiles),
            );
        }

        let value = PlistValue::Dictionary(dict);
        let mut buf: Vec<u8> = Vec::new();
        plist::to_writer_xml(&mut buf, &value).map_err(|e| FrameworkError::Context {
            context: "generating ExportOptions.plist".to_string(),
            message: e.to_string(),
        })?;

        String::from_utf8(buf).map_err(|e| FrameworkError::Context {
            context: "encoding ExportOptions.plist".to_string(),
            message: e.to_string(),
        })
    }

    /// Write the plist to a file and return its path.
    pub fn write_to_file(&self, dir: &Path) -> Result<PathBuf> {
        let content = self.to_plist_xml()?;
        let path = dir.join("ExportOptions.plist");
        std::fs::write(&path, &content).map_err(FrameworkError::Io)?;
        Ok(path)
    }
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Result of an `xcodebuild build` invocation.
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub output_path: Option<PathBuf>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
    pub duration: Duration,
}

/// Result of an `xcodebuild test` invocation.
#[derive(Debug, Clone)]
pub struct TestResult {
    pub success: bool,
    pub tests_passed: usize,
    pub tests_failed: usize,
    pub tests_skipped: usize,
    pub result_bundle_path: Option<PathBuf>,
    pub failures: Vec<TestFailure>,
    pub duration: Duration,
}

/// A single test failure with context.
#[derive(Debug, Clone)]
pub struct TestFailure {
    pub test_name: String,
    pub message: String,
}

/// Result of an `xcodebuild archive` invocation.
#[derive(Debug, Clone)]
pub struct ArchiveResult {
    pub success: bool,
    pub archive_path: PathBuf,
    pub duration: Duration,
}

/// Result of an `xcodebuild -exportArchive` invocation.
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub success: bool,
    pub ipa_path: PathBuf,
    pub duration: Duration,
}

// ---------------------------------------------------------------------------
// Runner
// ---------------------------------------------------------------------------

/// Async runner that shells out to `xcodebuild`.
pub struct XcodeBuildRunner;

impl XcodeBuildRunner {
    // -- public entry points ------------------------------------------------

    /// Run `xcodebuild build`.
    #[instrument(skip_all, fields(scheme = %opts.scheme, config = %opts.configuration.as_str()))]
    pub async fn build(opts: &XcodeBuildOptions) -> Result<BuildResult> {
        info!("xcodebuild build");
        let start = Instant::now();

        let mut args = vec!["build".to_string()];
        opts.push_common_args(&mut args);

        let output = Self::run(&args, &opts.env).await?;
        let (warnings, errors) = Self::parse_diagnostics(&output.stdout, &output.stderr);

        let output_path = opts.derived_data_path.clone().map(|dd| {
            dd.join("Build/Products")
                .join(format!("{}-iphoneos", opts.configuration.as_str()))
        });

        Ok(BuildResult {
            success: output.success,
            output_path,
            warnings,
            errors,
            duration: start.elapsed(),
        })
    }

    /// Run `xcodebuild test`.
    #[instrument(skip_all, fields(scheme = %opts.scheme))]
    pub async fn test(
        opts: &XcodeBuildOptions,
        result_bundle_path: Option<&Path>,
        test_plan: Option<&str>,
    ) -> Result<TestResult> {
        info!("xcodebuild test");
        let start = Instant::now();

        let mut args = vec!["test".to_string()];
        opts.push_common_args(&mut args);

        let bundle_path_str;
        if let Some(rbp) = result_bundle_path {
            args.push("-resultBundlePath".to_string());
            bundle_path_str = rbp.to_string_lossy().to_string();
            args.push(bundle_path_str.clone());
        }

        if let Some(plan) = test_plan {
            args.push("-testPlan".to_string());
            args.push(plan.to_string());
        }

        let output = Self::run(&args, &opts.env).await?;
        let (passed, failed, skipped, failures) =
            Self::parse_test_output(&output.stdout, &output.stderr);

        Ok(TestResult {
            success: output.success,
            tests_passed: passed,
            tests_failed: failed,
            tests_skipped: skipped,
            result_bundle_path: result_bundle_path.map(PathBuf::from),
            failures,
            duration: start.elapsed(),
        })
    }

    /// Run `xcodebuild archive`.
    #[instrument(skip_all, fields(scheme = %opts.scheme, archive = %archive_path.display()))]
    pub async fn archive(opts: &XcodeBuildOptions, archive_path: &Path) -> Result<ArchiveResult> {
        info!("xcodebuild archive");
        let start = Instant::now();

        let mut args = vec!["archive".to_string()];
        opts.push_common_args(&mut args);

        args.push("-archivePath".to_string());
        args.push(archive_path.to_string_lossy().to_string());
        args.push("-allowProvisioningUpdates".to_string());

        let output = Self::run(&args, &opts.env).await?;

        if !output.success {
            return Err(FrameworkError::BuildFailed {
                platform: "iOS".to_string(),
                message: format!("xcodebuild archive failed:\n{}", output.stderr),
                source: None,
            });
        }

        Ok(ArchiveResult {
            success: true,
            archive_path: archive_path.to_path_buf(),
            duration: start.elapsed(),
        })
    }

    /// Run `xcodebuild -exportArchive`.
    #[instrument(skip_all, fields(archive = %archive_path.display(), export_dir = %export_dir.display()))]
    pub async fn export_archive(
        archive_path: &Path,
        export_dir: &Path,
        export_options: &ExportOptions,
    ) -> Result<ExportResult> {
        info!("xcodebuild -exportArchive");
        let start = Instant::now();

        // Write export-options plist into the export directory.
        std::fs::create_dir_all(export_dir).map_err(FrameworkError::Io)?;
        let plist_path = export_options.write_to_file(export_dir)?;

        let args = vec![
            "-exportArchive".to_string(),
            "-archivePath".to_string(),
            archive_path.to_string_lossy().to_string(),
            "-exportPath".to_string(),
            export_dir.to_string_lossy().to_string(),
            "-exportOptionsPlist".to_string(),
            plist_path.to_string_lossy().to_string(),
            "-allowProvisioningUpdates".to_string(),
        ];

        let output = Self::run(&args, &HashMap::new()).await?;

        // Clean up the generated plist.
        let _ = std::fs::remove_file(&plist_path);

        if !output.success {
            return Err(FrameworkError::BuildFailed {
                platform: "iOS".to_string(),
                message: format!("xcodebuild -exportArchive failed:\n{}", output.stderr),
                source: None,
            });
        }

        // Find the .ipa in the export directory.
        let ipa_path = Self::find_ipa(export_dir).unwrap_or_else(|| export_dir.join("App.ipa"));

        Ok(ExportResult {
            success: true,
            ipa_path,
            duration: start.elapsed(),
        })
    }

    // -- internal helpers ---------------------------------------------------

    /// Execute xcodebuild with the given args and return structured output.
    async fn run(args: &[String], env: &HashMap<String, String>) -> Result<CommandOutput> {
        let args_str: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        debug!(cmd = %format!("xcodebuild {}", args_str.join(" ")), "executing xcodebuild");

        let output = tokio::process::Command::new("xcodebuild")
            .args(&args_str)
            .envs(env)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcodebuild {}", args_str.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        Ok(CommandOutput {
            success,
            stdout,
            stderr,
        })
    }

    /// Parse xcodebuild output for warnings and errors.
    fn parse_diagnostics(stdout: &str, stderr: &str) -> (Vec<String>, Vec<String>) {
        let mut warnings = Vec::new();
        let mut errors = Vec::new();

        for line in stdout.lines().chain(stderr.lines()) {
            let trimmed = line.trim();
            if trimmed.starts_with("warning:") {
                warnings.push(trimmed.to_string());
            } else if trimmed.starts_with("error:") {
                errors.push(trimmed.to_string());
            } else if trimmed.contains(": warning:") {
                warnings.push(trimmed.to_string());
            } else if trimmed.contains(": error:") {
                errors.push(trimmed.to_string());
            }
        }

        (warnings, errors)
    }

    /// Parse xcodebuild test output for pass/fail/skip counts and failures.
    fn parse_test_output(stdout: &str, stderr: &str) -> (usize, usize, usize, Vec<TestFailure>) {
        let mut passed: usize = 0;
        let mut failed: usize = 0;
        let mut skipped: usize = 0;
        let mut failures = Vec::new();

        let combined = format!("{}\n{}", stdout, stderr);

        for line in combined.lines() {
            let trimmed = line.trim();

            // Test Case '-[SuiteClass testMethod]' passed (0.001 seconds).
            if trimmed.starts_with("Test Case") && trimmed.contains("passed") {
                passed += 1;
            } else if trimmed.starts_with("Test Case") && trimmed.contains("failed") {
                failed += 1;
                // Extract test name from between quotes.
                if let Some(name) = Self::extract_test_name(trimmed) {
                    failures.push(TestFailure {
                        test_name: name,
                        message: trimmed.to_string(),
                    });
                }
            } else if trimmed.starts_with("Test Case") && trimmed.contains("skipped") {
                skipped += 1;
            }

            // Summary line: "Executed 42 tests, with 2 failures (1 unexpected) in 1.234 (1.300) seconds"
            if trimmed.starts_with("Executed") && trimmed.contains("test") {
                if let Some(counts) = Self::parse_summary_line(trimmed) {
                    // Prefer summary counts when available — they are authoritative.
                    let total = counts.0;
                    let fail_count = counts.1;
                    // passed = total - failed (skipped is counted separately by Xcode,
                    // but the summary line lumps everything together)
                    if total > 0 {
                        passed = total.saturating_sub(fail_count);
                        failed = fail_count;
                    }
                }
            }
        }

        (passed, failed, skipped, failures)
    }

    /// Extract the test name from a "Test Case" line.
    fn extract_test_name(line: &str) -> Option<String> {
        // Patterns:
        //  Test Case '-[SuiteClass testMethod]' failed ...
        //  Test Case 'SuiteClass.testMethod' failed ...   (Swift Testing)
        let start = line.find('\'')?;
        let end = line[start + 1..].find('\'')?;
        Some(line[start + 1..start + 1 + end].to_string())
    }

    /// Parse the "Executed N tests, with M failures ..." summary line.
    fn parse_summary_line(line: &str) -> Option<(usize, usize)> {
        // "Executed 42 tests, with 2 failures (1 unexpected) in 1.234 (1.300) seconds"
        let total = line
            .strip_prefix("Executed ")?
            .split_whitespace()
            .next()?
            .parse::<usize>()
            .ok()?;

        let failures = if let Some(idx) = line.find("with ") {
            line[idx + 5..]
                .split_whitespace()
                .next()?
                .parse::<usize>()
                .ok()?
        } else {
            0
        };

        Some((total, failures))
    }

    /// Walk `dir` looking for the first `.ipa` file.
    fn find_ipa(dir: &Path) -> Option<PathBuf> {
        let entries = std::fs::read_dir(dir).ok()?;
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().map(|e| e == "ipa").unwrap_or(false) {
                return Some(p);
            }
        }
        None
    }
}

/// Raw output from an xcodebuild subprocess.
struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Destination formatting ----------------------------------------------

    #[test]
    fn test_destination_simulator() {
        let dest = Destination::Simulator {
            name: "iPhone 16".to_string(),
            os: Some("18.0".to_string()),
        };
        assert_eq!(
            dest.to_xcodebuild_string(),
            "platform=iOS Simulator,name=iPhone 16,OS=18.0"
        );
    }

    #[test]
    fn test_destination_simulator_latest() {
        let dest = Destination::Simulator {
            name: "iPhone 16 Pro".to_string(),
            os: None,
        };
        assert_eq!(
            dest.to_xcodebuild_string(),
            "platform=iOS Simulator,name=iPhone 16 Pro,OS=latest"
        );
    }

    #[test]
    fn test_destination_device_with_id() {
        let dest = Destination::Device {
            id: Some("00008030-001A35E83C38802E".to_string()),
        };
        assert_eq!(
            dest.to_xcodebuild_string(),
            "platform=iOS,id=00008030-001A35E83C38802E"
        );
    }

    #[test]
    fn test_destination_device_without_id() {
        let dest = Destination::Device { id: None };
        assert_eq!(dest.to_xcodebuild_string(), "generic/platform=iOS");
    }

    #[test]
    fn test_destination_generic_ios() {
        assert_eq!(
            Destination::GenericIos.to_xcodebuild_string(),
            "generic/platform=iOS"
        );
    }

    #[test]
    fn test_destination_generic_macos() {
        assert_eq!(
            Destination::GenericMacos.to_xcodebuild_string(),
            "platform=macOS"
        );
    }

    // -- BuildConfiguration --------------------------------------------------

    #[test]
    fn test_build_configuration_as_str() {
        assert_eq!(BuildConfiguration::Debug.as_str(), "Debug");
        assert_eq!(BuildConfiguration::Release.as_str(), "Release");
        assert_eq!(
            BuildConfiguration::Custom("Staging".to_string()).as_str(),
            "Staging"
        );
    }

    // -- ExportMethod --------------------------------------------------------

    #[test]
    fn test_export_method_as_str() {
        assert_eq!(ExportMethod::AppStore.as_str(), "app-store");
        assert_eq!(ExportMethod::AdHoc.as_str(), "ad-hoc");
        assert_eq!(ExportMethod::Development.as_str(), "development");
        assert_eq!(ExportMethod::Enterprise.as_str(), "enterprise");
    }

    // -- SigningStyle ---------------------------------------------------------

    #[test]
    fn test_signing_style_as_str() {
        assert_eq!(SigningStyle::Automatic.as_str(), "automatic");
        assert_eq!(SigningStyle::Manual.as_str(), "manual");
    }

    // -- ExportOptions plist generation --------------------------------------

    #[test]
    fn test_export_options_plist_automatic() {
        let opts = ExportOptions::new(ExportMethod::AppStore, "ABCDE12345");

        let xml = opts.to_plist_xml().unwrap();
        assert!(xml.contains("<key>method</key>"));
        assert!(xml.contains("<string>app-store</string>"));
        assert!(xml.contains("<key>teamID</key>"));
        assert!(xml.contains("<string>ABCDE12345</string>"));
        assert!(xml.contains("<key>signingStyle</key>"));
        assert!(xml.contains("<string>automatic</string>"));
        assert!(xml.contains("<key>uploadSymbols</key>"));
        assert!(xml.contains("<true/>") || xml.contains("<true />"));
        assert!(xml.contains("<key>compileBitcode</key>"));
        assert!(xml.contains("<false/>") || xml.contains("<false />"));
        // No provisioning profiles section for automatic signing
        assert!(!xml.contains("provisioningProfiles"));
    }

    #[test]
    fn test_export_options_plist_manual_with_profiles() {
        let opts = ExportOptions::new(ExportMethod::AdHoc, "TEAM123")
            .with_signing_style(SigningStyle::Manual)
            .with_provisioning_profile("com.example.app", "MyAdHocProfile");

        let xml = opts.to_plist_xml().unwrap();
        assert!(xml.contains("<string>ad-hoc</string>"));
        assert!(xml.contains("<string>manual</string>"));
        assert!(xml.contains("<key>provisioningProfiles</key>"));
        assert!(xml.contains("<key>com.example.app</key>"));
        assert!(xml.contains("<string>MyAdHocProfile</string>"));
    }

    #[test]
    fn test_export_options_write_to_file() {
        let dir = tempfile::tempdir().unwrap();
        let opts = ExportOptions::new(ExportMethod::Development, "DEV_TEAM");

        let path = opts.write_to_file(dir.path()).unwrap();
        assert!(path.exists());
        assert_eq!(path.file_name().unwrap(), "ExportOptions.plist");

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("<string>development</string>"));
    }

    // -- XcodeBuildOptions argument building ----------------------------------

    #[test]
    fn test_options_common_args_workspace() {
        let opts = XcodeBuildOptions::new("/path/to/App.xcworkspace", "MyScheme")
            .with_configuration(BuildConfiguration::Release)
            .with_destination(Destination::GenericIos)
            .with_derived_data_path("/tmp/dd")
            .with_build_setting("DEVELOPMENT_TEAM", "ABCDE");

        let mut args = Vec::new();
        opts.push_common_args(&mut args);

        assert!(args.contains(&"-workspace".to_string()));
        assert!(args.contains(&"/path/to/App.xcworkspace".to_string()));
        assert!(args.contains(&"-scheme".to_string()));
        assert!(args.contains(&"MyScheme".to_string()));
        assert!(args.contains(&"-configuration".to_string()));
        assert!(args.contains(&"Release".to_string()));
        assert!(args.contains(&"-destination".to_string()));
        assert!(args.contains(&"generic/platform=iOS".to_string()));
        assert!(args.contains(&"-derivedDataPath".to_string()));
        assert!(args.contains(&"/tmp/dd".to_string()));
        assert!(args.contains(&"DEVELOPMENT_TEAM=ABCDE".to_string()));
    }

    #[test]
    fn test_options_common_args_project() {
        let opts = XcodeBuildOptions::new("/path/to/App.xcodeproj", "App");

        let mut args = Vec::new();
        opts.push_common_args(&mut args);

        assert!(args.contains(&"-project".to_string()));
        assert!(args.contains(&"/path/to/App.xcodeproj".to_string()));
    }

    #[test]
    fn test_options_extra_args() {
        let opts = XcodeBuildOptions::new("/p/A.xcodeproj", "S")
            .with_extra_arg("-quiet")
            .with_extra_arg("-showBuildTimingSummary");

        let mut args = Vec::new();
        opts.push_common_args(&mut args);

        assert!(args.contains(&"-quiet".to_string()));
        assert!(args.contains(&"-showBuildTimingSummary".to_string()));
    }

    // -- Diagnostic parsing --------------------------------------------------

    #[test]
    fn test_parse_diagnostics() {
        let stdout = "\
/path/File.swift:10:5: warning: unused variable 'x'
/path/File.swift:20:5: error: cannot find 'foo' in scope
BUILD SUCCEEDED
";
        let stderr = "warning: some generic warning\n";

        let (warnings, errors) = XcodeBuildRunner::parse_diagnostics(stdout, stderr);

        assert_eq!(warnings.len(), 2);
        assert_eq!(errors.len(), 1);
        assert!(warnings[0].contains("unused variable"));
        assert!(warnings[1].contains("some generic warning"));
        assert!(errors[0].contains("cannot find 'foo'"));
    }

    #[test]
    fn test_parse_diagnostics_empty() {
        let (warnings, errors) = XcodeBuildRunner::parse_diagnostics("", "");
        assert!(warnings.is_empty());
        assert!(errors.is_empty());
    }

    // -- Test output parsing -------------------------------------------------

    #[test]
    fn test_parse_test_output_basic() {
        let stdout = "\
Test Case '-[MyTests testA]' passed (0.001 seconds).
Test Case '-[MyTests testB]' passed (0.002 seconds).
Test Case '-[MyTests testC]' failed (0.003 seconds).
";

        let (passed, failed, skipped, failures) = XcodeBuildRunner::parse_test_output(stdout, "");

        assert_eq!(passed, 2);
        assert_eq!(failed, 1);
        assert_eq!(skipped, 0);
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].test_name, "-[MyTests testC]");
    }

    #[test]
    fn test_parse_test_output_with_summary() {
        let stdout = "\
Test Case '-[S testA]' passed (0.001 seconds).
Test Case '-[S testB]' passed (0.002 seconds).
Executed 10 tests, with 2 failures (1 unexpected) in 1.234 (1.300) seconds
";

        let (passed, failed, _skipped, _failures) = XcodeBuildRunner::parse_test_output(stdout, "");

        // Summary should override individual counts.
        assert_eq!(passed, 8);
        assert_eq!(failed, 2);
    }

    #[test]
    fn test_parse_test_output_skipped() {
        let stdout = "\
Test Case '-[S testA]' passed (0.001 seconds).
Test Case '-[S testB]' skipped (0.000 seconds).
";

        let (passed, _failed, skipped, _) = XcodeBuildRunner::parse_test_output(stdout, "");

        assert_eq!(passed, 1);
        assert_eq!(skipped, 1);
    }

    #[test]
    fn test_extract_test_name() {
        let line = "Test Case '-[MyTests testSomething]' failed (0.003 seconds).";
        assert_eq!(
            XcodeBuildRunner::extract_test_name(line),
            Some("-[MyTests testSomething]".to_string())
        );
    }

    #[test]
    fn test_extract_test_name_swift() {
        let line = "Test Case 'MyTests.testSomething' failed (0.003 seconds).";
        assert_eq!(
            XcodeBuildRunner::extract_test_name(line),
            Some("MyTests.testSomething".to_string())
        );
    }

    #[test]
    fn test_parse_summary_line() {
        let line = "Executed 42 tests, with 3 failures (2 unexpected) in 5.678 (6.000) seconds";
        let (total, failures) = XcodeBuildRunner::parse_summary_line(line).unwrap();
        assert_eq!(total, 42);
        assert_eq!(failures, 3);
    }

    #[test]
    fn test_parse_summary_line_zero_failures() {
        let line = "Executed 10 tests, with 0 failures (0 unexpected) in 1.000 (1.100) seconds";
        let (total, failures) = XcodeBuildRunner::parse_summary_line(line).unwrap();
        assert_eq!(total, 10);
        assert_eq!(failures, 0);
    }
}
