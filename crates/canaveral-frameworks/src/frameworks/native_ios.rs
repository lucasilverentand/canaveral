//! Native iOS (Swift/Objective-C) framework adapter
//!
//! Supports building, testing, archiving, and exporting native iOS apps using
//! xcodebuild. The heavy lifting is delegated to [`crate::xcodebuild`].

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use async_trait::async_trait;
use plist::Value as PlistValue;
use tracing::{debug, info, instrument};

use crate::artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
use crate::capabilities::Capabilities;
use crate::context::{BuildContext, BuildProfile, TestContext};
use crate::detection::{file_exists, Detection};
use crate::error::{FrameworkError, Result};
use crate::traits::{
    BuildAdapter, Platform, PrerequisiteStatus, TestAdapter, TestCase, TestReport, TestStatus,
    TestSuite, ToolStatus, VersionInfo,
};
use crate::xcodebuild::{
    ArchiveResult, BuildConfiguration, BuildResult, Destination, ExportMethod, ExportOptions,
    ExportResult, SigningStyle, TestResult, XcodeBuildOptions, XcodeBuildRunner,
};

/// Native iOS build adapter.
///
/// Implements both [`BuildAdapter`] and [`TestAdapter`] so it can be registered
/// as a build adapter (as before) and as a test adapter for running XCTest /
/// Swift Testing suites.
pub struct NativeIosAdapter;

impl NativeIosAdapter {
    pub fn new() -> Self {
        Self
    }

    // -----------------------------------------------------------------------
    // Project discovery helpers
    // -----------------------------------------------------------------------

