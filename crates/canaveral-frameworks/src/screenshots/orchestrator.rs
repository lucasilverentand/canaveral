//! Screenshot orchestrator for automated App Store screenshot capture
//!
//! Coordinates the full screenshot pipeline: boot simulators, configure
//! locale/appearance/status bar, install the app, and capture screenshots
//! across every combination of device, locale, and appearance mode.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument, warn};

use crate::error::{FrameworkError, Result};
use crate::simulator::{Appearance, SimulatorManager, StatusBarOverrides};

/// Configuration for a full screenshot capture session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotCaptureConfig {
    /// Devices to capture on
    pub devices: Vec<ScreenshotDevice>,
    /// Locales to capture, e.g. `["en-US", "de-DE", "ja"]`
    pub locales: Vec<String>,
    /// Appearance modes to capture
    pub appearances: Vec<Appearance>,
    /// Root output directory
    pub output_dir: PathBuf,
    /// Whether to override the status bar for clean screenshots
    pub clean_status_bar: bool,
    /// App bundle identifier (e.g. `com.example.MyApp`)
    pub app_bundle_id: String,
    /// Path to the `.app` bundle to install (optional — skip install if None)
    pub app_path: Option<PathBuf>,
    /// Time to wait after launching the app before capturing (default 3s)
    #[serde(with = "humantime_serde_compat")]
    pub pre_screenshot_wait: Duration,
}

impl Default for ScreenshotCaptureConfig {
    fn default() -> Self {
        Self {
            devices: Vec::new(),
            locales: vec!["en-US".to_string()],
            appearances: vec![Appearance::Light],
            output_dir: PathBuf::from("screenshots"),
            clean_status_bar: true,
            app_bundle_id: String::new(),
            app_path: None,
            pre_screenshot_wait: Duration::from_secs(3),
        }
    }
}

impl ScreenshotCaptureConfig {
    pub fn new(app_bundle_id: impl Into<String>) -> Self {
        Self {
            app_bundle_id: app_bundle_id.into(),
            ..Default::default()
        }
    }

    pub fn with_device(mut self, device: ScreenshotDevice) -> Self {
        self.devices.push(device);
        self
    }

    pub fn with_locales(mut self, locales: Vec<String>) -> Self {
        self.locales = locales;
        self
    }

    pub fn with_appearances(mut self, appearances: Vec<Appearance>) -> Self {
        self.appearances = appearances;
        self
    }

    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = dir.into();
        self
    }

    pub fn with_app_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.app_path = Some(path.into());
        self
    }

    pub fn with_wait(mut self, wait: Duration) -> Self {
        self.pre_screenshot_wait = wait;
        self
    }
}

/// A device to capture screenshots on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotDevice {
    /// Simulator device name, e.g. `"iPhone 16 Pro Max"`
    pub name: String,
    /// Human-readable display name for output categorization, e.g. `"6.9-inch"`
    pub display_name: String,
}

impl ScreenshotDevice {
    pub fn new(name: impl Into<String>, display_name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            display_name: display_name.into(),
        }
    }

    /// iPhone 16 Pro Max (6.9")
    pub fn iphone_16_pro_max() -> Self {
        Self::new("iPhone 16 Pro Max", "6.9-inch")
    }

    /// iPhone 16 Pro (6.3")
    pub fn iphone_16_pro() -> Self {
        Self::new("iPhone 16 Pro", "6.3-inch")
    }

    /// iPhone 14 Pro Max (6.7")
    pub fn iphone_14_pro_max() -> Self {
        Self::new("iPhone 14 Pro Max", "6.7-inch")
    }

    /// iPhone SE (4.7")
    pub fn iphone_se() -> Self {
        Self::new("iPhone SE (3rd generation)", "4.7-inch")
    }

    /// iPad Pro 13" (M4)
    pub fn ipad_pro_13() -> Self {
        Self::new("iPad Pro 13-inch (M4)", "13-inch")
    }
}

/// A single captured screenshot with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedScreenshot {
    /// Path to the PNG file
    pub path: PathBuf,
    /// Device name
    pub device: String,
    /// Locale string
    pub locale: String,
    /// Appearance mode
    pub appearance: Appearance,
}

/// Orchestrates a full screenshot capture session.
pub struct ScreenshotOrchestrator;

