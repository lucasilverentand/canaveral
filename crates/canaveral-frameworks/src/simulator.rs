//! iOS Simulator management via `xcrun simctl`
//!
//! Provides a high-level async API for managing iOS Simulator devices:
//! listing runtimes, device types, and devices; creating/booting/shutting down
//! simulators; capturing screenshots and video; installing and launching apps;
//! and configuring appearance, locale, and status bar overrides.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, instrument, warn};

use crate::error::{FrameworkError, Result};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// High-level manager for iOS Simulator operations.
///
/// All methods are stateless and use `xcrun simctl` under the hood.
pub struct SimulatorManager;

/// Information about an available simulator runtime.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimRuntime {
    /// Full identifier, e.g. `com.apple.CoreSimulator.SimRuntime.iOS-18-0`
    pub identifier: String,
    /// Human-readable name, e.g. `iOS 18.0`
    pub name: String,
    /// Platform kind: `iOS`, `watchOS`, `tvOS`, `visionOS`
    pub platform: String,
    /// Version string, e.g. `18.0`
    pub version: String,
    /// Whether this runtime is usable
    pub is_available: bool,
}

/// Information about a simulator device type.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimDeviceType {
    /// Full identifier, e.g. `com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro`
    pub identifier: String,
    /// Human-readable name, e.g. `iPhone 16 Pro`
    pub name: String,
    /// Minimum supported runtime version
    pub min_runtime: Option<String>,
    /// Maximum supported runtime version
    pub max_runtime: Option<String>,
}

/// A simulator device instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimDevice {
    /// Device UDID
    pub udid: String,
    /// Device name, e.g. `iPhone 16 Pro`
    pub name: String,
    /// Device type identifier
    pub device_type: String,
    /// Runtime identifier this device belongs to
    pub runtime: String,
    /// Current device state
    pub state: SimDeviceState,
}

/// Simulator device lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SimDeviceState {
    Shutdown,
    Booted,
    Creating,
    ShuttingDown,
}

impl SimDeviceState {
    fn from_str(s: &str) -> Self {
        match s {
            "Booted" => Self::Booted,
            "Creating" => Self::Creating,
            "ShuttingDown" => Self::ShuttingDown,
            _ => Self::Shutdown,
        }
    }
}

impl std::fmt::Display for SimDeviceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shutdown => write!(f, "Shutdown"),
            Self::Booted => write!(f, "Booted"),
            Self::Creating => write!(f, "Creating"),
            Self::ShuttingDown => write!(f, "ShuttingDown"),
        }
    }
}

/// Simulator appearance mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

impl std::fmt::Display for Appearance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Status bar overrides for clean screenshots.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatusBarOverrides {
    /// Time string, e.g. `"9:41"`
    pub time: Option<String>,
    /// Battery level 0-100
    pub battery_level: Option<u8>,
    /// Battery state: `"charged"`, `"charging"`, `"discharging"`
    pub battery_state: Option<String>,
    /// Cellular signal bars 0-4
    pub cellular_bars: Option<u8>,
    /// Wi-Fi signal bars 0-3
    pub wifi_bars: Option<u8>,
    /// Cellular mode string, e.g. `"active"`, `"searching"`, `"notSupported"`
    pub cellular_mode: Option<String>,
}

impl StatusBarOverrides {
    /// A clean status bar suitable for App Store screenshots.
    pub fn clean() -> Self {
        Self {
            time: Some("9:41".to_string()),
            battery_level: Some(100),
            battery_state: Some("charged".to_string()),
            cellular_bars: Some(4),
            wifi_bars: Some(3),
            cellular_mode: Some("active".to_string()),
        }
    }
}

/// Handle to an in-progress video recording.
///
/// Call [`RecordingHandle::stop`] to terminate the recording and get the
/// output path.
pub struct RecordingHandle {
    child: tokio::process::Child,
    output_path: PathBuf,
}

