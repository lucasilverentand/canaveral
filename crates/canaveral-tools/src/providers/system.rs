//! Detect-only providers for system tools
//!
//! These tools (git, docker, rustc, xcodebuild, etc.) are managed by the OS
//! or by external package managers. Canaveral can detect their presence and
//! version but cannot install them — it will suggest the appropriate install
//! command instead.

use std::path::Path;

use async_trait::async_trait;
use regex::Regex;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::traits::{InstallResult, ToolProvider};
use crate::version_match::version_satisfies;

// ---------------------------------------------------------------------------
// SystemProvider
// ---------------------------------------------------------------------------

/// A configurable detect-only provider for system-managed tools.
///
/// System tools live on PATH and are installed via OS package managers,
/// dedicated installers, or app stores. Canaveral can detect their version
/// but delegates installation to the user with a helpful hint message.
pub struct SystemProvider {
    tool_id: &'static str,
    tool_name: &'static str,
    binary: &'static str,
    version_args: &'static [&'static str],
    version_regex: &'static str,
    install_hint: &'static str,
}

impl SystemProvider {
    /// Try to extract a version string from the combined stdout/stderr output
    /// of the configured binary using the provider's regex pattern.
    fn parse_version(&self, output: &std::process::Output) -> Option<String> {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        let re = match Regex::new(self.version_regex) {
            Ok(re) => re,
            Err(e) => {
                warn!(
                    tool = self.tool_id,
                    regex = self.version_regex,
                    error = %e,
                    "failed to compile version regex"
                );
                return None;
            }
        };

        // Try stdout first, then stderr (some tools output version to stderr)
        for text in [stdout.as_ref(), stderr.as_ref()] {
            if let Some(caps) = re.captures(text) {
                if let Some(m) = caps.get(1) {
                    return Some(m.as_str().to_string());
                }
            }
        }

        None
    }
}

#[async_trait]
impl ToolProvider for SystemProvider {
    fn id(&self) -> &'static str {
        self.tool_id
    }

    fn name(&self) -> &'static str {
        self.tool_name
    }

    fn binary_name(&self) -> &'static str {
        self.binary
    }

    async fn detect_version(&self) -> Result<Option<String>, ToolError> {
        // Fast check: is the binary on PATH at all?
        if which::which(self.binary).is_err() {
            debug!(
                tool = self.tool_id,
                binary = self.binary,
                "not found on PATH"
            );
            return Ok(None);
        }

        let output = tokio::process::Command::new(self.binary)
            .args(self.version_args)
            .output()
            .await;

        match output {
            Ok(ref out) if out.status.success() => {
                let version = self.parse_version(out);
                debug!(
                    tool = self.tool_id,
                    version = ?version,
                    "detected system tool version"
                );
                Ok(version)
            }
            Ok(ref out) => {
                // Some tools (e.g. xcrun) may return non-zero but still print version info
                let version = self.parse_version(out);
                if version.is_some() {
                    debug!(
                        tool = self.tool_id,
                        version = ?version,
                        "detected version from non-zero exit"
                    );
                    return Ok(version);
                }
                warn!(
                    tool = self.tool_id,
                    "version command returned non-zero exit status"
                );
                Ok(None)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!(tool = self.tool_id, "binary not found");
                Ok(None)
            }
            Err(e) => Err(ToolError::DetectionFailed(format!(
                "failed to run `{} {}`: {e}",
                self.binary,
                self.version_args.join(" ")
            ))),
        }
    }

    async fn is_satisfied(&self, requested: &str) -> Result<bool, ToolError> {
        match self.detect_version().await? {
            Some(installed) => Ok(version_satisfies(&installed, requested)),
            None => Ok(false),
        }
    }

    async fn install(&self, version: &str) -> Result<InstallResult, ToolError> {
        Err(ToolError::InstallFailed {
            tool: self.tool_id.into(),
            version: version.into(),
            reason: self.install_hint.to_string(),
        })
    }

    async fn install_to_cache(
        &self,
        version: &str,
        _cache_dir: &Path,
    ) -> Result<InstallResult, ToolError> {
        self.install(version).await
    }

    async fn list_available(&self) -> Result<Vec<String>, ToolError> {
        Ok(Vec::new())
    }

    fn env_vars(&self, _install_path: &Path) -> Vec<(String, String)> {
        Vec::new()
    }
}

