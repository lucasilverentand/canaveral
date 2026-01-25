//! Screenshot capture implementation
//!
//! Handles capturing screenshots from iOS simulators, Android emulators,
//! and connected devices.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{FrameworkError, Result};
use crate::traits::Platform;

use super::devices::DeviceConfig;
use super::ScreenConfig;

/// Screenshot configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotConfig {
    /// Devices to capture on
    pub devices: Vec<DeviceConfig>,

    /// Locales to capture
    pub locales: Vec<String>,

    /// Screens to capture
    pub screens: Vec<ScreenConfig>,

    /// Output directory
    pub output_dir: PathBuf,

    /// App bundle ID (iOS) or package name (Android)
    pub app_id: Option<String>,

    /// App path for installation
    pub app_path: Option<PathBuf>,

    /// Clear data before each capture
    pub clear_data: bool,

    /// Add status bar overlay
    pub status_bar_overlay: bool,
}

impl Default for ScreenshotConfig {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            locales: vec!["en_US".to_string()],
            screens: Vec::new(),
            output_dir: PathBuf::from("screenshots"),
            app_id: None,
            app_path: None,
            clear_data: false,
            status_bar_overlay: true,
        }
    }
}

impl ScreenshotConfig {
    /// Create a new config
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a device
    pub fn with_device(mut self, device: DeviceConfig) -> Self {
        self.devices.push(device);
        self
    }

    /// Add multiple devices
    pub fn with_devices(mut self, devices: Vec<DeviceConfig>) -> Self {
        self.devices.extend(devices);
        self
    }

    /// Add a locale
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locales.push(locale.into());
        self
    }

    /// Set locales
    pub fn with_locales(mut self, locales: Vec<String>) -> Self {
        self.locales = locales;
        self
    }

    /// Add a screen
    pub fn with_screen(mut self, screen: ScreenConfig) -> Self {
        self.screens.push(screen);
        self
    }

    /// Set output directory
    pub fn with_output_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.output_dir = path.into();
        self
    }

    /// Set app ID
    pub fn with_app_id(mut self, id: impl Into<String>) -> Self {
        self.app_id = Some(id.into());
        self
    }

    /// Set app path
    pub fn with_app_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.app_path = Some(path.into());
        self
    }

    /// Load from YAML file
    pub fn from_yaml(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| FrameworkError::Context {
            context: "load screenshot config".to_string(),
            message: e.to_string(),
        })?;

        serde_yaml::from_str(&content).map_err(|e| FrameworkError::Context {
            context: "parse screenshot config".to_string(),
            message: e.to_string(),
        })
    }

    /// Save to YAML file
    pub fn to_yaml(&self, path: &Path) -> Result<()> {
        let content =
            serde_yaml::to_string(self).map_err(|e| FrameworkError::Context {
                context: "serialize screenshot config".to_string(),
                message: e.to_string(),
            })?;

        std::fs::write(path, content).map_err(|e| FrameworkError::Context {
            context: "write screenshot config".to_string(),
            message: e.to_string(),
        })?;

        Ok(())
    }
}

/// Screenshot capture result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotResult {
    /// Screen name
    pub screen_name: String,

    /// Device name
    pub device_name: String,

    /// Locale
    pub locale: String,

    /// Output path
    pub path: PathBuf,

    /// Whether capture was successful
    pub success: bool,

    /// Error message if failed
    pub error: Option<String>,
}

impl ScreenshotResult {
    /// Create a successful result
    pub fn success(
        screen_name: impl Into<String>,
        device_name: impl Into<String>,
        locale: impl Into<String>,
        path: PathBuf,
    ) -> Self {
        Self {
            screen_name: screen_name.into(),
            device_name: device_name.into(),
            locale: locale.into(),
            path,
            success: true,
            error: None,
        }
    }

    /// Create a failed result
    pub fn failure(
        screen_name: impl Into<String>,
        device_name: impl Into<String>,
        locale: impl Into<String>,
        error: impl Into<String>,
    ) -> Self {
        Self {
            screen_name: screen_name.into(),
            device_name: device_name.into(),
            locale: locale.into(),
            path: PathBuf::new(),
            success: false,
            error: Some(error.into()),
        }
    }
}