    /// Find `.xcworkspace` or `.xcodeproj` in the given path.
    fn find_xcode_project(&self, path: &Path) -> Result<XcodeProject> {
        let mut workspace: Option<PathBuf> = None;
        let mut project: Option<PathBuf> = None;

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.ends_with(".xcworkspace") && !name_str.starts_with("project.") {
                    workspace = Some(entry_path);
                } else if name_str.ends_with(".xcodeproj") {
                    project = Some(entry_path);
                }
            }
        }

        // Prefer workspace over project (CocoaPods/SPM)
        if let Some(ws) = workspace {
            Ok(XcodeProject::Workspace(ws))
        } else if let Some(proj) = project {
            Ok(XcodeProject::Project(proj))
        } else {
            Err(FrameworkError::InvalidConfig {
                message: "No .xcworkspace or .xcodeproj found".to_string(),
            })
        }
    }

    /// List available schemes via `xcodebuild -list -json`.
    fn list_schemes(&self, path: &Path, project: &XcodeProject) -> Result<Vec<String>> {
        let mut args = vec!["-list", "-json"];
        match project {
            XcodeProject::Workspace(ws) => {
                args.push("-workspace");
                args.push(ws.to_str().unwrap());
            }
            XcodeProject::Project(proj) => {
                args.push("-project");
                args.push(proj.to_str().unwrap());
            }
        }

        let output = Command::new("xcodebuild")
            .args(&args)
            .current_dir(path)
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcodebuild {}", args.join(" ")),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "xcodebuild -list".to_string(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value =
            serde_json::from_str(&stdout).map_err(|e| FrameworkError::Context {
                context: "parsing xcodebuild -list output".to_string(),
                message: e.to_string(),
            })?;

        let schemes = if let Some(ws) = json.get("workspace") {
            ws.get("schemes")
        } else {
            json.get("project").and_then(|p| p.get("schemes"))
        };

        let schemes = schemes
            .and_then(|s| s.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Ok(schemes)
    }

    /// Get default scheme (first non-test scheme or first scheme).
    fn get_default_scheme(&self, schemes: &[String]) -> Option<String> {
        schemes
            .iter()
            .find(|s| !s.ends_with("Tests") && !s.ends_with("UITests"))
            .or_else(|| schemes.first())
            .cloned()
    }

    /// Resolve the scheme to use from context config or auto-detection.
    fn resolve_scheme(
        &self,
        path: &Path,
        project: &XcodeProject,
        config: &HashMap<String, serde_json::Value>,
    ) -> Result<String> {
        if let Some(scheme) = config.get("scheme").and_then(|v| v.as_str()) {
            return Ok(scheme.to_string());
        }

        let schemes = self.list_schemes(path, project)?;
        self.get_default_scheme(&schemes)
            .ok_or_else(|| FrameworkError::InvalidConfig {
                message: "No scheme found or specified. Use --config scheme=<name>".to_string(),
            })
    }

    // -----------------------------------------------------------------------
    // Public xcodebuild operations
    // -----------------------------------------------------------------------

    /// Run `xcodebuild build` (compile only, no archive).
    ///
    /// Returns a structured [`BuildResult`] with warnings, errors, and output
    /// path.
    #[instrument(skip(self, opts), fields(scheme = %opts.scheme))]
    pub async fn build_xcode(&self, opts: &XcodeBuildOptions) -> Result<BuildResult> {
        XcodeBuildRunner::build(opts).await
    }

    /// Run `xcodebuild test`.
    ///
    /// A simulator destination is required. Optionally pass a result-bundle
    /// path and/or a test-plan name.
    #[instrument(skip(self, opts), fields(scheme = %opts.scheme))]
    pub async fn test_xcode(
        &self,
        opts: &XcodeBuildOptions,
        result_bundle_path: Option<&Path>,
        test_plan: Option<&str>,
    ) -> Result<TestResult> {
        XcodeBuildRunner::test(opts, result_bundle_path, test_plan).await
    }

    /// Run `xcodebuild archive`.
    #[instrument(skip(self, opts), fields(scheme = %opts.scheme, archive = %archive_path.display()))]
    pub async fn archive(
        &self,
        opts: &XcodeBuildOptions,
        archive_path: &Path,
    ) -> Result<ArchiveResult> {
        XcodeBuildRunner::archive(opts, archive_path).await
    }

    /// Run `xcodebuild -exportArchive` to produce an `.ipa`.
    #[instrument(skip(self, export_options), fields(archive = %archive_path.display()))]
    pub async fn export_archive(
        &self,
        archive_path: &Path,
        export_dir: &Path,
        export_options: &ExportOptions,
    ) -> Result<ExportResult> {
        XcodeBuildRunner::export_archive(archive_path, export_dir, export_options).await
    }

    // -----------------------------------------------------------------------
    // Plist & version helpers
    // -----------------------------------------------------------------------

    /// Find `Info.plist` path.
    fn find_info_plist(&self, path: &Path) -> Result<PathBuf> {
        let candidates = [
            path.join("Info.plist"),
            path.join("App/Info.plist"),
            path.join("Sources/Info.plist"),
        ];

        for candidate in &candidates {
            if candidate.exists() {
                return Ok(candidate.clone());
            }
        }

        self.find_file_recursive(path, "Info.plist", 3)
            .ok_or_else(|| FrameworkError::Context {
                context: "finding Info.plist".to_string(),
                message: "Info.plist not found".to_string(),
            })
    }

    /// Find a file recursively up to `max_depth`.
    #[allow(clippy::only_used_in_recursion)]
    fn find_file_recursive(
        &self,
        path: &Path,
        filename: &str,
        max_depth: usize,
    ) -> Option<PathBuf> {
        if max_depth == 0 {
            return None;
        }

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_file()
                    && entry_path
                        .file_name()
                        .map(|n| n == filename)
                        .unwrap_or(false)
                {
                    let path_str = entry_path.to_string_lossy();
                    if !path_str.contains("DerivedData")
                        && !path_str.contains("build/")
                        && !path_str.contains(".build/")
                        && !path_str.contains("Pods/")
                    {
                        return Some(entry_path);
                    }
                }
            }

            // Recurse into subdirectories
            for entry in std::fs::read_dir(path).ok()?.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    let name = entry_path.file_name()?.to_string_lossy();
                    if !name.starts_with('.')
                        && name != "DerivedData"
                        && name != "build"
                        && name != "Pods"
                        && name != "Carthage"
                    {
                        if let Some(found) =
                            self.find_file_recursive(&entry_path, filename, max_depth - 1)
                        {
                            return Some(found);
                        }
                    }
                }
            }
        }

        None
    }

    /// Parse version from `Info.plist`.
    fn parse_info_plist_version(&self, plist_path: &Path) -> Result<VersionInfo> {
        let plist: plist::Dictionary =
            plist::from_file(plist_path).map_err(|e| FrameworkError::Context {
                context: "parsing Info.plist".to_string(),
                message: e.to_string(),
            })?;

        let version = plist
            .get("CFBundleShortVersionString")
            .and_then(|v| v.as_string())
            .map(String::from)
            .ok_or_else(|| FrameworkError::VersionParseError {
                message: "CFBundleShortVersionString not found in Info.plist".to_string(),
            })?;

        let build_number = plist
            .get("CFBundleVersion")
            .and_then(|v| v.as_string())
            .and_then(|s| s.parse::<u64>().ok());

        Ok(VersionInfo {
            version,
            build_number,
            ..Default::default()
        })
    }

    /// Update version in `Info.plist`.
    fn update_info_plist_version(&self, plist_path: &Path, version: &VersionInfo) -> Result<()> {
        let mut plist: plist::Dictionary =
            plist::from_file(plist_path).map_err(|e| FrameworkError::Context {
                context: "reading Info.plist".to_string(),
                message: e.to_string(),
            })?;

        plist.insert(
            "CFBundleShortVersionString".to_string(),
            PlistValue::String(version.version.clone()),
        );

        if let Some(build) = version.build_number {
            plist.insert(
                "CFBundleVersion".to_string(),
                PlistValue::String(build.to_string()),
            );
        }

        let file = std::fs::File::create(plist_path).map_err(FrameworkError::Io)?;
        plist::to_writer_xml(file, &PlistValue::Dictionary(plist)).map_err(|e| {
            FrameworkError::Context {
                context: "writing Info.plist".to_string(),
                message: e.to_string(),
            }
        })?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Artifact helpers
    // -----------------------------------------------------------------------

    /// Find built artifacts in archive/export directory.
    fn find_artifacts(&self, path: &Path, is_archive: bool) -> Vec<Artifact> {
        let mut artifacts = Vec::new();

        // Check for IPA files
        if let Some(ipa) = self.find_file_recursive(path, "*.ipa", 5).or_else(|| {
            self.find_files_with_extension(path, "ipa")
                .into_iter()
                .next()
        }) {
            let metadata = ArtifactMetadata::new()
                .with_framework("native-ios")
                .with_signed(true);
            artifacts
                .push(Artifact::new(ipa, ArtifactKind::Ipa, Platform::Ios).with_metadata(metadata));
        }

        // Check for xcarchive
        if is_archive {
            if let Some(archive) = self
                .find_files_with_extension(path, "xcarchive")
                .into_iter()
                .next()
            {
                let metadata = ArtifactMetadata::new().with_framework("native-ios");
                artifacts.push(
                    Artifact::new(archive, ArtifactKind::XcArchive, Platform::Ios)
                        .with_metadata(metadata),
                );
            }
        }

        // Check for app bundles
        for app in self.find_files_with_extension(path, "app") {
            if artifacts
                .iter()
                .any(|a| matches!(a.kind, ArtifactKind::Ipa))
            {
                continue;
            }
            let metadata = ArtifactMetadata::new().with_framework("native-ios");
            artifacts
                .push(Artifact::new(app, ArtifactKind::App, Platform::Ios).with_metadata(metadata));
        }

        artifacts
    }

    /// Find files with given extension.
    fn find_files_with_extension(&self, path: &Path, ext: &str) -> Vec<PathBuf> {
        let mut results = Vec::new();

        fn walk(dir: &Path, ext: &str, results: &mut Vec<PathBuf>, depth: usize) {
            if depth > 5 {
                return;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        walk(&path, ext, results, depth + 1);
                    } else if path.extension().map(|e| e == ext).unwrap_or(false) {
                        results.push(path);
                    }
                }
            }
        }

        walk(path, ext, &mut results, 0);
        results
    }

    // -----------------------------------------------------------------------
    // BuildContext → ExportOptions bridge
    // -----------------------------------------------------------------------

    /// Derive [`ExportOptions`] from a [`BuildContext`].
    fn export_options_from_ctx(&self, ctx: &BuildContext) -> ExportOptions {
        let method = match ctx.profile {
            BuildProfile::Debug => ExportMethod::Development,
            BuildProfile::Release | BuildProfile::Profile => {
                if let Some(ref signing) = ctx.signing {
                    if signing
                        .provisioning_profile
                        .as_ref()
                        .map(|p| p.contains("AdHoc"))
                        .unwrap_or(false)
                    {
                        ExportMethod::AdHoc
                    } else if signing
                        .provisioning_profile
                        .as_ref()
                        .map(|p| p.contains("Enterprise"))
                        .unwrap_or(false)
                    {
                        ExportMethod::Enterprise
                    } else {
                        ExportMethod::AppStore
                    }
                } else {
                    ExportMethod::AppStore
                }
            }
        };

        let team_id = ctx
            .signing
            .as_ref()
            .and_then(|s| s.team_id.clone())
            .unwrap_or_default();

        let signing_style = ctx
            .signing
            .as_ref()
            .map(|s| {
                if s.automatic {
                    SigningStyle::Automatic
                } else {
                    SigningStyle::Manual
                }
            })
            .unwrap_or(SigningStyle::Automatic);

        let mut opts = ExportOptions::new(method, team_id).with_signing_style(signing_style);

        // Wire up provisioning profiles for manual signing.
        if signing_style == SigningStyle::Manual {
            if let Some(ref signing) = ctx.signing {
                if let Some(ref profile) = signing.provisioning_profile {
                    let bundle_id = ctx
                        .config
                        .get("bundle_id")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .unwrap_or_else(|| "com.example.app".to_string());
                    opts = opts.with_provisioning_profile(bundle_id, profile);
                }
            }
        }

        opts
    }

    /// Map [`BuildProfile`] to [`BuildConfiguration`].
    fn build_configuration(profile: &BuildProfile) -> BuildConfiguration {
        match profile {
            BuildProfile::Debug => BuildConfiguration::Debug,
            BuildProfile::Release | BuildProfile::Profile => BuildConfiguration::Release,
        }
    }

    /// Run CocoaPods install if a Podfile exists and Pods/ doesn't.
    async fn ensure_pods(&self, path: &Path) -> Result<()> {
        if file_exists(path, "Podfile") && !file_exists(path, "Pods") {
            info!("installing CocoaPods dependencies");
            let output = tokio::process::Command::new("pod")
                .args(["install"])
                .current_dir(path)
                .output()
                .await
                .map_err(|e| FrameworkError::CommandFailed {
                    command: "pod install".to_string(),
                    exit_code: None,
                    stdout: String::new(),
                    stderr: e.to_string(),
                })?;

            if !output.status.success() {
                return Err(FrameworkError::BuildFailed {
                    platform: "iOS".to_string(),
                    message: format!(
                        "pod install failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                    source: None,
                });
            }
        }
        Ok(())
    }
}