// ---------------------------------------------------------------------------
// Factory functions
// ---------------------------------------------------------------------------

/// Git version control system.
pub fn git() -> SystemProvider {
    SystemProvider {
        tool_id: "git",
        tool_name: "Git",
        binary: "git",
        version_args: &["--version"],
        version_regex: r"git version (\d+\.\d+\.\d+)",
        install_hint: "Install Git from https://git-scm.com/",
    }
}

/// Docker container runtime.
pub fn docker() -> SystemProvider {
    SystemProvider {
        tool_id: "docker",
        tool_name: "Docker",
        binary: "docker",
        version_args: &["--version"],
        version_regex: r"Docker version (\d+\.\d+\.\d+)",
        install_hint: "Install Docker from https://docker.com/",
    }
}

/// Rust compiler.
pub fn rustc() -> SystemProvider {
    SystemProvider {
        tool_id: "rustc",
        tool_name: "Rust compiler",
        binary: "rustc",
        version_args: &["--version"],
        version_regex: r"rustc (\d+\.\d+\.\d+)",
        install_hint: "Install Rust via https://rustup.rs/",
    }
}

/// Cargo package manager and build tool.
pub fn cargo() -> SystemProvider {
    SystemProvider {
        tool_id: "cargo",
        tool_name: "Cargo",
        binary: "cargo",
        version_args: &["--version"],
        version_regex: r"cargo (\d+\.\d+\.\d+)",
        install_hint: "Install Rust via https://rustup.rs/",
    }
}

/// Xcode build system.
pub fn xcodebuild() -> SystemProvider {
    SystemProvider {
        tool_id: "xcodebuild",
        tool_name: "Xcode",
        binary: "xcodebuild",
        version_args: &["-version"],
        version_regex: r"Xcode (\d+\.\d+[\.\d]*)",
        install_hint: "Install Xcode from the Mac App Store or run `xcode-select --install`",
    }
}

/// Xcode command-line tool runner.
pub fn xcrun() -> SystemProvider {
    SystemProvider {
        tool_id: "xcrun",
        tool_name: "xcrun",
        binary: "xcrun",
        version_args: &["--version"],
        version_regex: r"xcrun version (\d+)",
        install_hint: "Run `xcode-select --install`",
    }
}

/// GnuPG for signing and verification.
pub fn gpg() -> SystemProvider {
    SystemProvider {
        tool_id: "gpg",
        tool_name: "GnuPG",
        binary: "gpg",
        version_args: &["--version"],
        version_regex: r"gpg \(GnuPG\) (\d+\.\d+\.\d+)",
        install_hint: "Install GnuPG from https://gnupg.org/",
    }
}

/// Android Debug Bridge.
pub fn adb() -> SystemProvider {
    SystemProvider {
        tool_id: "adb",
        tool_name: "Android Debug Bridge",
        binary: "adb",
        version_args: &["version"],
        version_regex: r"Android Debug Bridge version (\d+\.\d+\.\d+)",
        install_hint: "Install Android SDK Platform Tools",
    }
}

/// CocoaPods dependency manager.
pub fn pod() -> SystemProvider {
    SystemProvider {
        tool_id: "pod",
        tool_name: "CocoaPods",
        binary: "pod",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install CocoaPods with `gem install cocoapods`",
    }
}

/// npx package runner (bundled with Node.js).
pub fn npx() -> SystemProvider {
    SystemProvider {
        tool_id: "npx",
        tool_name: "npx",
        binary: "npx",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint:
            "npx is included with Node.js \u{2014} install Node via `canaveral tools install node`",
    }
}

