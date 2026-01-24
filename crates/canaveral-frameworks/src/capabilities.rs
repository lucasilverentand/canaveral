//! Capability declarations for framework adapters
//!
//! Capabilities declare what features an adapter supports, allowing the
//! orchestrator to make decisions about which adapters to use and what
//! operations are available.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

/// A capability that an adapter might support
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    // Build capabilities
    /// Can build for iOS
    BuildIos,
    /// Can build for Android
    BuildAndroid,
    /// Can build for macOS
    BuildMacos,
    /// Can build for Windows
    BuildWindows,
    /// Can build for Linux
    BuildLinux,
    /// Can build for Web
    BuildWeb,

    // Build features
    /// Supports debug builds
    DebugBuild,
    /// Supports release builds
    ReleaseBuild,
    /// Supports profile/instrumented builds
    ProfileBuild,
    /// Supports build flavors/variants
    BuildFlavors,
    /// Supports code signing during build
    CodeSigning,

    // Test capabilities
    /// Can run unit tests
    UnitTests,
    /// Can run integration tests
    IntegrationTests,
    /// Can run end-to-end tests
    E2eTests,
    /// Can run widget/component tests
    WidgetTests,
    /// Can collect code coverage
    Coverage,

    // Screenshot capabilities
    /// Can capture automated screenshots
    Screenshots,
    /// Can capture screenshots on simulators
    SimulatorScreenshots,
    /// Can capture screenshots on real devices
    DeviceScreenshots,

    // Version capabilities
    /// Can read version from project
    ReadVersion,
    /// Can write version to project
    WriteVersion,
    /// Supports build numbers
    BuildNumbers,

    // Distribution capabilities
    /// Supports OTA updates
    OtaUpdates,
    /// Supports hot reload during development
    HotReload,

    // Advanced
    /// Supports incremental builds
    IncrementalBuild,
    /// Supports parallel builds
    ParallelBuild,
    /// Supports remote/cloud builds
    RemoteBuild,
}

impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BuildIos => "build_ios",
            Self::BuildAndroid => "build_android",
            Self::BuildMacos => "build_macos",
            Self::BuildWindows => "build_windows",
            Self::BuildLinux => "build_linux",
            Self::BuildWeb => "build_web",
            Self::DebugBuild => "debug_build",
            Self::ReleaseBuild => "release_build",
            Self::ProfileBuild => "profile_build",
            Self::BuildFlavors => "build_flavors",
            Self::CodeSigning => "code_signing",
            Self::UnitTests => "unit_tests",
            Self::IntegrationTests => "integration_tests",
            Self::E2eTests => "e2e_tests",
            Self::WidgetTests => "widget_tests",
            Self::Coverage => "coverage",
            Self::Screenshots => "screenshots",
            Self::SimulatorScreenshots => "simulator_screenshots",
            Self::DeviceScreenshots => "device_screenshots",
            Self::ReadVersion => "read_version",
            Self::WriteVersion => "write_version",
            Self::BuildNumbers => "build_numbers",
            Self::OtaUpdates => "ota_updates",
            Self::HotReload => "hot_reload",
            Self::IncrementalBuild => "incremental_build",
            Self::ParallelBuild => "parallel_build",
            Self::RemoteBuild => "remote_build",
        }
    }

    /// Get all build platform capabilities
    pub fn build_platforms() -> &'static [Capability] {
        &[
            Self::BuildIos,
            Self::BuildAndroid,
            Self::BuildMacos,
            Self::BuildWindows,
            Self::BuildLinux,
            Self::BuildWeb,
        ]
    }

    /// Get all test capabilities
    pub fn test_capabilities() -> &'static [Capability] {
        &[
            Self::UnitTests,
            Self::IntegrationTests,
            Self::E2eTests,
            Self::WidgetTests,
            Self::Coverage,
        ]
    }
}

/// Set of capabilities for an adapter
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    capabilities: HashSet<Capability>,
}

impl Capabilities {
    /// Create an empty capability set
    pub fn new() -> Self {
        Self::default()
    }

    /// Create capabilities from a list
    pub fn from_list(caps: &[Capability]) -> Self {
        Self {
            capabilities: caps.iter().copied().collect(),
        }
    }

    /// Add a capability
    pub fn add(&mut self, cap: Capability) -> &mut Self {
        self.capabilities.insert(cap);
        self
    }

    /// Add multiple capabilities
    pub fn add_all(&mut self, caps: &[Capability]) -> &mut Self {
        for cap in caps {
            self.capabilities.insert(*cap);
        }
        self
    }

