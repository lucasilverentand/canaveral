//! Device management for screenshot capture
//!
//! Manages iOS simulators, Android emulators, and device configurations.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::error::{FrameworkError, Result};
use crate::traits::Platform;

/// Device configuration for screenshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Device display name
    pub name: String,

    /// Target platform
    pub platform: Platform,

    /// iOS Simulator UDID (optional, will auto-detect)
    pub simulator_udid: Option<String>,

    /// Android AVD name
    pub avd_name: Option<String>,

    /// Screen resolution (width, height)
    pub resolution: (u32, u32),

    /// Device type for framing
    pub device_type: DeviceType,

    /// Scale factor for retina displays
    pub scale: f32,
}

impl DeviceConfig {
    /// Create a new iOS device config
    pub fn ios(name: impl Into<String>, resolution: (u32, u32)) -> Self {
        Self {
            name: name.into(),
            platform: Platform::Ios,
            simulator_udid: None,
            avd_name: None,
            resolution,
            device_type: DeviceType::IPhone,
            scale: 3.0,
        }
    }

    /// Create a new Android device config
    pub fn android(
        name: impl Into<String>,
        avd: impl Into<String>,
        resolution: (u32, u32),
    ) -> Self {
        Self {
            name: name.into(),
            platform: Platform::Android,
            simulator_udid: None,
            avd_name: Some(avd.into()),
            resolution,
            device_type: DeviceType::AndroidPhone,
            scale: 1.0,
        }
    }

    /// Set simulator UDID
    pub fn with_simulator(mut self, udid: impl Into<String>) -> Self {
        self.simulator_udid = Some(udid.into());
        self
    }

    /// Set device type
    pub fn with_device_type(mut self, device_type: DeviceType) -> Self {
        self.device_type = device_type;
        self
    }

    /// Get the device ID for commands
    pub fn device_id(&self) -> String {
        match self.platform {
            Platform::Ios => self
                .simulator_udid
                .clone()
                .unwrap_or_else(|| "booted".to_string()),
            Platform::Android => self.avd_name.clone().unwrap_or_default(),
            _ => String::new(),
        }
    }
}

/// Device type for frame selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceType {
    /// iPhone
    IPhone,
    /// iPhone with Dynamic Island
    IPhoneDynamicIsland,
    /// iPad
    IPad,
    /// iPad Pro
    IPadPro,
    /// Android Phone
    AndroidPhone,
    /// Android Tablet
    AndroidTablet,
}

/// Device specification for matching available devices
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceSpec {
    /// Device model name pattern
    pub model: String,

    /// Minimum iOS version (for iOS devices)
    pub min_ios_version: Option<String>,

    /// Minimum Android API level (for Android devices)
    pub min_api_level: Option<u32>,
}

/// iOS Simulator device info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorDevice {
    /// Device UDID
    pub udid: String,

    /// Device name
    pub name: String,

    /// Device state (Booted, Shutdown)
    pub state: String,

    /// iOS runtime version
    pub runtime: String,

    /// Device type identifier
    pub device_type_identifier: Option<String>,
}

/// Android emulator device info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmulatorDevice {
    /// AVD name
    pub name: String,

    /// Device display name
    pub display_name: String,

    /// Target API level
    pub api_level: u32,

    /// Device tag (phone, tablet, etc.)
    pub tag: String,

    /// ABI (x86_64, arm64-v8a)
    pub abi: String,
}

/// Device manager for screenshot automation
pub struct DeviceManager {
    /// Cached iOS simulators
    ios_simulators: Option<Vec<SimulatorDevice>>,

    /// Cached Android emulators
    android_emulators: Option<Vec<EmulatorDevice>>,

    /// Currently booted devices
    booted_devices: HashMap<String, bool>,
}

impl DeviceManager {
    /// Create a new device manager
    pub fn new() -> Self {
        Self {
            ios_simulators: None,
            android_emulators: None,
            booted_devices: HashMap::new(),
        }
    }

    /// List available iOS simulators
    pub fn list_ios_simulators(&mut self) -> Result<&[SimulatorDevice]> {
        if self.ios_simulators.is_none() {
            self.ios_simulators = Some(self.fetch_ios_simulators()?);
        }
        Ok(self.ios_simulators.as_ref().unwrap())
    }