/// Yarn package manager.
pub fn yarn() -> SystemProvider {
    SystemProvider {
        tool_id: "yarn",
        tool_name: "Yarn",
        binary: "yarn",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install Yarn with `npm install -g yarn` or `corepack enable`",
    }
}

/// Expo Application Services CLI.
pub fn eas() -> SystemProvider {
    SystemProvider {
        tool_id: "eas",
        tool_name: "EAS CLI",
        binary: "eas",
        version_args: &["--version"],
        version_regex: r"eas-cli/(\d+\.\d+\.\d+)",
        install_hint: "Install EAS CLI with `npm install -g eas-cli`",
    }
}

// ---------------------------------------------------------------------------
// TypeScript ecosystem
// ---------------------------------------------------------------------------

/// TypeScript compiler.
pub fn tsc() -> SystemProvider {
    SystemProvider {
        tool_id: "tsc",
        tool_name: "TypeScript compiler",
        binary: "tsc",
        version_args: &["--version"],
        version_regex: r"Version (\d+\.\d+\.\d+)",
        install_hint: "Install TypeScript with `npm install -g typescript`",
    }
}

/// Turbo monorepo build system.
pub fn turbo() -> SystemProvider {
    SystemProvider {
        tool_id: "turbo",
        tool_name: "Turborepo",
        binary: "turbo",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install Turbo with `npm install -g turbo`",
    }
}

/// esbuild JavaScript bundler.
pub fn esbuild() -> SystemProvider {
    SystemProvider {
        tool_id: "esbuild",
        tool_name: "esbuild",
        binary: "esbuild",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install esbuild with `npm install -g esbuild`",
    }
}

// ---------------------------------------------------------------------------
// iOS / Swift ecosystem
// ---------------------------------------------------------------------------

/// Swift compiler.
pub fn swift() -> SystemProvider {
    SystemProvider {
        tool_id: "swift",
        tool_name: "Swift",
        binary: "swift",
        version_args: &["--version"],
        version_regex: r"Swift version (\d+\.\d+[\.\d]*)",
        install_hint:
            "Install Xcode from the Mac App Store or download Swift from https://swift.org/",
    }
}

/// Xcode toolchain selector.
pub fn xcode_select() -> SystemProvider {
    SystemProvider {
        tool_id: "xcode-select",
        tool_name: "xcode-select",
        binary: "xcode-select",
        version_args: &["--version"],
        version_regex: r"xcode-select version (\d+)",
        install_hint: "Run `xcode-select --install`",
    }
}

// ---------------------------------------------------------------------------
// Expo / React Native ecosystem
// ---------------------------------------------------------------------------

/// Expo CLI.
pub fn expo() -> SystemProvider {
    SystemProvider {
        tool_id: "expo",
        tool_name: "Expo CLI",
        binary: "expo",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install Expo CLI with `npm install -g expo-cli` or use `npx expo`",
    }
}

// ---------------------------------------------------------------------------
// Android SDK & JDK tools
// ---------------------------------------------------------------------------

/// Java keytool for keystore management.
pub fn keytool() -> SystemProvider {
    SystemProvider {
        tool_id: "keytool",
        tool_name: "keytool",
        binary: "keytool",
        // keytool doesn't have a --version flag; its version matches the JDK
        version_args: &["-help"],
        version_regex: r"Key and Certificate Management Tool",
        install_hint: "keytool is included with the JDK \u{2014} install Java via `canaveral tools install java`",
    }
}

/// Android SDK Manager.
pub fn sdkmanager() -> SystemProvider {
    SystemProvider {
        tool_id: "sdkmanager",
        tool_name: "Android SDK Manager",
        binary: "sdkmanager",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+)",
        install_hint: "Install Android SDK command-line tools from https://developer.android.com/studio#command-tools \
                        and add $ANDROID_HOME/cmdline-tools/latest/bin to PATH",
    }
}