    /// Check if a capability is present
    pub fn has(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Check if all capabilities are present
    pub fn has_all(&self, caps: &[Capability]) -> bool {
        caps.iter().all(|c| self.has(*c))
    }

    /// Check if any capability is present
    pub fn has_any(&self, caps: &[Capability]) -> bool {
        caps.iter().any(|c| self.has(*c))
    }

    /// Get all capabilities
    pub fn all(&self) -> impl Iterator<Item = &Capability> {
        self.capabilities.iter()
    }

    /// Check if can build for any mobile platform
    pub fn can_build_mobile(&self) -> bool {
        self.has(Capability::BuildIos) || self.has(Capability::BuildAndroid)
    }

    /// Check if can build for any desktop platform
    pub fn can_build_desktop(&self) -> bool {
        self.has(Capability::BuildMacos)
            || self.has(Capability::BuildWindows)
            || self.has(Capability::BuildLinux)
    }

    /// Check if can run any tests
    pub fn can_test(&self) -> bool {
        self.has_any(Capability::test_capabilities())
    }

    /// Builder pattern - with capability
    pub fn with(mut self, cap: Capability) -> Self {
        self.add(cap);
        self
    }

    /// Builder pattern - with multiple capabilities
    pub fn with_all(mut self, caps: &[Capability]) -> Self {
        self.add_all(caps);
        self
    }
}

impl FromIterator<Capability> for Capabilities {
    fn from_iter<T: IntoIterator<Item = Capability>>(iter: T) -> Self {
        Self {
            capabilities: iter.into_iter().collect(),
        }
    }
}

/// Common capability sets for frameworks
impl Capabilities {
    /// Capabilities typical for Flutter
    pub fn flutter() -> Self {
        Self::new()
            .with(Capability::BuildIos)
            .with(Capability::BuildAndroid)
            .with(Capability::BuildMacos)
            .with(Capability::BuildWindows)
            .with(Capability::BuildLinux)
            .with(Capability::BuildWeb)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::ProfileBuild)
            .with(Capability::BuildFlavors)
            .with(Capability::UnitTests)
            .with(Capability::WidgetTests)
            .with(Capability::IntegrationTests)
            .with(Capability::Coverage)
            .with(Capability::Screenshots)
            .with(Capability::SimulatorScreenshots)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::BuildNumbers)
            .with(Capability::HotReload)
            .with(Capability::IncrementalBuild)
    }

    /// Capabilities typical for Expo
    pub fn expo() -> Self {
        Self::new()
            .with(Capability::BuildIos)
            .with(Capability::BuildAndroid)
            .with(Capability::BuildWeb)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::UnitTests)
            .with(Capability::E2eTests)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::BuildNumbers)
            .with(Capability::OtaUpdates)
            .with(Capability::HotReload)
            .with(Capability::RemoteBuild)
    }

    /// Capabilities typical for React Native (bare)
    pub fn react_native() -> Self {
        Self::new()
            .with(Capability::BuildIos)
            .with(Capability::BuildAndroid)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::BuildFlavors)
            .with(Capability::UnitTests)
            .with(Capability::E2eTests)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::BuildNumbers)
            .with(Capability::HotReload)
            .with(Capability::CodeSigning)
    }

    /// Capabilities typical for native iOS
    pub fn native_ios() -> Self {
        Self::new()
            .with(Capability::BuildIos)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::ProfileBuild)
            .with(Capability::BuildFlavors)
            .with(Capability::CodeSigning)
            .with(Capability::UnitTests)
            .with(Capability::IntegrationTests)
            .with(Capability::Coverage)
            .with(Capability::Screenshots)
            .with(Capability::SimulatorScreenshots)
            .with(Capability::DeviceScreenshots)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::BuildNumbers)
            .with(Capability::IncrementalBuild)
    }

    /// Capabilities typical for Tauri
    pub fn tauri() -> Self {
        Self::new()
            .with(Capability::BuildMacos)
            .with(Capability::BuildWindows)
            .with(Capability::BuildLinux)
            .with(Capability::DebugBuild)
            .with(Capability::ReleaseBuild)
            .with(Capability::CodeSigning)
            .with(Capability::UnitTests)
            .with(Capability::E2eTests)
            .with(Capability::ReadVersion)
            .with(Capability::WriteVersion)
            .with(Capability::HotReload)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities_basic() {
        let mut caps = Capabilities::new();
        caps.add(Capability::BuildIos);
        caps.add(Capability::BuildAndroid);

        assert!(caps.has(Capability::BuildIos));
        assert!(caps.has(Capability::BuildAndroid));
        assert!(!caps.has(Capability::BuildWeb));
    }

    #[test]
    fn test_capabilities_builder() {
        let caps = Capabilities::new()
            .with(Capability::BuildIos)
            .with(Capability::UnitTests)
            .with(Capability::Coverage);

        assert!(caps.has(Capability::BuildIos));
        assert!(caps.has(Capability::UnitTests));
        assert!(caps.has(Capability::Coverage));
        assert!(caps.can_test());
    }

    #[test]
    fn test_capabilities_has_all() {
        let caps = Capabilities::flutter();

        assert!(caps.has_all(&[Capability::BuildIos, Capability::BuildAndroid]));
        assert!(!caps.has_all(&[Capability::BuildIos, Capability::RemoteBuild]));
    }

    #[test]
    fn test_capabilities_has_any() {
        let caps = Capabilities::new().with(Capability::BuildWeb);

        assert!(caps.has_any(&[Capability::BuildIos, Capability::BuildWeb]));
        assert!(!caps.has_any(&[Capability::BuildIos, Capability::BuildAndroid]));
    }

    #[test]
    fn test_flutter_capabilities() {
        let caps = Capabilities::flutter();

        assert!(caps.can_build_mobile());
        assert!(caps.can_build_desktop());
        assert!(caps.can_test());
        assert!(caps.has(Capability::HotReload));
        assert!(caps.has(Capability::BuildWeb));
    }

    #[test]
    fn test_expo_capabilities() {
        let caps = Capabilities::expo();

        assert!(caps.can_build_mobile());
        assert!(!caps.can_build_desktop());
        assert!(caps.has(Capability::OtaUpdates));
        assert!(caps.has(Capability::RemoteBuild));
    }
}
