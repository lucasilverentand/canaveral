//! Execution contexts for framework operations
//!
//! Contexts provide all the information needed to execute a build, test, or
//! screenshot operation. They are framework-agnostic and get translated by
//! each adapter into framework-specific commands.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::traits::Platform;

/// Build context - everything needed to build a project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildContext {
    /// Path to the project root
    pub path: PathBuf,

    /// Target platform
    pub platform: Platform,

    /// Build profile/configuration
    pub profile: BuildProfile,

    /// Output directory (optional, adapter may have defaults)
    pub output_dir: Option<PathBuf>,

    /// Whether this is a dry run (validate but don't build)
    pub dry_run: bool,

    /// Whether to run in CI mode (non-interactive, stricter)
    pub ci: bool,

    /// Framework-specific configuration
    pub config: HashMap<String, serde_json::Value>,

    /// Environment variables to set
    pub env: HashMap<String, String>,

    /// Build flavor/variant (e.g., "production", "staging")
    pub flavor: Option<String>,

    /// Code signing configuration
    pub signing: Option<SigningConfig>,

    /// Version to embed in build
    pub version: Option<String>,

    /// Build number to embed
    pub build_number: Option<u64>,
}

impl BuildContext {
    /// Create a new build context
    pub fn new(path: impl Into<PathBuf>, platform: Platform) -> Self {
        Self {
            path: path.into(),
            platform,
            profile: BuildProfile::Release,
            output_dir: None,
            dry_run: false,
            ci: std::env::var("CI").is_ok(),
            config: HashMap::new(),
            env: HashMap::new(),
            flavor: None,
            signing: None,
            version: None,
            build_number: None,
        }
    }

    pub fn with_profile(mut self, profile: BuildProfile) -> Self {
        self.profile = profile;
        self
    }

    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = Some(dir.into());
        self
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.dry_run = dry_run;
        self
    }

    pub fn with_ci(mut self, ci: bool) -> Self {
        self.ci = ci;
        self
    }

    pub fn with_config(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.config.insert(key.into(), value);
        self
    }

    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    pub fn with_flavor(mut self, flavor: impl Into<String>) -> Self {
        self.flavor = Some(flavor.into());
        self
    }

    pub fn with_signing(mut self, signing: SigningConfig) -> Self {
        self.signing = Some(signing);
        self
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn with_build_number(mut self, build_number: u64) -> Self {
        self.build_number = Some(build_number);
        self
    }

    /// Load context from environment variables (CI/CD mode)
    pub fn from_env(path: impl Into<PathBuf>, platform: Platform) -> Self {
        let mut ctx = Self::new(path, platform);

        // Standard CI env vars
        if let Ok(v) = std::env::var("CANAVERAL_PROFILE") {
            ctx.profile = BuildProfile::parse(&v).unwrap_or(BuildProfile::Release);
        }
        if let Ok(v) = std::env::var("CANAVERAL_FLAVOR") {
            ctx.flavor = Some(v);
        }
        if let Ok(v) = std::env::var("CANAVERAL_VERSION") {
            ctx.version = Some(v);
        }
        if let Ok(v) = std::env::var("CANAVERAL_BUILD_NUMBER") {
            if let Ok(n) = v.parse() {
                ctx.build_number = Some(n);
            }
        }
        if let Ok(v) = std::env::var("CANAVERAL_OUTPUT_DIR") {
            ctx.output_dir = Some(PathBuf::from(v));
        }
        if std::env::var("CANAVERAL_DRY_RUN").is_ok() {
            ctx.dry_run = true;
        }

        ctx
    }
}

/// Build profile/configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BuildProfile {
    /// Debug build (fast compilation, no optimization)
    Debug,
    /// Release build (optimized, stripped)
    #[default]
    Release,
    /// Profile build (release with debug symbols)
    Profile,
}

impl BuildProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Debug => "debug",
            Self::Release => "release",
            Self::Profile => "profile",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "debug" | "dev" => Some(Self::Debug),
            "release" | "prod" | "production" => Some(Self::Release),
            "profile" => Some(Self::Profile),
            _ => None,
        }
    }
}

/// Code signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SigningConfig {
    /// iOS: signing identity name or hash
    pub identity: Option<String>,
    /// iOS: provisioning profile name or UUID
    pub provisioning_profile: Option<String>,
    /// iOS: team ID
    pub team_id: Option<String>,
    /// Android: keystore path
    pub keystore_path: Option<PathBuf>,
    /// Android: key alias
    pub key_alias: Option<String>,
    /// Whether to use automatic signing
    pub automatic: bool,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            identity: None,
            provisioning_profile: None,
            team_id: None,
            keystore_path: None,
            key_alias: None,
            automatic: true,
        }
    }
}

/// Test context - everything needed to run tests
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestContext {
    /// Path to the project root
    pub path: PathBuf,

    /// Target platform (optional, some tests are platform-agnostic)
    pub platform: Option<Platform>,

    /// Test filter/pattern
    pub filter: Option<String>,

    /// Whether to collect coverage
    pub coverage: bool,

    /// Whether to run in CI mode
    pub ci: bool,

    /// Whether this is a dry run
    pub dry_run: bool,

    /// Maximum test duration in seconds
    pub timeout: Option<u64>,

    /// Number of parallel test jobs
    pub jobs: Option<usize>,

    /// Framework-specific configuration
    pub config: HashMap<String, serde_json::Value>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Test reporter format
    pub reporter: TestReporter,
}