/// Android APK signer.
pub fn apksigner() -> SystemProvider {
    SystemProvider {
        tool_id: "apksigner",
        tool_name: "apksigner",
        binary: "apksigner",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+[\.\d]*)",
        install_hint: "apksigner is part of Android SDK build-tools \u{2014} install via \
                        `sdkmanager \"build-tools;35.0.0\"` and add $ANDROID_HOME/build-tools/<version> to PATH",
    }
}

/// Android APK alignment tool.
pub fn zipalign() -> SystemProvider {
    SystemProvider {
        tool_id: "zipalign",
        tool_name: "zipalign",
        binary: "zipalign",
        // zipalign has no version flag; just detect presence
        version_args: &["--help"],
        version_regex: r"Zip alignment",
        install_hint: "zipalign is part of Android SDK build-tools \u{2014} install via \
                        `sdkmanager \"build-tools;35.0.0\"` and add $ANDROID_HOME/build-tools/<version> to PATH",
    }
}

/// Android emulator.
pub fn emulator() -> SystemProvider {
    SystemProvider {
        tool_id: "emulator",
        tool_name: "Android Emulator",
        binary: "emulator",
        version_args: &["-version"],
        version_regex: r"Android emulator version (\d+\.\d+\.\d+[\.\d]*)",
        install_hint: "Install via `sdkmanager emulator` and add $ANDROID_HOME/emulator to PATH",
    }
}

/// Android Asset Packaging Tool.
pub fn aapt2() -> SystemProvider {
    SystemProvider {
        tool_id: "aapt2",
        tool_name: "aapt2",
        binary: "aapt2",
        version_args: &["version"],
        version_regex: r"(\d+\.\d+[\.\d]*-\d+)",
        install_hint: "aapt2 is part of Android SDK build-tools \u{2014} install via \
                        `sdkmanager \"build-tools;35.0.0\"` and add $ANDROID_HOME/build-tools/<version> to PATH",
    }
}

/// Android Virtual Device manager.
pub fn avdmanager() -> SystemProvider {
    SystemProvider {
        tool_id: "avdmanager",
        tool_name: "avdmanager",
        binary: "avdmanager",
        version_args: &["--version"],
        version_regex: r"(\d+\.\d+[\.\d]*)",
        install_hint: "Install Android SDK command-line tools from https://developer.android.com/studio#command-tools \
                        and add $ANDROID_HOME/cmdline-tools/latest/bin to PATH",
    }
}