impl Default for NativeIosAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal enum representing the detected Xcode project type.
enum XcodeProject {
    Workspace(PathBuf),
    Project(PathBuf),
}

// ---------------------------------------------------------------------------
// BuildAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl BuildAdapter for NativeIosAdapter {
    fn id(&self) -> &'static str {
        "native-ios"
    }

    fn name(&self) -> &'static str {
        "Native iOS (Xcode)"
    }

    fn detect(&self, path: &Path) -> Detection {
        debug!(path = %path.display(), "detecting native iOS project");
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name_str = name.to_string_lossy();

                if name_str.ends_with(".xcworkspace") && !name_str.starts_with("project.") {
                    return Detection::Yes(85);
                }
                if name_str.ends_with(".xcodeproj") {
                    return Detection::Yes(80);
                }
            }
        }

        if file_exists(path, "Package.swift") {
            return Detection::Maybe(50);
        }

        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::native_ios()
    }

    fn supported_platforms(&self) -> &[Platform] {
        &[Platform::Ios]
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        let mut status = PrerequisiteStatus::ok();

        match which::which("xcodebuild") {
            Ok(_) => {
                let version = Command::new("xcodebuild")
                    .arg("-version")
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
                        s.lines()
                            .next()
                            .and_then(|line| line.strip_prefix("Xcode "))
                            .map(String::from)
                    });

                status = status.with_tool(ToolStatus::found("xcodebuild", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "xcodebuild",
                    "Install Xcode from the App Store",
                ));
            }
        }

        match which::which("xcrun") {
            Ok(_) => {
                status = status.with_tool(ToolStatus::found("xcrun", None));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "xcrun",
                    "Install Xcode Command Line Tools: xcode-select --install",
                ));
            }
        }

        match which::which("pod") {
            Ok(_) => {
                let version = Command::new("pod")
                    .arg("--version")
                    .output()
                    .ok()
                    .and_then(|o| {
                        if o.status.success() {
                            String::from_utf8(o.stdout)
                                .ok()
                                .map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    });
                status = status.with_tool(ToolStatus::found("pod", version));
            }
            Err(_) => {
                status = status.with_tool(ToolStatus::missing(
                    "pod",
                    "(Optional) Install CocoaPods: gem install cocoapods",
                ));
            }
        }

        Ok(status)
    }

    #[instrument(skip(self, ctx), fields(framework = "native-ios", platform = "ios"))]
    async fn build(&self, ctx: &BuildContext) -> Result<Vec<Artifact>> {
        let path = &ctx.path;
        info!(profile = ?ctx.profile, "building native iOS project");

        let xcode_project = self.find_xcode_project(path)?;
        let scheme = self.resolve_scheme(path, &xcode_project, &ctx.config)?;
        let configuration = Self::build_configuration(&ctx.profile);

        let output_dir = ctx
            .output_dir
            .clone()
            .unwrap_or_else(|| path.join("build/ios"));
        std::fs::create_dir_all(&output_dir).map_err(FrameworkError::Io)?;

        // Install CocoaPods if needed.
        self.ensure_pods(path).await?;

        let project_path = match &xcode_project {
            XcodeProject::Workspace(ws) => ws.clone(),
            XcodeProject::Project(proj) => proj.clone(),
        };

        let mut env = ctx.env.clone();
        if ctx.ci {
            env.insert("CI".to_string(), "true".to_string());
        }

        let mut opts = XcodeBuildOptions::new(&project_path, &scheme)
            .with_configuration(configuration)
            .with_destination(Destination::GenericIos);

        // Apply env.
        for (k, v) in &env {
            opts = opts.with_env(k, v);
        }

        // Apply signing build settings.
        if let Some(ref signing) = ctx.signing {
            if let Some(ref team_id) = signing.team_id {
                opts = opts.with_build_setting("DEVELOPMENT_TEAM", team_id);
            }
            if !signing.automatic {
                opts = opts.with_build_setting("CODE_SIGN_STYLE", "Manual");
                if let Some(ref identity) = signing.identity {
                    opts = opts.with_build_setting("CODE_SIGN_IDENTITY", identity);
                }
            }
        }

        // Archive
        let archive_path = output_dir.join(format!("{}.xcarchive", scheme));
        let archive_result = self.archive(&opts, &archive_path).await?;
        info!(
            duration_secs = archive_result.duration.as_secs_f64(),
            "archive complete"
        );

        // For release builds, export IPA.
        if matches!(ctx.profile, BuildProfile::Release | BuildProfile::Profile) {
            let export_options = self.export_options_from_ctx(ctx);
            let export_result = self
                .export_archive(&archive_path, &output_dir, &export_options)
                .await?;
            info!(
                ipa = %export_result.ipa_path.display(),
                duration_secs = export_result.duration.as_secs_f64(),
                "export complete"
            );
        }

        // Collect artifacts.
        let artifacts = self.find_artifacts(&output_dir, true);

        if artifacts.is_empty() {
            return Err(FrameworkError::ArtifactNotFound {
                expected_path: output_dir,
            });
        }

        Ok(artifacts)
    }

    async fn clean(&self, path: &Path) -> Result<()> {
        let derived_data = path.join("DerivedData");
        if derived_data.exists() {
            std::fs::remove_dir_all(&derived_data).map_err(FrameworkError::Io)?;
        }

        let build_dir = path.join("build");
        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir).map_err(FrameworkError::Io)?;
        }

        if let Ok(xcode_project) = self.find_xcode_project(path) {
            let project_path = match &xcode_project {
                XcodeProject::Workspace(ws) => ws.clone(),
                XcodeProject::Project(proj) => proj.clone(),
            };

            let opts = XcodeBuildOptions::new(&project_path, "placeholder")
                .with_destination(Destination::GenericIos);

            // Best-effort clean — ignore errors.
            let _ = XcodeBuildRunner::build(&opts).await;
        }

        Ok(())
    }

    fn get_version(&self, path: &Path) -> Result<VersionInfo> {
        let info_plist = self.find_info_plist(path)?;
        self.parse_info_plist_version(&info_plist)
    }

    fn set_version(&self, path: &Path, version: &VersionInfo) -> Result<()> {
        let info_plist = self.find_info_plist(path)?;
        self.update_info_plist_version(&info_plist, version)
    }
}