impl RecordingHandle {
    /// Stop the recording and return the path to the video file.
    ///
    /// On Unix, sends SIGINT to the `simctl` process so it can finalize the
    /// video file cleanly.  On other platforms, kills the process.
    pub async fn stop(mut self) -> Result<PathBuf> {
        #[cfg(unix)]
        {
            if let Some(id) = self.child.id() {
                // simctl recordVideo listens for SIGINT to stop gracefully.
                // SAFETY: we are sending a standard signal to a child process
                // that we own.
                unsafe {
                    libc::kill(id as libc::pid_t, libc::SIGINT);
                }
            }
        }

        #[cfg(not(unix))]
        {
            let _ = self.child.kill().await;
        }

        let status = self
            .child
            .wait()
            .await
            .map_err(|e| FrameworkError::context("stop recording", e.to_string()))?;

        // simctl exits with 0 on SIGINT when it finishes writing the file
        debug!(status = ?status, path = %self.output_path.display(), "recording stopped");

        Ok(self.output_path)
    }
}

// ---------------------------------------------------------------------------
// Raw simctl JSON shapes (private)
// ---------------------------------------------------------------------------

/// Top-level wrapper returned by `simctl list runtimes -j`
#[derive(Deserialize)]
struct SimctlRuntimes {
    runtimes: Vec<RawRuntime>,
}

#[derive(Deserialize)]
struct RawRuntime {
    identifier: String,
    name: String,
    platform: Option<String>,
    version: Option<String>,
    #[serde(default, alias = "isAvailable")]
    is_available: bool,
}

/// Top-level wrapper returned by `simctl list devicetypes -j`
#[derive(Deserialize)]
struct SimctlDeviceTypes {
    #[serde(alias = "devicetypes")]
    device_types: Vec<RawDeviceType>,
}

#[derive(Deserialize)]
struct RawDeviceType {
    identifier: String,
    name: String,
    #[serde(alias = "minRuntimeVersion")]
    min_runtime_version: Option<u64>,
    #[serde(alias = "maxRuntimeVersion")]
    max_runtime_version: Option<u64>,
    #[serde(alias = "minRuntimeVersionString")]
    min_runtime_version_string: Option<String>,
    #[serde(alias = "maxRuntimeVersionString")]
    max_runtime_version_string: Option<String>,
}

/// Top-level wrapper returned by `simctl list devices -j`
#[derive(Deserialize)]
struct SimctlDevices {
    devices: std::collections::HashMap<String, Vec<RawDevice>>,
}

#[derive(Deserialize)]
struct RawDevice {
    udid: String,
    name: String,
    state: String,
    #[serde(default, alias = "isAvailable")]
    is_available: bool,
    #[serde(default, alias = "deviceTypeIdentifier")]
    device_type_identifier: Option<String>,
}

// ---------------------------------------------------------------------------
// Helper: run a command and return stdout or a FrameworkError
// ---------------------------------------------------------------------------