impl ScreenshotOrchestrator {
    /// Run a complete screenshot capture session.
    ///
    /// For each `device x locale x appearance` combination this will:
    ///
    /// 1. Find (or boot) the simulator
    /// 2. Set locale and language
    /// 3. Reboot the simulator so locale/language take effect
    /// 4. Set appearance (light/dark)
    /// 5. Override the status bar (if configured)
    /// 6. Install and launch the app
    /// 7. Wait for the app to settle
    /// 8. Capture a screenshot
    /// 9. Save to `{output_dir}/{locale}/{display_name}/{appearance}/screenshot.png`
    ///
    /// Returns all successfully captured screenshots.
    #[instrument(skip(config), fields(
        devices = config.devices.len(),
        locales = config.locales.len(),
        appearances = config.appearances.len(),
    ))]
    pub async fn capture(config: &ScreenshotCaptureConfig) -> Result<Vec<CapturedScreenshot>> {
        if config.devices.is_empty() {
            return Err(FrameworkError::InvalidConfig {
                message: "no devices specified for screenshot capture".to_string(),
            });
        }
        if config.locales.is_empty() {
            return Err(FrameworkError::InvalidConfig {
                message: "no locales specified for screenshot capture".to_string(),
            });
        }

        let total = config.devices.len() * config.locales.len() * config.appearances.len();
        info!(total = total, "starting screenshot capture session");

        let mut results = Vec::with_capacity(total);
        let mut errors: Vec<String> = Vec::new();

        for device_spec in &config.devices {
            // Find the simulator
            let sim = match SimulatorManager::find_device(&device_spec.name, None).await? {
                Some(d) => d,
                None => {
                    let msg = format!("simulator '{}' not found — skipping", device_spec.name);
                    warn!("{}", msg);
                    errors.push(msg);
                    continue;
                }
            };
            let udid = &sim.udid;

            for locale in &config.locales {
                // Boot the simulator
                SimulatorManager::boot(udid).await?;

                // Set locale and language
                let lang = locale_to_language(locale);
                SimulatorManager::set_locale(udid, locale).await?;
                SimulatorManager::set_language(udid, &lang).await?;

                // Reboot so locale/language changes take effect
                SimulatorManager::shutdown(udid).await?;
                SimulatorManager::boot(udid).await?;

                // Install the app if a path was provided
                if let Some(ref app_path) = config.app_path {
                    SimulatorManager::install_app(udid, app_path).await?;
                }

                for appearance in &config.appearances {
                    // Set appearance
                    SimulatorManager::set_appearance(udid, *appearance).await?;

                    // Override status bar
                    if config.clean_status_bar {
                        SimulatorManager::override_status_bar(udid, &StatusBarOverrides::clean())
                            .await?;
                    }

                    // Launch the app
                    SimulatorManager::launch_app(udid, &config.app_bundle_id).await?;

                    // Wait for the app to settle
                    tokio::time::sleep(config.pre_screenshot_wait).await;

                    // Build output path
                    let output_path = screenshot_output_path(
                        &config.output_dir,
                        locale,
                        &device_spec.display_name,
                        *appearance,
                    );

                    // Capture
                    match SimulatorManager::screenshot(udid, &output_path).await {
                        Ok(()) => {
                            info!(
                                device = %device_spec.name,
                                locale = %locale,
                                appearance = %appearance,
                                path = %output_path.display(),
                                "screenshot captured"
                            );
                            results.push(CapturedScreenshot {
                                path: output_path,
                                device: device_spec.name.clone(),
                                locale: locale.clone(),
                                appearance: *appearance,
                            });
                        }
                        Err(e) => {
                            error!(
                                device = %device_spec.name,
                                locale = %locale,
                                appearance = %appearance,
                                error = %e,
                                "screenshot capture failed"
                            );
                            errors.push(format!(
                                "{} / {} / {}: {}",
                                device_spec.name, locale, appearance, e
                            ));
                        }
                    }

                    // Terminate the app between captures
                    let _ = SimulatorManager::terminate_app(udid, &config.app_bundle_id).await;

                    // Clear status bar overrides
                    if config.clean_status_bar {
                        let _ = SimulatorManager::clear_status_bar(udid).await;
                    }
                }
            }

            // Shutdown the simulator when done with this device
            let _ = SimulatorManager::shutdown(udid).await;
        }

        if results.is_empty() && !errors.is_empty() {
            return Err(FrameworkError::ScreenshotFailed {
                message: format!("all screenshot captures failed:\n  {}", errors.join("\n  ")),
            });
        }

        if !errors.is_empty() {
            warn!(
                failed = errors.len(),
                succeeded = results.len(),
                "some screenshots failed"
            );
        }

        info!(
            captured = results.len(),
            total = total,
            "screenshot session complete"
        );

        Ok(results)
    }
}