/// Screenshot capture implementation
pub struct ScreenshotCapture {
    /// Target platform
    platform: Platform,
}

impl ScreenshotCapture {
    /// Create a new screenshot capture for a platform
    pub fn new(platform: Platform) -> Self {
        Self { platform }
    }

    /// Capture a screenshot
    pub async fn capture(&self, device_id: &str, output: &Path) -> Result<()> {
        // Ensure output directory exists
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent).map_err(|e| FrameworkError::Context {
                context: "create screenshot output dir".to_string(),
                message: e.to_string(),
            })?;
        }

        match self.platform {
            Platform::Ios => self.capture_ios(device_id, output).await,
            Platform::Android => self.capture_android(device_id, output).await,
            Platform::MacOs => self.capture_macos(output).await,
            _ => Err(FrameworkError::Context {
                context: "screenshot capture".to_string(),
                message: format!("Unsupported platform: {:?}", self.platform),
            }),
        }
    }

    /// Capture iOS simulator screenshot
    async fn capture_ios(&self, device_id: &str, output: &Path) -> Result<()> {
        let output = Command::new("xcrun")
            .args([
                "simctl",
                "io",
                device_id,
                "screenshot",
                "--type=png",
                output.to_str().unwrap_or_default(),
            ])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "xcrun simctl io screenshot".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "xcrun simctl io screenshot".to_string(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Capture Android emulator/device screenshot
    async fn capture_android(&self, _device_id: &str, output: &Path) -> Result<()> {
        // First capture to device
        let temp_path = "/sdcard/screenshot.png";

        let screencap = Command::new("adb")
            .args(["shell", "screencap", "-p", temp_path])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "adb shell screencap".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !screencap.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "adb shell screencap".to_string(),
                exit_code: screencap.status.code(),
                stdout: String::from_utf8_lossy(&screencap.stdout).to_string(),
                stderr: String::from_utf8_lossy(&screencap.stderr).to_string(),
            });
        }

        // Pull from device
        let pull = Command::new("adb")
            .args(["pull", temp_path, output.to_str().unwrap_or_default()])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "adb pull".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !pull.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "adb pull".to_string(),
                exit_code: pull.status.code(),
                stdout: String::from_utf8_lossy(&pull.stdout).to_string(),
                stderr: String::from_utf8_lossy(&pull.stderr).to_string(),
            });
        }

        // Clean up temp file
        Command::new("adb")
            .args(["shell", "rm", temp_path])
            .output()
            .ok();

        Ok(())
    }

    /// Capture macOS screenshot
    async fn capture_macos(&self, output: &Path) -> Result<()> {
        let result = Command::new("screencapture")
            .args(["-x", output.to_str().unwrap_or_default()])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "screencapture".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !result.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "screencapture".to_string(),
                exit_code: result.status.code(),
                stdout: String::from_utf8_lossy(&result.stdout).to_string(),
                stderr: String::from_utf8_lossy(&result.stderr).to_string(),
            });
        }

        Ok(())
    }

    /// Set a clean status bar (iOS)
    pub async fn set_clean_status_bar(&self, device_id: &str) -> Result<()> {
        if self.platform != Platform::Ios {
            return Ok(());
        }

        // Set clean status bar with simctl
        let output = Command::new("xcrun")
            .args([
                "simctl",
                "status_bar",
                device_id,
                "override",
                "--time",
                "9:41",
                "--batteryState",
                "charged",
                "--batteryLevel",
                "100",
                "--cellularMode",
                "active",
                "--cellularBars",
                "4",
                "--wifiBars",
                "3",
            ])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "xcrun simctl status_bar override".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            // Status bar override might not be available on all simulator versions
            // Don't fail if it doesn't work
            eprintln!(
                "Warning: Could not set clean status bar: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        Ok(())
    }

    /// Clear status bar override (iOS)
    pub async fn clear_status_bar(&self, device_id: &str) -> Result<()> {
        if self.platform != Platform::Ios {
            return Ok(());
        }

        Command::new("xcrun")
            .args(["simctl", "status_bar", device_id, "clear"])
            .output()
            .ok();

        Ok(())
    }
}