/// Bundletool for Android App Bundles.
pub fn bundletool() -> SystemProvider {
    SystemProvider {
        tool_id: "bundletool",
        tool_name: "bundletool",
        binary: "bundletool",
        version_args: &["version"],
        version_regex: r"(\d+\.\d+\.\d+)",
        install_hint: "Install bundletool via `brew install bundletool` or download from \
                        https://github.com/google/bundletool/releases",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // -- Factory identity tests -----------------------------------------------

    #[test]
    fn git_provider_identity() {
        let p = git();
        assert_eq!(p.id(), "git");
        assert_eq!(p.name(), "Git");
        assert_eq!(p.binary_name(), "git");
    }

    #[test]
    fn docker_provider_identity() {
        let p = docker();
        assert_eq!(p.id(), "docker");
        assert_eq!(p.name(), "Docker");
        assert_eq!(p.binary_name(), "docker");
    }

    #[test]
    fn rustc_provider_identity() {
        let p = rustc();
        assert_eq!(p.id(), "rustc");
        assert_eq!(p.name(), "Rust compiler");
        assert_eq!(p.binary_name(), "rustc");
    }

    #[test]
    fn cargo_provider_identity() {
        let p = cargo();
        assert_eq!(p.id(), "cargo");
        assert_eq!(p.name(), "Cargo");
        assert_eq!(p.binary_name(), "cargo");
    }

    #[test]
    fn xcodebuild_provider_identity() {
        let p = xcodebuild();
        assert_eq!(p.id(), "xcodebuild");
        assert_eq!(p.name(), "Xcode");
        assert_eq!(p.binary_name(), "xcodebuild");
    }

    #[test]
    fn xcrun_provider_identity() {
        let p = xcrun();
        assert_eq!(p.id(), "xcrun");
        assert_eq!(p.name(), "xcrun");
        assert_eq!(p.binary_name(), "xcrun");
    }

    #[test]
    fn gpg_provider_identity() {
        let p = gpg();
        assert_eq!(p.id(), "gpg");
        assert_eq!(p.name(), "GnuPG");
        assert_eq!(p.binary_name(), "gpg");
    }

    #[test]
    fn adb_provider_identity() {
        let p = adb();
        assert_eq!(p.id(), "adb");
        assert_eq!(p.name(), "Android Debug Bridge");
        assert_eq!(p.binary_name(), "adb");
    }

    #[test]
    fn pod_provider_identity() {
        let p = pod();
        assert_eq!(p.id(), "pod");
        assert_eq!(p.name(), "CocoaPods");
        assert_eq!(p.binary_name(), "pod");
    }

    #[test]
    fn npx_provider_identity() {
        let p = npx();
        assert_eq!(p.id(), "npx");
        assert_eq!(p.name(), "npx");
        assert_eq!(p.binary_name(), "npx");
    }

    #[test]
    fn yarn_provider_identity() {
        let p = yarn();
        assert_eq!(p.id(), "yarn");
        assert_eq!(p.name(), "Yarn");
        assert_eq!(p.binary_name(), "yarn");
    }

    #[test]
    fn eas_provider_identity() {
        let p = eas();
        assert_eq!(p.id(), "eas");
        assert_eq!(p.name(), "EAS CLI");
        assert_eq!(p.binary_name(), "eas");
    }

    #[test]
    fn tsc_provider_identity() {
        let p = tsc();
        assert_eq!(p.id(), "tsc");
        assert_eq!(p.name(), "TypeScript compiler");
        assert_eq!(p.binary_name(), "tsc");
    }

    #[test]
    fn turbo_provider_identity() {
        let p = turbo();
        assert_eq!(p.id(), "turbo");
        assert_eq!(p.name(), "Turborepo");
        assert_eq!(p.binary_name(), "turbo");
    }

    #[test]
    fn esbuild_provider_identity() {
        let p = esbuild();
        assert_eq!(p.id(), "esbuild");
        assert_eq!(p.name(), "esbuild");
        assert_eq!(p.binary_name(), "esbuild");
    }

    #[test]
    fn swift_provider_identity() {
        let p = swift();
        assert_eq!(p.id(), "swift");
        assert_eq!(p.name(), "Swift");
        assert_eq!(p.binary_name(), "swift");
    }

    #[test]
    fn xcode_select_provider_identity() {
        let p = xcode_select();
        assert_eq!(p.id(), "xcode-select");
        assert_eq!(p.name(), "xcode-select");
        assert_eq!(p.binary_name(), "xcode-select");
    }

    #[test]
    fn expo_provider_identity() {
        let p = expo();
        assert_eq!(p.id(), "expo");
        assert_eq!(p.name(), "Expo CLI");
        assert_eq!(p.binary_name(), "expo");
    }

    #[test]
    fn keytool_provider_identity() {
        let p = keytool();
        assert_eq!(p.id(), "keytool");
        assert_eq!(p.name(), "keytool");
        assert_eq!(p.binary_name(), "keytool");
    }

    #[test]
    fn sdkmanager_provider_identity() {
        let p = sdkmanager();
        assert_eq!(p.id(), "sdkmanager");
        assert_eq!(p.name(), "Android SDK Manager");
        assert_eq!(p.binary_name(), "sdkmanager");
    }

    #[test]
    fn apksigner_provider_identity() {
        let p = apksigner();
        assert_eq!(p.id(), "apksigner");
        assert_eq!(p.name(), "apksigner");
        assert_eq!(p.binary_name(), "apksigner");
    }

    #[test]
    fn zipalign_provider_identity() {
        let p = zipalign();
        assert_eq!(p.id(), "zipalign");
        assert_eq!(p.name(), "zipalign");
        assert_eq!(p.binary_name(), "zipalign");
    }

    #[test]
    fn emulator_provider_identity() {
        let p = emulator();
        assert_eq!(p.id(), "emulator");
        assert_eq!(p.name(), "Android Emulator");
        assert_eq!(p.binary_name(), "emulator");
    }

    #[test]
    fn aapt2_provider_identity() {
        let p = aapt2();
        assert_eq!(p.id(), "aapt2");
        assert_eq!(p.name(), "aapt2");
        assert_eq!(p.binary_name(), "aapt2");
    }

    #[test]
    fn avdmanager_provider_identity() {
        let p = avdmanager();
        assert_eq!(p.id(), "avdmanager");
        assert_eq!(p.name(), "avdmanager");
        assert_eq!(p.binary_name(), "avdmanager");
    }

    #[test]
    fn bundletool_provider_identity() {
        let p = bundletool();
        assert_eq!(p.id(), "bundletool");
        assert_eq!(p.name(), "bundletool");
        assert_eq!(p.binary_name(), "bundletool");
    }

    // -- Version regex tests --------------------------------------------------

    fn make_output(stdout: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn make_output_stderr(stderr: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::default(),
            stdout: Vec::new(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    #[test]
    fn git_version_regex_matches() {
        let p = git();
        let out = make_output("git version 2.43.0\n");
        assert_eq!(p.parse_version(&out), Some("2.43.0".to_string()));
    }

    #[test]
    fn git_version_regex_with_extra_info() {
        let p = git();
        let out = make_output("git version 2.43.0 (Apple Git-146)\n");
        assert_eq!(p.parse_version(&out), Some("2.43.0".to_string()));
    }

    #[test]
    fn docker_version_regex_matches() {
        let p = docker();
        let out = make_output("Docker version 24.0.7, build afdd53b\n");
        assert_eq!(p.parse_version(&out), Some("24.0.7".to_string()));
    }

    #[test]
    fn rustc_version_regex_matches() {
        let p = rustc();
        let out = make_output("rustc 1.75.0 (82e1608df 2023-12-21)\n");
        assert_eq!(p.parse_version(&out), Some("1.75.0".to_string()));
    }

    #[test]
    fn cargo_version_regex_matches() {
        let p = cargo();
        let out = make_output("cargo 1.75.0 (1d8b05cdd 2023-11-20)\n");
        assert_eq!(p.parse_version(&out), Some("1.75.0".to_string()));
    }

    #[test]
    fn xcodebuild_version_regex_matches() {
        let p = xcodebuild();
        let out = make_output("Xcode 15.2\nBuild version 15C500b\n");
        assert_eq!(p.parse_version(&out), Some("15.2".to_string()));
    }

    #[test]
    fn xcodebuild_version_regex_with_patch() {
        let p = xcodebuild();
        let out = make_output("Xcode 16.0.1\nBuild version 16A242d\n");
        assert_eq!(p.parse_version(&out), Some("16.0.1".to_string()));
    }

    #[test]
    fn xcrun_version_regex_matches() {
        let p = xcrun();
        let out = make_output("xcrun version 76\n");
        assert_eq!(p.parse_version(&out), Some("76".to_string()));
    }

    #[test]
    fn gpg_version_regex_matches() {
        let p = gpg();
        let out = make_output("gpg (GnuPG) 2.4.3\nlibgcrypt 1.10.2\n");
        assert_eq!(p.parse_version(&out), Some("2.4.3".to_string()));
    }

    #[test]
    fn adb_version_regex_matches() {
        let p = adb();
        let out = make_output(
            "Android Debug Bridge version 1.0.41\nVersion 34.0.5-10900879\nInstalled as /usr/bin/adb\n",
        );
        assert_eq!(p.parse_version(&out), Some("1.0.41".to_string()));
    }

    #[test]
    fn pod_version_regex_matches() {
        let p = pod();
        let out = make_output("1.15.2\n");
        assert_eq!(p.parse_version(&out), Some("1.15.2".to_string()));
    }

    #[test]
    fn npx_version_regex_matches() {
        let p = npx();
        let out = make_output("10.2.4\n");
        assert_eq!(p.parse_version(&out), Some("10.2.4".to_string()));
    }

    #[test]
    fn yarn_version_regex_matches() {
        let p = yarn();
        let out = make_output("1.22.19\n");
        assert_eq!(p.parse_version(&out), Some("1.22.19".to_string()));
    }

    #[test]
    fn yarn_v4_version_regex_matches() {
        let p = yarn();
        let out = make_output("4.1.0\n");
        assert_eq!(p.parse_version(&out), Some("4.1.0".to_string()));
    }

    #[test]
    fn eas_version_regex_matches() {
        let p = eas();
        let out = make_output("eas-cli/12.6.2 darwin-arm64 node-v22.14.0\n");
        assert_eq!(p.parse_version(&out), Some("12.6.2".to_string()));
    }

    #[test]
    fn tsc_version_regex_matches() {
        let p = tsc();
        let out = make_output("Version 5.7.2\n");
        assert_eq!(p.parse_version(&out), Some("5.7.2".to_string()));
    }

    #[test]
    fn turbo_version_regex_matches() {
        let p = turbo();
        let out = make_output("2.3.4\n");
        assert_eq!(p.parse_version(&out), Some("2.3.4".to_string()));
    }

    #[test]
    fn esbuild_version_regex_matches() {
        let p = esbuild();
        let out = make_output("0.24.0\n");
        assert_eq!(p.parse_version(&out), Some("0.24.0".to_string()));
    }

    #[test]
    fn swift_version_regex_matches() {
        let p = swift();
        let out = make_output(
            "Swift version 6.0.3 (swift-6.0.3-RELEASE)\nTarget: arm64-apple-macosx15.0\n",
        );
        assert_eq!(p.parse_version(&out), Some("6.0.3".to_string()));
    }

    #[test]
    fn swift_version_regex_with_two_part_version() {
        let p = swift();
        let out = make_output("Swift version 6.0 (swift-6.0-RELEASE)\n");
        assert_eq!(p.parse_version(&out), Some("6.0".to_string()));
    }

    #[test]
    fn xcode_select_version_regex_matches() {
        let p = xcode_select();
        let out = make_output("xcode-select version 2408.\n");
        assert_eq!(p.parse_version(&out), Some("2408".to_string()));
    }

    #[test]
    fn expo_version_regex_matches() {
        let p = expo();
        let out = make_output("6.4.0\n");
        assert_eq!(p.parse_version(&out), Some("6.4.0".to_string()));
    }

    #[test]
    fn sdkmanager_version_regex_matches() {
        let p = sdkmanager();
        let out = make_output("12.0\n");
        assert_eq!(p.parse_version(&out), Some("12.0".to_string()));
    }

    #[test]
    fn apksigner_version_regex_matches() {
        let p = apksigner();
        let out = make_output("0.9\n");
        assert_eq!(p.parse_version(&out), Some("0.9".to_string()));
    }

    #[test]
    fn emulator_version_regex_matches() {
        let p = emulator();
        let out = make_output("Android emulator version 34.2.16.0 (build_id 12345678) (CL:N/A)\n");
        assert_eq!(p.parse_version(&out), Some("34.2.16.0".to_string()));
    }

    #[test]
    fn aapt2_version_regex_matches() {
        let p = aapt2();
        let out =
            make_output("Android Asset Packaging Tool (aapt) 2.19-11797org (build 12345678)\n");
        assert_eq!(p.parse_version(&out), Some("2.19-11797".to_string()));
    }

    #[test]
    fn bundletool_version_regex_matches() {
        let p = bundletool();
        let out = make_output("1.18.3\n");
        assert_eq!(p.parse_version(&out), Some("1.18.3".to_string()));
    }

    // -- stderr fallback test -------------------------------------------------

    #[test]
    fn version_detected_from_stderr() {
        let p = gpg();
        let out = make_output_stderr("gpg (GnuPG) 2.4.3\nlibgcrypt 1.10.2\n");
        assert_eq!(p.parse_version(&out), Some("2.4.3".to_string()));
    }

    // -- no match tests -------------------------------------------------------

    #[test]
    fn parse_version_returns_none_on_garbage() {
        let p = git();
        let out = make_output("totally unrelated output\n");
        assert_eq!(p.parse_version(&out), None);
    }

    #[test]
    fn parse_version_returns_none_on_empty() {
        let p = docker();
        let out = make_output("");
        assert_eq!(p.parse_version(&out), None);
    }

    // -- install error tests --------------------------------------------------

    #[tokio::test]
    async fn install_returns_error_with_hint() {
        let p = git();
        let result = p.install("2.43").await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("https://git-scm.com/"),
            "error should contain install hint: {err}"
        );
    }

    #[tokio::test]
    async fn install_to_cache_returns_same_error() {
        let p = docker();
        let result = p.install_to_cache("24", Path::new("/tmp/cache")).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("https://docker.com/"),
            "error should contain install hint: {err}"
        );
    }

    #[tokio::test]
    async fn rustc_install_hint_mentions_rustup() {
        let p = rustc();
        let result = p.install("1.75").await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("rustup.rs"),
            "error should mention rustup: {err}"
        );
    }

    #[tokio::test]
    async fn npx_install_hint_mentions_node() {
        let p = npx();
        let result = p.install("10").await;
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("canaveral tools install node"),
            "error should mention canaveral: {err}"
        );
    }

    // -- list_available tests -------------------------------------------------

    #[tokio::test]
    async fn list_available_returns_empty() {
        let p = git();
        let result = p.list_available().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_available_empty_for_all_providers() {
        let providers: Vec<SystemProvider> = vec![
            git(),
            docker(),
            rustc(),
            cargo(),
            xcodebuild(),
            xcrun(),
            gpg(),
            adb(),
            pod(),
            npx(),
            yarn(),
            eas(),
            tsc(),
            turbo(),
            esbuild(),
            swift(),
            xcode_select(),
            expo(),
            keytool(),
            sdkmanager(),
            apksigner(),
            zipalign(),
            emulator(),
            aapt2(),
            avdmanager(),
            bundletool(),
        ];
        for p in providers {
            let result = p.list_available().await.unwrap();
            assert!(
                result.is_empty(),
                "{} should have no available versions",
                p.tool_id
            );
        }
    }

    // -- env_vars tests -------------------------------------------------------

    #[test]
    fn env_vars_returns_empty() {
        let p = git();
        let vars = p.env_vars(Path::new("/usr/bin"));
        assert!(vars.is_empty());
    }

    #[test]
    fn env_vars_empty_for_all_providers() {
        let providers: Vec<SystemProvider> = vec![
            git(),
            docker(),
            rustc(),
            cargo(),
            xcodebuild(),
            xcrun(),
            gpg(),
            adb(),
            pod(),
            npx(),
            yarn(),
            eas(),
            tsc(),
            turbo(),
            esbuild(),
            swift(),
            xcode_select(),
            expo(),
            keytool(),
            sdkmanager(),
            apksigner(),
            zipalign(),
            emulator(),
            aapt2(),
            avdmanager(),
            bundletool(),
        ];
        for p in providers {
            let vars = p.env_vars(Path::new("/usr/bin"));
            assert!(vars.is_empty(), "{} should have no env vars", p.tool_id);
        }
    }

    // -- detect_version with missing binary -----------------------------------

    #[tokio::test]
    async fn detect_version_returns_none_for_missing_binary() {
        // Use a binary name that definitely does not exist
        let p = SystemProvider {
            tool_id: "nonexistent",
            tool_name: "Nonexistent Tool",
            binary: "canaveral_nonexistent_tool_xyz_999",
            version_args: &["--version"],
            version_regex: r"(\d+\.\d+\.\d+)",
            install_hint: "This tool does not exist",
        };
        let result = p.detect_version().await.unwrap();
        assert!(result.is_none());
    }
}
