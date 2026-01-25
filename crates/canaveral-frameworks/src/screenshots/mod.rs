//! Screenshot automation framework
//!
//! Provides screenshot capture, device management, and framing capabilities
//! for app store screenshots across multiple platforms and frameworks.

pub mod capture;
pub mod devices;
pub mod framing;

pub use capture::{ScreenshotCapture, ScreenshotConfig, ScreenshotResult};
pub use devices::{DeviceConfig, DeviceManager, DeviceSpec, SimulatorDevice};
pub use framing::{FrameConfig, FrameTemplate, ScreenshotFramer};

use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Screen configuration for screenshot capture
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenConfig {
    /// Name/identifier for this screen
    pub name: String,

    /// Route or deep link to navigate to
    pub route: String,

    /// Optional setup script to run before capturing
    pub setup_script: Option<String>,

    /// Time to wait after navigation before capture (ms)
    pub wait_ms: u64,

    /// Optional locale override for this screen
    pub locale: Option<String>,
}

impl ScreenConfig {
    /// Create a new screen config
    pub fn new(name: impl Into<String>, route: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            route: route.into(),
            setup_script: None,
            wait_ms: 1000,
            locale: None,
        }
    }

    /// Set wait time
    pub fn with_wait(mut self, ms: u64) -> Self {
        self.wait_ms = ms;
        self
    }

    /// Set setup script
    pub fn with_setup(mut self, script: impl Into<String>) -> Self {
        self.setup_script = Some(script.into());
        self
    }

    /// Set locale
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }
}

/// Screenshot session for orchestrating captures
pub struct ScreenshotSession {
    /// Configuration
    pub config: ScreenshotConfig,

    /// Device manager
    device_manager: DeviceManager,

    /// Captured results
    results: Vec<ScreenshotResult>,
}

impl ScreenshotSession {
    /// Create a new screenshot session
    pub fn new(config: ScreenshotConfig) -> Self {
        Self {
            config,
            device_manager: DeviceManager::new(),
            results: Vec::new(),
        }
    }

    /// Run the screenshot session
    pub async fn run(&mut self) -> Result<Vec<ScreenshotResult>> {
        let mut all_results = Vec::new();

        for device in &self.config.devices.clone() {
            // Boot device if needed
            self.device_manager.boot_device(device).await?;

            for locale in &self.config.locales.clone() {
                // Set device locale
                self.device_manager
                    .set_locale(device, locale)
                    .await?;

                for screen in &self.config.screens.clone() {
                    // Capture screenshot
                    let result = self
                        .capture_screen(device, screen, locale)
                        .await?;
                    all_results.push(result);
                }
            }

            // Shutdown device
            self.device_manager.shutdown_device(device).await?;
        }

        self.results = all_results.clone();
        Ok(all_results)
    }

    /// Capture a single screen
    async fn capture_screen(
        &self,
        device: &DeviceConfig,
        screen: &ScreenConfig,
        locale: &str,
    ) -> Result<ScreenshotResult> {
        // Build output path
        let filename = format!(
            "{}_{}_{}_{}.png",
            screen.name,
            device.name.replace(' ', "_").to_lowercase(),
            locale,
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        let output_path = self.config.output_dir.join(&filename);

        // Navigate to screen
        if let Some(ref setup) = screen.setup_script {
            self.run_setup_script(setup).await?;
        }

        // Wait for content to load
        tokio::time::sleep(std::time::Duration::from_millis(screen.wait_ms)).await;

        // Capture based on platform
        let capture = ScreenshotCapture::new(device.platform);
        capture.capture(&device.device_id(), &output_path).await?;

        Ok(ScreenshotResult {
            screen_name: screen.name.clone(),
            device_name: device.name.clone(),
            locale: locale.to_string(),
            path: output_path,
            success: true,
            error: None,
        })
    }

    /// Run a setup script
    async fn run_setup_script(&self, _script: &str) -> Result<()> {
        // TODO: Execute setup script
        Ok(())
    }

    /// Get captured results
    pub fn results(&self) -> &[ScreenshotResult] {
        &self.results
    }
}

/// App Store screenshot sizes
#[derive(Debug, Clone, Copy)]
pub enum AppStoreScreenSize {
    /// iPhone 6.9" (iPhone 16 Pro Max) - 1320 x 2868
    IPhone69,
    /// iPhone 6.7" (iPhone 14 Pro Max) - 1290 x 2796
    IPhone67,
    /// iPhone 6.5" (iPhone 11 Pro Max) - 1284 x 2778
    IPhone65,
    /// iPhone 5.5" (iPhone 8 Plus) - 1242 x 2208
    IPhone55,
    /// iPad Pro 13" - 2064 x 2752
    IPadPro13,
    /// iPad Pro 12.9" (6th gen) - 2048 x 2732
    IPadPro129,
    /// iPad Pro 11" - 1668 x 2388
    IPadPro11,
}

impl AppStoreScreenSize {
    /// Get the resolution (width, height)
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            Self::IPhone69 => (1320, 2868),
            Self::IPhone67 => (1290, 2796),
            Self::IPhone65 => (1284, 2778),
            Self::IPhone55 => (1242, 2208),
            Self::IPadPro13 => (2064, 2752),
            Self::IPadPro129 => (2048, 2732),
            Self::IPadPro11 => (1668, 2388),
        }
    }

    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::IPhone69 => "iPhone 6.9\"",
            Self::IPhone67 => "iPhone 6.7\"",
            Self::IPhone65 => "iPhone 6.5\"",
            Self::IPhone55 => "iPhone 5.5\"",
            Self::IPadPro13 => "iPad Pro 13\"",
            Self::IPadPro129 => "iPad Pro 12.9\"",
            Self::IPadPro11 => "iPad Pro 11\"",
        }
    }
}

/// Google Play screenshot sizes
#[derive(Debug, Clone, Copy)]
pub enum PlayStoreScreenSize {
    /// Phone - 1080 x 1920 minimum
    Phone,
    /// 7" Tablet - 1200 x 1920
    Tablet7,
    /// 10" Tablet - 1920 x 1200
    Tablet10,
}

impl PlayStoreScreenSize {
    /// Get the minimum resolution (width, height)
    pub fn resolution(&self) -> (u32, u32) {
        match self {
            Self::Phone => (1080, 1920),
            Self::Tablet7 => (1200, 1920),
            Self::Tablet10 => (1920, 1200),
        }
    }

    /// Get the display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Phone => "Phone",
            Self::Tablet7 => "7\" Tablet",
            Self::Tablet10 => "10\" Tablet",
        }
    }
}