/// Build the output path for a screenshot.
///
/// Layout: `{output_dir}/{locale}/{device_display_name}/{appearance}/screenshot.png`
fn screenshot_output_path(
    output_dir: &Path,
    locale: &str,
    device_display_name: &str,
    appearance: Appearance,
) -> PathBuf {
    output_dir
        .join(locale)
        .join(device_display_name)
        .join(appearance.as_str())
        .join("screenshot.png")
}

/// Convert a locale like `"en-US"` or `"de-DE"` into the language tag
/// used by `AppleLanguages` (same format, just the first component or the
/// full tag).
fn locale_to_language(locale: &str) -> String {
    // AppleLanguages accepts BCP-47 tags. The locale itself (e.g. "en-US")
    // works, but we can also use just the language subtag.
    locale.to_string()
}

/// Serde helper for `Duration` — serializes as seconds (u64).
mod humantime_serde_compat {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S: Serializer>(
        duration: &Duration,
        ser: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        ser.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        de: D,
    ) -> std::result::Result<Duration, D::Error> {
        let secs = u64::deserialize(de)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screenshot_output_path() {
        let path =
            screenshot_output_path(Path::new("output"), "en-US", "6.9-inch", Appearance::Light);
        assert_eq!(
            path,
            PathBuf::from("output/en-US/6.9-inch/light/screenshot.png")
        );
    }

    #[test]
    fn test_screenshot_output_path_dark() {
        let path = screenshot_output_path(
            Path::new("/tmp/screenshots"),
            "de-DE",
            "13-inch",
            Appearance::Dark,
        );
        assert_eq!(
            path,
            PathBuf::from("/tmp/screenshots/de-DE/13-inch/dark/screenshot.png")
        );
    }

    #[test]
    fn test_locale_to_language() {
        assert_eq!(locale_to_language("en-US"), "en-US");
        assert_eq!(locale_to_language("de-DE"), "de-DE");
    }

    #[test]
    fn test_screenshot_device_presets() {
        let device = ScreenshotDevice::iphone_16_pro_max();
        assert_eq!(device.name, "iPhone 16 Pro Max");
        assert_eq!(device.display_name, "6.9-inch");
    }

    #[test]
    fn test_screenshot_capture_config_default() {
        let config = ScreenshotCaptureConfig::default();
        assert!(config.devices.is_empty());
        assert_eq!(config.locales, vec!["en-US"]);
        assert_eq!(config.appearances, vec![Appearance::Light]);
        assert!(config.clean_status_bar);
        assert_eq!(config.pre_screenshot_wait, Duration::from_secs(3));
    }

    #[test]
    fn test_screenshot_capture_config_builder() {
        let config = ScreenshotCaptureConfig::new("com.example.app")
            .with_device(ScreenshotDevice::iphone_16_pro_max())
            .with_device(ScreenshotDevice::ipad_pro_13())
            .with_locales(vec!["en-US".to_string(), "ja".to_string()])
            .with_appearances(vec![Appearance::Light, Appearance::Dark])
            .with_output_dir("/tmp/screenshots")
            .with_wait(Duration::from_secs(5));

        assert_eq!(config.app_bundle_id, "com.example.app");
        assert_eq!(config.devices.len(), 2);
        assert_eq!(config.locales.len(), 2);
        assert_eq!(config.appearances.len(), 2);
        assert_eq!(config.output_dir, PathBuf::from("/tmp/screenshots"));
        assert_eq!(config.pre_screenshot_wait, Duration::from_secs(5));
    }

    #[test]
    fn test_captured_screenshot_serialization() {
        let captured = CapturedScreenshot {
            path: PathBuf::from("screenshots/en-US/6.9-inch/light/screenshot.png"),
            device: "iPhone 16 Pro Max".to_string(),
            locale: "en-US".to_string(),
            appearance: Appearance::Light,
        };

        let json = serde_json::to_string(&captured).unwrap();
        let deserialized: CapturedScreenshot = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.device, "iPhone 16 Pro Max");
        assert_eq!(deserialized.locale, "en-US");
        assert_eq!(deserialized.appearance, Appearance::Light);
    }
}