// ---------------------------------------------------------------------------
// TestAdapter implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl TestAdapter for NativeIosAdapter {
    fn id(&self) -> &'static str {
        "native-ios"
    }

    fn name(&self) -> &'static str {
        "Native iOS (Xcode)"
    }

    fn detect(&self, path: &Path) -> Detection {
        // Detect if there is an Xcode project with at least one test scheme.
        if let Ok(xcode_project) = self.find_xcode_project(path) {
            if let Ok(schemes) = self.list_schemes(path, &xcode_project) {
                let has_tests = schemes
                    .iter()
                    .any(|s| s.ends_with("Tests") || s.ends_with("UITests"));
                if has_tests {
                    return Detection::Yes(85);
                }
            }
            // Has a project but couldn't confirm test schemes — still possible.
            return Detection::Maybe(50);
        }
        Detection::No
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::native_ios()
    }

    async fn check_prerequisites(&self) -> Result<PrerequisiteStatus> {
        // Same checks as BuildAdapter.
        <Self as BuildAdapter>::check_prerequisites(self).await
    }

    #[instrument(skip(self, ctx), fields(framework = "native-ios"))]
    async fn test(&self, ctx: &TestContext) -> Result<TestReport> {
        let path = &ctx.path;
        info!("running native iOS tests");

        let xcode_project = self.find_xcode_project(path)?;
        let scheme = self.resolve_scheme(path, &xcode_project, &ctx.config)?;

        self.ensure_pods(path).await?;

        let project_path = match &xcode_project {
            XcodeProject::Workspace(ws) => ws.clone(),
            XcodeProject::Project(proj) => proj.clone(),
        };

        // Default simulator destination.
        let destination = ctx
            .config
            .get("destination")
            .and_then(|v| v.as_str())
            .map(|_| {
                let name = ctx
                    .config
                    .get("simulator_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("iPhone 16")
                    .to_string();
                let os = ctx
                    .config
                    .get("simulator_os")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                Destination::Simulator { name, os }
            })
            .unwrap_or_else(|| Destination::Simulator {
                name: "iPhone 16".to_string(),
                os: None,
            });

        let mut opts = XcodeBuildOptions::new(&project_path, &scheme)
            .with_configuration(BuildConfiguration::Debug)
            .with_destination(destination);

        // Apply env.
        let mut env = ctx.env.clone();
        if ctx.ci {
            env.insert("CI".to_string(), "true".to_string());
        }
        for (k, v) in &env {
            opts = opts.with_env(k, v);
        }

        // Result bundle path for xcresult parsing.
        let result_bundle_dir = path.join("build/test-results");
        std::fs::create_dir_all(&result_bundle_dir).map_err(FrameworkError::Io)?;
        let result_bundle_path = result_bundle_dir.join(format!("{}.xcresult", scheme));
        // Remove stale result bundle if it exists (xcodebuild refuses to
        // overwrite).
        if result_bundle_path.exists() {
            let _ = std::fs::remove_dir_all(&result_bundle_path);
        }

        let test_plan = ctx
            .config
            .get("test_plan")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

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

        let result = self
            .test_xcode(&opts, Some(&result_bundle_path), test_plan.as_deref())
            .await?;

        // Convert xcodebuild::TestResult → traits::TestReport.
        let mut suites = Vec::new();

        if !result.failures.is_empty() {
            let failed_cases: Vec<TestCase> = result
                .failures
                .iter()
                .map(|f| TestCase {
                    name: f.test_name.clone(),
                    status: TestStatus::Failed,
                    duration_ms: 0,
                    error: Some(f.message.clone()),
                })
                .collect();

            suites.push(TestSuite {
                name: scheme.clone(),
                tests: failed_cases,
                duration_ms: result.duration.as_millis() as u64,
            });
        }

        Ok(TestReport {
            passed: result.tests_passed,
            failed: result.tests_failed,
            skipped: result.tests_skipped,
            duration_ms: result.duration.as_millis() as u64,
            suites,
            coverage: None,
        })
    }
}