    /// Fetch iOS simulators from simctl
    fn fetch_ios_simulators(&self) -> Result<Vec<SimulatorDevice>> {
        let output = Command::new("xcrun")
            .args(["simctl", "list", "devices", "--json"])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "xcrun simctl list".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        if !output.status.success() {
            return Err(FrameworkError::CommandFailed {
                command: "xcrun simctl list".to_string(),
                exit_code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            });
        }

        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).map_err(|e| FrameworkError::Context {
                context: "parse simctl output".to_string(),
                message: e.to_string(),
            })?;

        let mut devices = Vec::new();

        if let Some(device_map) = json.get("devices").and_then(|d| d.as_object()) {
            for (runtime, runtime_devices) in device_map {
                if let Some(device_list) = runtime_devices.as_array() {
                    for device in device_list {
                        if let (Some(udid), Some(name), Some(state)) = (
                            device.get("udid").and_then(|v| v.as_str()),
                            device.get("name").and_then(|v| v.as_str()),
                            device.get("state").and_then(|v| v.as_str()),
                        ) {
                            // Only include available devices
                            let is_available = device
                                .get("isAvailable")
                                .and_then(|v| v.as_bool())
                                .unwrap_or(true);

                            if is_available {
                                devices.push(SimulatorDevice {
                                    udid: udid.to_string(),
                                    name: name.to_string(),
                                    state: state.to_string(),
                                    runtime: runtime.clone(),
                                    device_type_identifier: device
                                        .get("deviceTypeIdentifier")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(devices)
    }

    /// List available Android emulators
    pub fn list_android_emulators(&mut self) -> Result<&[EmulatorDevice]> {
        if self.android_emulators.is_none() {
            self.android_emulators = Some(self.fetch_android_emulators()?);
        }
        Ok(self.android_emulators.as_ref().unwrap())
    }

    /// Fetch Android emulators from avdmanager
    fn fetch_android_emulators(&self) -> Result<Vec<EmulatorDevice>> {
        let output = Command::new("emulator")
            .args(["-list-avds"])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: "emulator -list-avds".to_string(),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let emulators: Vec<EmulatorDevice> = stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|name| EmulatorDevice {
                name: name.to_string(),
                display_name: name.to_string(),
                api_level: 0, // Would need to parse from AVD config
                tag: "phone".to_string(),
                abi: "x86_64".to_string(),
            })
            .collect();

        Ok(emulators)
    }

    /// Find a simulator matching the spec
    pub fn find_simulator(&mut self, spec: &DeviceSpec) -> Result<Option<SimulatorDevice>> {
        let simulators = self.list_ios_simulators()?;
        let pattern = spec.model.to_lowercase();

        Ok(simulators
            .iter()
            .find(|s| s.name.to_lowercase().contains(&pattern))
            .cloned())
    }

    /// Boot a device
    pub async fn boot_device(&mut self, device: &DeviceConfig) -> Result<()> {
        let device_id = device.device_id();

        if self
            .booted_devices
            .get(&device_id)
            .copied()
            .unwrap_or(false)
        {
            return Ok(());
        }

        match device.platform {
            Platform::Ios => {
                self.boot_ios_simulator(&device_id).await?;
            }
            Platform::Android => {
                self.boot_android_emulator(device.avd_name.as_deref().unwrap_or_default())
                    .await?;
            }
            _ => {
                return Err(FrameworkError::Context {
                    context: "boot device".to_string(),
                    message: format!("Unsupported platform: {:?}", device.platform),
                });
            }
        }

        self.booted_devices.insert(device_id, true);
        Ok(())
    }

    /// Boot an iOS simulator
    async fn boot_ios_simulator(&self, udid: &str) -> Result<()> {
        let output = Command::new("xcrun")
            .args(["simctl", "boot", udid])
            .output()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("xcrun simctl boot {}", udid),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        // Ignore "already booted" errors
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("Unable to boot device in current state: Booted") {
                return Err(FrameworkError::CommandFailed {
                    command: format!("xcrun simctl boot {}", udid),
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: stderr.to_string(),
                });
            }
        }

        // Wait for device to be ready
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;

        Ok(())
    }

    /// Boot an Android emulator
    async fn boot_android_emulator(&self, avd_name: &str) -> Result<()> {
        // Start emulator in background
        let _child = Command::new("emulator")
            .args(["-avd", avd_name, "-no-window", "-no-audio"])
            .spawn()
            .map_err(|e| FrameworkError::CommandFailed {
                command: format!("emulator -avd {}", avd_name),
                exit_code: None,
                stdout: String::new(),
                stderr: e.to_string(),
            })?;

        // Wait for emulator to boot
        self.wait_for_emulator().await?;

        Ok(())
    }

    /// Wait for Android emulator to be ready
    async fn wait_for_emulator(&self) -> Result<()> {
        for _ in 0..60 {
            let output = Command::new("adb")
                .args(["shell", "getprop", "sys.boot_completed"])
                .output();

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                if stdout.trim() == "1" {
                    return Ok(());
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        }

        Err(FrameworkError::Context {
            context: "emulator boot".to_string(),
            message: "Timeout waiting for emulator to boot".to_string(),
        })
    }

    /// Shutdown a device
    pub async fn shutdown_device(&mut self, device: &DeviceConfig) -> Result<()> {
        let device_id = device.device_id();

        match device.platform {
            Platform::Ios => {
                Command::new("xcrun")
                    .args(["simctl", "shutdown", &device_id])
                    .output()
                    .ok();
            }
            Platform::Android => {
                Command::new("adb").args(["emu", "kill"]).output().ok();
            }
            _ => {}
        }

        self.booted_devices.remove(&device_id);
        Ok(())
    }

    /// Set device locale
    pub async fn set_locale(&self, device: &DeviceConfig, locale: &str) -> Result<()> {
        match device.platform {
            Platform::Ios => {
                self.set_ios_locale(&device.device_id(), locale).await?;
            }
            Platform::Android => {
                self.set_android_locale(locale).await?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Set iOS simulator locale
    async fn set_ios_locale(&self, udid: &str, locale: &str) -> Result<()> {
        // Parse locale into language and region
        let parts: Vec<&str> = locale.split('_').collect();
        let language = parts.first().copied().unwrap_or("en");
        let region = parts.get(1).copied().unwrap_or("US");

        // Set language
        Command::new("xcrun")
            .args([
                "simctl",
                "spawn",
                udid,
                "defaults",
                "write",
                "NSGlobalDomain",
                "AppleLanguages",
                "-array",
                &format!("{}-{}", language, region),
            ])
            .output()
            .ok();

        // Set locale
        Command::new("xcrun")
            .args([
                "simctl",
                "spawn",
                udid,
                "defaults",
                "write",
                "NSGlobalDomain",
                "AppleLocale",
                "-string",
                locale,
            ])
            .output()
            .ok();

        Ok(())
    }

    /// Set Android emulator locale
    async fn set_android_locale(&self, locale: &str) -> Result<()> {
        let parts: Vec<&str> = locale.split('_').collect();
        let language = parts.first().copied().unwrap_or("en");
        let region = parts.get(1).copied().unwrap_or("US");

        Command::new("adb")
            .args([
                "shell",
                "setprop",
                "persist.sys.locale",
                &format!("{}-{}", language, region),
            ])
            .output()
            .ok();

        // Restart activity to apply locale
        Command::new("adb")
            .args([
                "shell",
                "am",
                "broadcast",
                "-a",
                "android.intent.action.LOCALE_CHANGED",
            ])
            .output()
            .ok();

        Ok(())
    }

    /// Install an app on device
    pub async fn install_app(&self, device: &DeviceConfig, app_path: &Path) -> Result<()> {
        match device.platform {
            Platform::Ios => {
                let output = Command::new("xcrun")
                    .args([
                        "simctl",
                        "install",
                        &device.device_id(),
                        app_path.to_str().unwrap_or_default(),
                    ])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: "xcrun simctl install".to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;

                if !output.status.success() {
                    return Err(FrameworkError::CommandFailed {
                        command: "xcrun simctl install".to_string(),
                        exit_code: output.status.code(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    });
                }
            }
            Platform::Android => {
                let output = Command::new("adb")
                    .args(["install", "-r", app_path.to_str().unwrap_or_default()])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: "adb install".to_string(),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;

                if !output.status.success() {
                    return Err(FrameworkError::CommandFailed {
                        command: "adb install".to_string(),
                        exit_code: output.status.code(),
                        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    });
                }
            }
            _ => {
                return Err(FrameworkError::Context {
                    context: "install app".to_string(),
                    message: format!("Unsupported platform: {:?}", device.platform),
                });
            }
        }

        Ok(())
    }

    /// Launch an app on device
    pub async fn launch_app(&self, device: &DeviceConfig, bundle_id: &str) -> Result<()> {
        match device.platform {
            Platform::Ios => {
                Command::new("xcrun")
                    .args(["simctl", "launch", &device.device_id(), bundle_id])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: format!(
                            "xcrun simctl launch {} {}",
                            device.device_id(),
                            bundle_id
                        ),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;
            }
            Platform::Android => {
                Command::new("adb")
                    .args([
                        "shell",
                        "am",
                        "start",
                        "-n",
                        &format!("{}/.MainActivity", bundle_id),
                    ])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: format!("adb shell am start {}", bundle_id),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Open a deep link on device
    pub async fn open_url(&self, device: &DeviceConfig, url: &str) -> Result<()> {
        match device.platform {
            Platform::Ios => {
                Command::new("xcrun")
                    .args(["simctl", "openurl", &device.device_id(), url])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: format!("xcrun simctl openurl {} {}", device.device_id(), url),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;
            }
            Platform::Android => {
                Command::new("adb")
                    .args([
                        "shell",
                        "am",
                        "start",
                        "-a",
                        "android.intent.action.VIEW",
                        "-d",
                        url,
                    ])
                    .output()
                    .map_err(|e| FrameworkError::CommandFailed {
                        command: format!("adb shell am start -d {}", url),
                        exit_code: None,
                        stdout: String::new(),
                        stderr: e.to_string(),
                    })?;
            }
            _ => {}
        }

        Ok(())
    }
}

impl Default for DeviceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-defined device configurations for common screenshot sizes
pub mod presets {
    use super::*;

    /// iPhone 16 Pro Max (6.9")
    pub fn iphone_16_pro_max() -> DeviceConfig {
        DeviceConfig::ios("iPhone 16 Pro Max", (1320, 2868))
            .with_device_type(DeviceType::IPhoneDynamicIsland)
    }

    /// iPhone 14 Pro Max (6.7")
    pub fn iphone_14_pro_max() -> DeviceConfig {
        DeviceConfig::ios("iPhone 14 Pro Max", (1290, 2796))
            .with_device_type(DeviceType::IPhoneDynamicIsland)
    }

    /// iPhone 8 Plus (5.5")
    pub fn iphone_8_plus() -> DeviceConfig {
        DeviceConfig::ios("iPhone 8 Plus", (1242, 2208)).with_device_type(DeviceType::IPhone)
    }

    /// iPad Pro 12.9"
    pub fn ipad_pro_129() -> DeviceConfig {
        DeviceConfig::ios("iPad Pro (12.9-inch)", (2048, 2732))
            .with_device_type(DeviceType::IPadPro)
    }

    /// iPad Pro 11"
    pub fn ipad_pro_11() -> DeviceConfig {
        DeviceConfig::ios("iPad Pro (11-inch)", (1668, 2388)).with_device_type(DeviceType::IPadPro)
    }

    /// All required iPhone sizes for App Store
    pub fn all_iphones() -> Vec<DeviceConfig> {
        vec![iphone_16_pro_max(), iphone_14_pro_max(), iphone_8_plus()]
    }

    /// All required iPad sizes for App Store
    pub fn all_ipads() -> Vec<DeviceConfig> {
        vec![ipad_pro_129(), ipad_pro_11()]
    }

    /// Pixel 7 Pro
    pub fn pixel_7_pro() -> DeviceConfig {
        DeviceConfig::android("Pixel 7 Pro", "Pixel_7_Pro_API_34", (1440, 3120))
    }

    /// Pixel Tablet
    pub fn pixel_tablet() -> DeviceConfig {
        DeviceConfig::android("Pixel Tablet", "Pixel_Tablet_API_34", (2560, 1600))
            .with_device_type(DeviceType::AndroidTablet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_config() {
        let device = DeviceConfig::ios("iPhone 14 Pro", (1179, 2556));
        assert_eq!(device.platform, Platform::Ios);
        assert_eq!(device.resolution, (1179, 2556));
    }

    #[test]
    fn test_presets() {
        let iphones = presets::all_iphones();
        assert_eq!(iphones.len(), 3);

        let ipads = presets::all_ipads();
        assert_eq!(ipads.len(), 2);
    }
}