/// Screenshot batch processor
pub struct ScreenshotBatch {
    /// Results
    results: Vec<ScreenshotResult>,

    /// Total screenshots to capture
    total: usize,

    /// Completed count
    completed: usize,

    /// Failed count
    failed: usize,
}

impl ScreenshotBatch {
    /// Create a new batch
    pub fn new(total: usize) -> Self {
        Self {
            results: Vec::with_capacity(total),
            total,
            completed: 0,
            failed: 0,
        }
    }

    /// Add a result
    pub fn add(&mut self, result: ScreenshotResult) {
        if result.success {
            self.completed += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(result);
    }

    /// Get progress (0.0 - 1.0)
    pub fn progress(&self) -> f32 {
        if self.total == 0 {
            return 1.0;
        }
        (self.completed + self.failed) as f32 / self.total as f32
    }

    /// Check if all completed successfully
    pub fn all_success(&self) -> bool {
        self.failed == 0 && self.completed == self.total
    }

    /// Get results
    pub fn results(&self) -> &[ScreenshotResult] {
        &self.results
    }

    /// Get successful results
    pub fn successful(&self) -> Vec<&ScreenshotResult> {
        self.results.iter().filter(|r| r.success).collect()
    }

    /// Get failed results
    pub fn failed(&self) -> Vec<&ScreenshotResult> {
        self.results.iter().filter(|r| !r.success).collect()
    }

    /// Generate summary report
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();

        lines.push(format!(
            "Screenshot Batch: {} total, {} completed, {} failed",
            self.total, self.completed, self.failed
        ));

        if self.failed > 0 {
            lines.push("\nFailed captures:".to_string());
            for result in self.failed() {
                lines.push(format!(
                    "  - {} / {} / {}: {}",
                    result.screen_name,
                    result.device_name,
                    result.locale,
                    result.error.as_deref().unwrap_or("Unknown error")
                ));
            }
        }

        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_config_default() {
        let config = ScreenshotConfig::default();
        assert!(config.devices.is_empty());
        assert_eq!(config.locales, vec!["en_US"]);
    }

    #[test]
    fn test_screenshot_config_builder() {
        // Use with_locales to replace the default locale list
        let config = ScreenshotConfig::new()
            .with_locales(vec!["en_US".to_string(), "de_DE".to_string()])
            .with_output_dir("output/screenshots");

        assert_eq!(config.locales.len(), 2);
        assert_eq!(config.output_dir, PathBuf::from("output/screenshots"));
    }

    #[test]
    fn test_screenshot_config_add_locale() {
        // with_locale adds to the existing default locale list
        let config = ScreenshotConfig::new()
            .with_locale("de_DE")
            .with_locale("fr_FR");

        // Default has "en_US", adding two more gives 3
        assert_eq!(config.locales.len(), 3);
        assert!(config.locales.contains(&"en_US".to_string()));
        assert!(config.locales.contains(&"de_DE".to_string()));
        assert!(config.locales.contains(&"fr_FR".to_string()));
    }

    #[test]
    fn test_screenshot_result() {
        let success = ScreenshotResult::success(
            "home_screen",
            "iPhone 14 Pro",
            "en_US",
            PathBuf::from("test.png"),
        );
        assert!(success.success);

        let failure = ScreenshotResult::failure(
            "home_screen",
            "iPhone 14 Pro",
            "en_US",
            "Device not booted",
        );
        assert!(!failure.success);
    }

    #[test]
    fn test_screenshot_batch() {
        let mut batch = ScreenshotBatch::new(3);

        batch.add(ScreenshotResult::success(
            "screen1",
            "device1",
            "en",
            PathBuf::from("1.png"),
        ));
        batch.add(ScreenshotResult::success(
            "screen2",
            "device1",
            "en",
            PathBuf::from("2.png"),
        ));
        batch.add(ScreenshotResult::failure("screen3", "device1", "en", "Failed"));

        assert_eq!(batch.completed, 2);
        assert_eq!(batch.failed, 1);
        assert!(!batch.all_success());
    }
}