// ---------------------------------------------------------------------------
// Extension trait for XcodeBuildOptions env convenience
// ---------------------------------------------------------------------------

impl XcodeBuildOptions {
    /// Bulk-set environment variables from a map.
    pub fn with_env_map(mut self, map: HashMap<String, String>) -> Self {
        self.env = map;
        self
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_detection_xcodeproj() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcodeproj")).unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = BuildAdapter::detect(&adapter, dir.path());

        assert!(matches!(detection, Detection::Yes(80)));
    }

    #[test]
    fn test_detection_xcworkspace() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcworkspace")).unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = BuildAdapter::detect(&adapter, dir.path());

        assert!(matches!(detection, Detection::Yes(85)));
    }

    #[test]
    fn test_detection_no_project() {
        let dir = tempdir().unwrap();

        let adapter = NativeIosAdapter::new();
        let detection = BuildAdapter::detect(&adapter, dir.path());

        assert!(matches!(detection, Detection::No));
    }

    #[test]
    fn test_version_parsing() {
        let dir = tempdir().unwrap();
        let plist_path = dir.path().join("Info.plist");

        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleShortVersionString</key>
    <string>1.2.3</string>
    <key>CFBundleVersion</key>
    <string>42</string>
    <key>CFBundleIdentifier</key>
    <string>com.example.myapp</string>
</dict>
</plist>"#;

        let mut file = std::fs::File::create(&plist_path).unwrap();
        file.write_all(plist_content.as_bytes()).unwrap();

        let adapter = NativeIosAdapter::new();
        let version = adapter.parse_info_plist_version(&plist_path).unwrap();

        assert_eq!(version.version, "1.2.3");
        assert_eq!(version.build_number, Some(42));
    }

    #[test]
    fn test_capabilities() {
        let adapter = NativeIosAdapter::new();
        let caps = BuildAdapter::capabilities(&adapter);

        assert!(caps.has(Capability::BuildIos));
        assert!(caps.has(Capability::ReleaseBuild));
        assert!(caps.has(Capability::DebugBuild));
        assert!(caps.has(Capability::UnitTests));
        assert!(caps.has(Capability::IntegrationTests));
        assert!(caps.has(Capability::CodeSigning));
    }

    #[test]
    fn test_build_configuration_mapping() {
        assert_eq!(
            NativeIosAdapter::build_configuration(&BuildProfile::Debug),
            BuildConfiguration::Debug
        );
        assert_eq!(
            NativeIosAdapter::build_configuration(&BuildProfile::Release),
            BuildConfiguration::Release
        );
        assert_eq!(
            NativeIosAdapter::build_configuration(&BuildProfile::Profile),
            BuildConfiguration::Release
        );
    }

    #[test]
    fn test_export_options_from_ctx_default() {
        let ctx = BuildContext::new("/project", Platform::Ios).with_profile(BuildProfile::Release);

        let adapter = NativeIosAdapter::new();
        let opts = adapter.export_options_from_ctx(&ctx);

        assert_eq!(opts.method, ExportMethod::AppStore);
        assert_eq!(opts.signing_style, SigningStyle::Automatic);
        assert!(opts.upload_symbols);
        assert!(!opts.compile_bitcode);
    }

    #[test]
    fn test_export_options_from_ctx_debug() {
        let ctx = BuildContext::new("/project", Platform::Ios).with_profile(BuildProfile::Debug);

        let adapter = NativeIosAdapter::new();
        let opts = adapter.export_options_from_ctx(&ctx);

        assert_eq!(opts.method, ExportMethod::Development);
    }

    #[test]
    fn test_export_options_from_ctx_manual_signing() {
        use crate::context::SigningConfig;

        let signing = SigningConfig {
            automatic: false,
            team_id: Some("TEAM123".to_string()),
            provisioning_profile: Some("MyProfile".to_string()),
            identity: Some("iPhone Distribution".to_string()),
            ..Default::default()
        };

        let ctx = BuildContext::new("/project", Platform::Ios)
            .with_profile(BuildProfile::Release)
            .with_signing(signing);

        let adapter = NativeIosAdapter::new();
        let opts = adapter.export_options_from_ctx(&ctx);

        assert_eq!(opts.method, ExportMethod::AppStore);
        assert_eq!(opts.signing_style, SigningStyle::Manual);
        assert_eq!(opts.team_id, "TEAM123");
        assert!(opts.provisioning_profiles.contains_key("com.example.app"));
        assert_eq!(
            opts.provisioning_profiles.get("com.example.app"),
            Some(&"MyProfile".to_string())
        );
    }

    #[test]
    fn test_find_xcode_project_workspace_preferred() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcodeproj")).unwrap();
        std::fs::create_dir(dir.path().join("MyApp.xcworkspace")).unwrap();

        let adapter = NativeIosAdapter::new();
        let project = adapter.find_xcode_project(dir.path()).unwrap();

        assert!(matches!(project, XcodeProject::Workspace(_)));
    }

    #[test]
    fn test_get_default_scheme() {
        let adapter = NativeIosAdapter::new();

        let schemes = vec![
            "MyApp".to_string(),
            "MyAppTests".to_string(),
            "MyAppUITests".to_string(),
        ];
        assert_eq!(
            adapter.get_default_scheme(&schemes),
            Some("MyApp".to_string())
        );

        let test_only = vec!["MyAppTests".to_string()];
        assert_eq!(
            adapter.get_default_scheme(&test_only),
            Some("MyAppTests".to_string())
        );

        let empty: Vec<String> = vec![];
        assert_eq!(adapter.get_default_scheme(&empty), None);
    }

    #[test]
    fn test_version_update_roundtrip() {
        let dir = tempdir().unwrap();
        let plist_path = dir.path().join("Info.plist");

        let plist_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
</dict>
</plist>"#;

        std::fs::write(&plist_path, plist_content).unwrap();

        let adapter = NativeIosAdapter::new();
        let new_version = VersionInfo::new("2.5.0").with_build_number(99);
        adapter
            .update_info_plist_version(&plist_path, &new_version)
            .unwrap();

        let read_back = adapter.parse_info_plist_version(&plist_path).unwrap();
        assert_eq!(read_back.version, "2.5.0");
        assert_eq!(read_back.build_number, Some(99));
    }
}