impl TestContext {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            platform: None,
            filter: None,
            coverage: false,
            ci: std::env::var("CI").is_ok(),
            dry_run: false,
            timeout: None,
            jobs: None,
            config: HashMap::new(),
            env: HashMap::new(),
            reporter: TestReporter::default(),
        }
    }

    pub fn with_platform(mut self, platform: Platform) -> Self {
        self.platform = Some(platform);
        self
    }

    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    pub fn with_coverage(mut self, coverage: bool) -> Self {
        self.coverage = coverage;
        self
    }

    pub fn with_timeout(mut self, seconds: u64) -> Self {
        self.timeout = Some(seconds);
        self
    }

    pub fn with_reporter(mut self, reporter: TestReporter) -> Self {
        self.reporter = reporter;
        self
    }
}

/// Test reporter format
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TestReporter {
    /// Human-readable output
    #[default]
    Pretty,
    /// JSON output
    Json,
    /// JUnit XML
    Junit,
    /// GitHub Actions annotations
    GithubActions,
}

/// Screenshot context - everything needed to capture screenshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenshotContext {
    /// Path to the project root
    pub path: PathBuf,

    /// Target platform
    pub platform: Platform,

    /// Devices to capture on
    pub devices: Vec<DeviceConfig>,

    /// Locales to capture
    pub locales: Vec<String>,

    /// Output directory for screenshots
    pub output_dir: PathBuf,

    /// Whether to run in CI mode
    pub ci: bool,

    /// Whether this is a dry run
    pub dry_run: bool,

    /// Screenshot configuration file path
    pub config_file: Option<PathBuf>,

    /// Framework-specific configuration
    pub config: HashMap<String, serde_json::Value>,
}

impl ScreenshotContext {
    pub fn new(path: impl Into<PathBuf>, platform: Platform) -> Self {
        let path = path.into();
        Self {
            output_dir: path.join("screenshots"),
            path,
            platform,
            devices: Vec::new(),
            locales: vec!["en-US".to_string()],
            ci: std::env::var("CI").is_ok(),
            dry_run: false,
            config_file: None,
            config: HashMap::new(),
        }
    }

    pub fn with_device(mut self, device: DeviceConfig) -> Self {
        self.devices.push(device);
        self
    }

    pub fn with_devices(mut self, devices: Vec<DeviceConfig>) -> Self {
        self.devices = devices;
        self
    }

    pub fn with_locales(mut self, locales: Vec<String>) -> Self {
        self.locales = locales;
        self
    }

    pub fn with_output_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.output_dir = dir.into();
        self
    }
}

/// Device configuration for screenshots
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceConfig {
    /// Device name/identifier
    pub name: String,
    /// Device type (phone, tablet)
    pub device_type: DeviceType,
    /// Screen width in pixels
    pub width: u32,
    /// Screen height in pixels
    pub height: u32,
    /// Simulator/emulator ID (optional)
    pub simulator_id: Option<String>,
}

impl DeviceConfig {
    pub fn new(name: impl Into<String>, width: u32, height: u32) -> Self {
        Self {
            name: name.into(),
            device_type: DeviceType::Phone,
            width,
            height,
            simulator_id: None,
        }
    }

    /// iPhone 15 Pro Max (6.7")
    pub fn iphone_15_pro_max() -> Self {
        Self::new("iPhone 15 Pro Max", 1290, 2796)
    }

    /// iPhone 8 Plus (5.5")
    pub fn iphone_8_plus() -> Self {
        Self::new("iPhone 8 Plus", 1242, 2208)
    }

    /// iPad Pro 12.9" (6th gen)
    pub fn ipad_pro_12_9() -> Self {
        Self {
            name: "iPad Pro 12.9".to_string(),
            device_type: DeviceType::Tablet,
            width: 2048,
            height: 2732,
            simulator_id: None,
        }
    }

    /// Pixel 7 Pro
    pub fn pixel_7_pro() -> Self {
        Self::new("Pixel 7 Pro", 1440, 3120)
    }
}

/// Device type
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Phone,
    Tablet,
    Desktop,
    Tv,
    Watch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_context_builder() {
        let ctx = BuildContext::new("/project", Platform::Ios)
            .with_profile(BuildProfile::Release)
            .with_flavor("production")
            .with_version("1.2.3")
            .with_build_number(42);

        assert_eq!(ctx.platform, Platform::Ios);
        assert_eq!(ctx.profile, BuildProfile::Release);
        assert_eq!(ctx.flavor, Some("production".to_string()));
        assert_eq!(ctx.version, Some("1.2.3".to_string()));
        assert_eq!(ctx.build_number, Some(42));
    }

    #[test]
    fn test_build_profile_parsing() {
        assert_eq!(BuildProfile::parse("debug"), Some(BuildProfile::Debug));
        assert_eq!(BuildProfile::parse("release"), Some(BuildProfile::Release));
        assert_eq!(BuildProfile::parse("prod"), Some(BuildProfile::Release));
        assert_eq!(BuildProfile::parse("invalid"), None);
    }

    #[test]
    fn test_device_presets() {
        let iphone = DeviceConfig::iphone_15_pro_max();
        assert_eq!(iphone.width, 1290);
        assert_eq!(iphone.height, 2796);

        let ipad = DeviceConfig::ipad_pro_12_9();
        assert!(matches!(ipad.device_type, DeviceType::Tablet));
    }
}