async fn run_simctl(args: &[&str]) -> Result<Vec<u8>> {
    let mut full_args = vec!["simctl"];
    full_args.extend_from_slice(args);

    let cmd_display = format!("xcrun {}", full_args.join(" "));
    debug!(command = %cmd_display, "running simctl command");

    let output = tokio::process::Command::new("xcrun")
        .args(&full_args)
        .output()
        .await
        .map_err(|e| FrameworkError::CommandFailed {
            command: cmd_display.clone(),
            exit_code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(FrameworkError::CommandFailed {
            command: cmd_display,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(output.stdout)
}

/// Like `run_simctl` but ignores specific known-benign error messages.
async fn run_simctl_tolerant(args: &[&str], tolerate: &[&str]) -> Result<Vec<u8>> {
    let mut full_args = vec!["simctl"];
    full_args.extend_from_slice(args);

    let cmd_display = format!("xcrun {}", full_args.join(" "));
    debug!(command = %cmd_display, "running simctl command (tolerant)");

    let output = tokio::process::Command::new("xcrun")
        .args(&full_args)
        .output()
        .await
        .map_err(|e| FrameworkError::CommandFailed {
            command: cmd_display.clone(),
            exit_code: None,
            stdout: String::new(),
            stderr: e.to_string(),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if tolerate.iter().any(|msg| stderr.contains(msg)) {
            debug!(stderr = %stderr, "ignoring tolerated simctl error");
            return Ok(output.stdout);
        }
        return Err(FrameworkError::CommandFailed {
            command: cmd_display,
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: stderr.to_string(),
        });
    }

    Ok(output.stdout)
}

// ---------------------------------------------------------------------------
// Parsing helpers
// ---------------------------------------------------------------------------

/// Extract the platform name from a runtime identifier.
/// e.g. `com.apple.CoreSimulator.SimRuntime.iOS-18-0` -> `"iOS"`
fn platform_from_identifier(id: &str) -> String {
    // The identifier format is: ...SimRuntime.<Platform>-<Major>-<Minor>
    if let Some(last) = id.rsplit('.').next() {
        // last = "iOS-18-0"
        if let Some(idx) = last.find('-') {
            return last[..idx].to_string();
        }
        return last.to_string();
    }
    "Unknown".to_string()
}

/// Extract the version from a runtime identifier.
/// e.g. `com.apple.CoreSimulator.SimRuntime.iOS-18-0` -> `"18.0"`
fn version_from_identifier(id: &str) -> String {
    if let Some(last) = id.rsplit('.').next() {
        if let Some(idx) = last.find('-') {
            return last[idx + 1..].replace('-', ".");
        }
    }
    String::new()
}

/// Extract a human-readable runtime name from the runtime key in the devices
/// dict.  Keys can be either the full identifier
/// (`com.apple.CoreSimulator.SimRuntime.iOS-18-0`) or a display name like
/// `"iOS 18.0"`.
fn runtime_display_name(key: &str) -> String {
    if key.starts_with("com.apple.") {
        let platform = platform_from_identifier(key);
        let version = version_from_identifier(key);
        format!("{} {}", platform, version)
    } else {
        key.to_string()
    }
}

// ---------------------------------------------------------------------------
// SimulatorManager implementation
// ---------------------------------------------------------------------------

impl SimulatorManager {
    // ----- Listing -----------------------------------------------------------

    /// List available simulator runtimes.
    #[instrument]
    pub async fn list_runtimes() -> Result<Vec<SimRuntime>> {
        let stdout = run_simctl(&["list", "runtimes", "-j"]).await?;
        let parsed: SimctlRuntimes = serde_json::from_slice(&stdout)
            .map_err(|e| FrameworkError::context("parse simctl runtimes JSON", e.to_string()))?;

        let runtimes = parsed
            .runtimes
            .into_iter()
            .map(|r| {
                let platform = r
                    .platform
                    .unwrap_or_else(|| platform_from_identifier(&r.identifier));
                let version = r
                    .version
                    .unwrap_or_else(|| version_from_identifier(&r.identifier));
                SimRuntime {
                    identifier: r.identifier,
                    name: r.name,
                    platform,
                    version,
                    is_available: r.is_available,
                }
            })
            .collect();

        Ok(runtimes)
    }

    /// List available device types.
    #[instrument]
    pub async fn list_device_types() -> Result<Vec<SimDeviceType>> {
        let stdout = run_simctl(&["list", "devicetypes", "-j"]).await?;
        let parsed: SimctlDeviceTypes = serde_json::from_slice(&stdout)
            .map_err(|e| FrameworkError::context("parse simctl devicetypes JSON", e.to_string()))?;

        let types = parsed
            .device_types
            .into_iter()
            .map(|dt| SimDeviceType {
                identifier: dt.identifier,
                name: dt.name,
                min_runtime: dt
                    .min_runtime_version_string
                    .or_else(|| dt.min_runtime_version.map(|v| v.to_string())),
                max_runtime: dt
                    .max_runtime_version_string
                    .or_else(|| dt.max_runtime_version.map(|v| v.to_string())),
            })
            .collect();

        Ok(types)
    }

    /// List all simulator device instances.
    #[instrument]
    pub async fn list_devices() -> Result<Vec<SimDevice>> {
        let stdout = run_simctl(&["list", "devices", "-j"]).await?;
        Self::parse_devices_json(&stdout)
    }

    /// List only booted simulator devices.
    #[instrument]
    pub async fn list_booted() -> Result<Vec<SimDevice>> {
        let all = Self::list_devices().await?;
        Ok(all
            .into_iter()
            .filter(|d| d.state == SimDeviceState::Booted)
            .collect())
    }

    /// Find a device by name and optional runtime filter.
    ///
    /// `runtime` can be a substring like `"iOS 18"` — it is matched
    /// case-insensitively against the runtime key.
    #[instrument]
    pub async fn find_device(name: &str, runtime: Option<&str>) -> Result<Option<SimDevice>> {
        let all = Self::list_devices().await?;
        let name_lower = name.to_lowercase();

        Ok(all.into_iter().find(|d| {
            let name_matches = d.name.to_lowercase() == name_lower;
            let runtime_matches = match runtime {
                Some(rt) => {
                    let rt_lower = rt.to_lowercase();
                    let display = runtime_display_name(&d.runtime).to_lowercase();
                    display.contains(&rt_lower) || d.runtime.to_lowercase().contains(&rt_lower)
                }
                None => true,
            };
            name_matches && runtime_matches
        }))
    }

    // ----- Lifecycle ---------------------------------------------------------

    /// Create a new simulator device.
    ///
    /// `device_type` is a device type identifier like
    /// `com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro` and `runtime` is
    /// a runtime identifier like
    /// `com.apple.CoreSimulator.SimRuntime.iOS-18-0`.
    #[instrument]
    pub async fn create(name: &str, device_type: &str, runtime: &str) -> Result<SimDevice> {
        let stdout = run_simctl(&["create", name, device_type, runtime]).await?;
        let udid = String::from_utf8_lossy(&stdout).trim().to_string();

        info!(udid = %udid, name = %name, "created simulator");

        Ok(SimDevice {
            udid,
            name: name.to_string(),
            device_type: device_type.to_string(),
            runtime: runtime.to_string(),
            state: SimDeviceState::Shutdown,
        })
    }

    /// Boot a simulator.
    #[instrument]
    pub async fn boot(udid: &str) -> Result<()> {
        run_simctl_tolerant(
            &["boot", udid],
            &["Unable to boot device in current state: Booted"],
        )
        .await?;
        info!(udid = %udid, "simulator booted");

        // Give SpringBoard time to launch
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        Ok(())
    }

    /// Shutdown a simulator.
    #[instrument]
    pub async fn shutdown(udid: &str) -> Result<()> {
        run_simctl_tolerant(
            &["shutdown", udid],
            &["Unable to shutdown device in current state: Shutdown"],
        )
        .await?;
        info!(udid = %udid, "simulator shut down");
        Ok(())
    }

    /// Shutdown all booted simulators.
    #[instrument]
    pub async fn shutdown_all() -> Result<()> {
        run_simctl(&["shutdown", "all"]).await?;
        info!("all simulators shut down");
        Ok(())
    }

    /// Erase (reset) a simulator to a clean state.
    #[instrument]
    pub async fn erase(udid: &str) -> Result<()> {
        run_simctl(&["erase", udid]).await?;
        info!(udid = %udid, "simulator erased");
        Ok(())
    }

    /// Delete a simulator device.
    #[instrument]
    pub async fn delete(udid: &str) -> Result<()> {
        run_simctl(&["delete", udid]).await?;
        info!(udid = %udid, "simulator deleted");
        Ok(())
    }

    // ----- I/O ---------------------------------------------------------------

    /// Take a screenshot from a booted simulator.
    #[instrument]
    pub async fn screenshot(udid: &str, output_path: &Path) -> Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                FrameworkError::context("create screenshot output directory", e.to_string())
            })?;
        }

        let path_str = output_path.to_string_lossy();
        run_simctl(&["io", udid, "screenshot", "--type=png", &path_str]).await?;

        debug!(udid = %udid, path = %path_str, "screenshot captured");
        Ok(())
    }

    /// Start recording video from a booted simulator.
    ///
    /// Returns a [`RecordingHandle`] — call `.stop()` to finish the recording.
    #[instrument]
    pub async fn start_recording(udid: &str, output_path: &Path) -> Result<RecordingHandle> {
        if let Some(parent) = output_path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                FrameworkError::context("create recording output directory", e.to_string())
            })?;
        }

        let path_str = output_path.to_string_lossy().to_string();

        let child = tokio::process::Command::new("xcrun")
            .args(["simctl", "io", udid, "recordVideo", &path_str])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcrun simctl io {} recordVideo {}", udid, path_str),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        info!(udid = %udid, path = %path_str, "video recording started");

        Ok(RecordingHandle {
            child,
            output_path: output_path.to_path_buf(),
        })
    }

    // ----- App management ----------------------------------------------------

    /// Install an app bundle on the simulator.
    #[instrument]
    pub async fn install_app(udid: &str, app_path: &Path) -> Result<()> {
        let path_str = app_path.to_string_lossy();
        run_simctl(&["install", udid, &path_str]).await?;
        info!(udid = %udid, app = %path_str, "app installed");
        Ok(())
    }

    /// Launch an app on the simulator by bundle identifier.
    #[instrument]
    pub async fn launch_app(udid: &str, bundle_id: &str) -> Result<()> {
        run_simctl(&["launch", udid, bundle_id]).await?;
        info!(udid = %udid, bundle_id = %bundle_id, "app launched");
        Ok(())
    }

    /// Terminate a running app on the simulator.
    #[instrument]
    pub async fn terminate_app(udid: &str, bundle_id: &str) -> Result<()> {
        run_simctl_tolerant(&["terminate", udid, bundle_id], &["not running"]).await?;
        debug!(udid = %udid, bundle_id = %bundle_id, "app terminated");
        Ok(())
    }

    // ----- Configuration -----------------------------------------------------

    /// Set simulator appearance (light / dark).
    #[instrument]
    pub async fn set_appearance(udid: &str, appearance: Appearance) -> Result<()> {
        run_simctl(&["ui", udid, "appearance", appearance.as_str()]).await?;
        debug!(udid = %udid, appearance = %appearance, "appearance set");
        Ok(())
    }

    /// Set the simulator locale by writing to `NSGlobalDomain` preferences.
    ///
    /// This modifies the simulator's plist; a reboot is required for the
    /// change to take effect.
    #[instrument]
    pub async fn set_locale(udid: &str, locale: &str) -> Result<()> {
        run_simctl(&[
            "spawn",
            udid,
            "defaults",
            "write",
            "NSGlobalDomain",
            "AppleLocale",
            "-string",
            locale,
        ])
        .await?;
        debug!(udid = %udid, locale = %locale, "locale set");
        Ok(())
    }

    /// Set the simulator language.
    ///
    /// `language` should be a BCP-47 language tag like `"en"` or `"de-DE"`.
    /// A reboot is required for the change to take effect.
    #[instrument]
    pub async fn set_language(udid: &str, language: &str) -> Result<()> {
        run_simctl(&[
            "spawn",
            udid,
            "defaults",
            "write",
            "NSGlobalDomain",
            "AppleLanguages",
            "-array",
            language,
        ])
        .await?;
        debug!(udid = %udid, language = %language, "language set");
        Ok(())
    }

    /// Override the status bar on a booted simulator for clean screenshots.
    #[instrument]
    pub async fn override_status_bar(udid: &str, overrides: &StatusBarOverrides) -> Result<()> {
        let mut args: Vec<String> = vec![
            "status_bar".to_string(),
            udid.to_string(),
            "override".to_string(),
        ];

        if let Some(ref time) = overrides.time {
            args.extend(["--time".to_string(), time.clone()]);
        }
        if let Some(level) = overrides.battery_level {
            args.extend(["--batteryLevel".to_string(), level.to_string()]);
        }
        if let Some(ref state) = overrides.battery_state {
            args.extend(["--batteryState".to_string(), state.clone()]);
        }
        if let Some(bars) = overrides.cellular_bars {
            args.extend(["--cellularBars".to_string(), bars.to_string()]);
        }
        if let Some(bars) = overrides.wifi_bars {
            args.extend(["--wifiBars".to_string(), bars.to_string()]);
        }
        if let Some(ref mode) = overrides.cellular_mode {
            args.extend(["--cellularMode".to_string(), mode.clone()]);
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        // Status bar override may not be available on all simulator runtimes;
        // we treat it as non-fatal.
        match run_simctl(&arg_refs).await {
            Ok(_) => {
                debug!(udid = %udid, "status bar overridden");
            }
            Err(e) => {
                warn!(udid = %udid, error = %e, "could not override status bar (non-fatal)");
            }
        }
        Ok(())
    }

    /// Clear all status bar overrides.
    #[instrument]
    pub async fn clear_status_bar(udid: &str) -> Result<()> {
        match run_simctl(&["status_bar", udid, "clear"]).await {
            Ok(_) => {
                debug!(udid = %udid, "status bar cleared");
            }
            Err(e) => {
                warn!(udid = %udid, error = %e, "could not clear status bar (non-fatal)");
            }
        }
        Ok(())
    }

    // ----- Utilities ---------------------------------------------------------

    /// Get the `xcodebuild -destination` string for a device.
    ///
    /// Returns something like
    /// `"platform=iOS Simulator,id=XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"`.
    pub fn destination_string(device: &SimDevice) -> String {
        format!("platform=iOS Simulator,id={}", device.udid)
    }

    // ----- Internal ----------------------------------------------------------

    /// Parse the JSON output of `simctl list devices -j`.
    fn parse_devices_json(data: &[u8]) -> Result<Vec<SimDevice>> {
        let parsed: SimctlDevices = serde_json::from_slice(data)
            .map_err(|e| FrameworkError::context("parse simctl devices JSON", e.to_string()))?;

        let mut devices = Vec::new();
        for (runtime_key, raw_devices) in parsed.devices {
            for raw in raw_devices {
                if !raw.is_available {
                    continue;
                }
                devices.push(SimDevice {
                    udid: raw.udid,
                    name: raw.name,
                    device_type: raw.device_type_identifier.unwrap_or_default(),
                    runtime: runtime_key.clone(),
                    state: SimDeviceState::from_str(&raw.state),
                });
            }
        }

        Ok(devices)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Sample `simctl list runtimes -j` output.
    const RUNTIMES_JSON: &str = r#"{
        "runtimes": [
            {
                "identifier": "com.apple.CoreSimulator.SimRuntime.iOS-17-5",
                "name": "iOS 17.5",
                "platform": "iOS",
                "version": "17.5",
                "isAvailable": true
            },
            {
                "identifier": "com.apple.CoreSimulator.SimRuntime.iOS-18-0",
                "name": "iOS 18.0",
                "platform": "iOS",
                "version": "18.0",
                "isAvailable": true
            },
            {
                "identifier": "com.apple.CoreSimulator.SimRuntime.watchOS-11-0",
                "name": "watchOS 11.0",
                "platform": "watchOS",
                "version": "11.0",
                "isAvailable": false
            }
        ]
    }"#;

    /// Sample `simctl list devicetypes -j` output.
    const DEVICE_TYPES_JSON: &str = r#"{
        "devicetypes": [
            {
                "identifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-16",
                "name": "iPhone 16",
                "minRuntimeVersionString": "18.0",
                "maxRuntimeVersionString": "18.2"
            },
            {
                "identifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro",
                "name": "iPhone 16 Pro"
            },
            {
                "identifier": "com.apple.CoreSimulator.SimDeviceType.iPad-Pro-13-inch-M4",
                "name": "iPad Pro 13-inch (M4)",
                "minRuntimeVersion": 18,
                "maxRuntimeVersion": 19
            }
        ]
    }"#;

    /// Sample `simctl list devices -j` output.
    const DEVICES_JSON: &str = r#"{
        "devices": {
            "com.apple.CoreSimulator.SimRuntime.iOS-18-0": [
                {
                    "udid": "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE",
                    "name": "iPhone 16 Pro",
                    "state": "Shutdown",
                    "isAvailable": true,
                    "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro"
                },
                {
                    "udid": "11111111-2222-3333-4444-555555555555",
                    "name": "iPhone 16",
                    "state": "Booted",
                    "isAvailable": true,
                    "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-16"
                },
                {
                    "udid": "DEADBEEF-DEAD-BEEF-DEAD-BEEFDEADBEEF",
                    "name": "iPhone SE",
                    "state": "Shutdown",
                    "isAvailable": false,
                    "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-SE"
                }
            ],
            "com.apple.CoreSimulator.SimRuntime.iOS-17-5": [
                {
                    "udid": "FFFFFFFF-FFFF-FFFF-FFFF-FFFFFFFFFFFF",
                    "name": "iPhone 15",
                    "state": "Shutdown",
                    "isAvailable": true,
                    "deviceTypeIdentifier": "com.apple.CoreSimulator.SimDeviceType.iPhone-15"
                }
            ]
        }
    }"#;

    #[test]
    fn test_parse_runtimes() {
        let parsed: SimctlRuntimes = serde_json::from_str(RUNTIMES_JSON).unwrap();
        assert_eq!(parsed.runtimes.len(), 3);
        assert_eq!(parsed.runtimes[0].name, "iOS 17.5");
        assert!(parsed.runtimes[0].is_available);
        assert!(!parsed.runtimes[2].is_available);
    }

    #[test]
    fn test_parse_device_types() {
        let parsed: SimctlDeviceTypes = serde_json::from_str(DEVICE_TYPES_JSON).unwrap();
        assert_eq!(parsed.device_types.len(), 3);

        let iphone16 = &parsed.device_types[0];
        assert_eq!(iphone16.name, "iPhone 16");
        assert_eq!(
            iphone16.min_runtime_version_string,
            Some("18.0".to_string())
        );
        assert_eq!(
            iphone16.max_runtime_version_string,
            Some("18.2".to_string())
        );

        // The iPad Pro uses numeric fields
        let ipad = &parsed.device_types[2];
        assert_eq!(ipad.name, "iPad Pro 13-inch (M4)");
        assert_eq!(ipad.min_runtime_version, Some(18));
    }

    #[test]
    fn test_parse_devices() {
        let devices = SimulatorManager::parse_devices_json(DEVICES_JSON.as_bytes()).unwrap();

        // The unavailable iPhone SE should be filtered out
        assert_eq!(devices.len(), 3);

        let booted: Vec<_> = devices
            .iter()
            .filter(|d| d.state == SimDeviceState::Booted)
            .collect();
        assert_eq!(booted.len(), 1);
        assert_eq!(booted[0].name, "iPhone 16");

        let iphone16_pro = devices.iter().find(|d| d.name == "iPhone 16 Pro").unwrap();
        assert_eq!(iphone16_pro.state, SimDeviceState::Shutdown);
        assert!(iphone16_pro.runtime.contains("iOS-18-0"));
    }

    #[test]
    fn test_parse_devices_empty() {
        let json = r#"{"devices": {}}"#;
        let devices = SimulatorManager::parse_devices_json(json.as_bytes()).unwrap();
        assert!(devices.is_empty());
    }

    #[test]
    fn test_platform_from_identifier() {
        assert_eq!(
            platform_from_identifier("com.apple.CoreSimulator.SimRuntime.iOS-18-0"),
            "iOS"
        );
        assert_eq!(
            platform_from_identifier("com.apple.CoreSimulator.SimRuntime.watchOS-11-0"),
            "watchOS"
        );
        assert_eq!(
            platform_from_identifier("com.apple.CoreSimulator.SimRuntime.visionOS-2-0"),
            "visionOS"
        );
    }

    #[test]
    fn test_version_from_identifier() {
        assert_eq!(
            version_from_identifier("com.apple.CoreSimulator.SimRuntime.iOS-18-0"),
            "18.0"
        );
        assert_eq!(
            version_from_identifier("com.apple.CoreSimulator.SimRuntime.iOS-17-5"),
            "17.5"
        );
    }

    #[test]
    fn test_runtime_display_name() {
        assert_eq!(
            runtime_display_name("com.apple.CoreSimulator.SimRuntime.iOS-18-0"),
            "iOS 18.0"
        );
        assert_eq!(runtime_display_name("iOS 18.0"), "iOS 18.0");
    }

    #[test]
    fn test_device_state_from_str() {
        assert_eq!(SimDeviceState::from_str("Booted"), SimDeviceState::Booted);
        assert_eq!(
            SimDeviceState::from_str("Shutdown"),
            SimDeviceState::Shutdown
        );
        assert_eq!(
            SimDeviceState::from_str("Creating"),
            SimDeviceState::Creating
        );
        assert_eq!(
            SimDeviceState::from_str("ShuttingDown"),
            SimDeviceState::ShuttingDown
        );
        assert_eq!(
            SimDeviceState::from_str("Unknown"),
            SimDeviceState::Shutdown
        );
    }

    #[test]
    fn test_device_state_display() {
        assert_eq!(SimDeviceState::Booted.to_string(), "Booted");
        assert_eq!(SimDeviceState::Shutdown.to_string(), "Shutdown");
    }

    #[test]
    fn test_appearance_as_str() {
        assert_eq!(Appearance::Light.as_str(), "light");
        assert_eq!(Appearance::Dark.as_str(), "dark");
    }

    #[test]
    fn test_status_bar_overrides_clean() {
        let overrides = StatusBarOverrides::clean();
        assert_eq!(overrides.time, Some("9:41".to_string()));
        assert_eq!(overrides.battery_level, Some(100));
        assert_eq!(overrides.cellular_bars, Some(4));
        assert_eq!(overrides.wifi_bars, Some(3));
    }

    #[test]
    fn test_destination_string() {
        let device = SimDevice {
            udid: "AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE".to_string(),
            name: "iPhone 16 Pro".to_string(),
            device_type: "com.apple.CoreSimulator.SimDeviceType.iPhone-16-Pro".to_string(),
            runtime: "com.apple.CoreSimulator.SimRuntime.iOS-18-0".to_string(),
            state: SimDeviceState::Booted,
        };

        assert_eq!(
            SimulatorManager::destination_string(&device),
            "platform=iOS Simulator,id=AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE"
        );
    }

    #[test]
    fn test_sim_device_serialization() {
        let device = SimDevice {
            udid: "test-udid".to_string(),
            name: "iPhone 16".to_string(),
            device_type: "com.apple.CoreSimulator.SimDeviceType.iPhone-16".to_string(),
            runtime: "com.apple.CoreSimulator.SimRuntime.iOS-18-0".to_string(),
            state: SimDeviceState::Booted,
        };

        let json = serde_json::to_string(&device).unwrap();
        let deserialized: SimDevice = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.udid, "test-udid");
        assert_eq!(deserialized.state, SimDeviceState::Booted);
    }
}
