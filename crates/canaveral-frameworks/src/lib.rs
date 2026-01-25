//! Canaveral Frameworks - Framework-agnostic build, test, and release adapters
//!
//! This crate provides a unified interface for building, testing, and releasing
//! applications regardless of the underlying framework (Flutter, Expo, React Native,
//! native iOS/Android, Tauri, Electron, etc.).
//!
//! # Philosophy
//!
//! "One interface, any toolchain"
//!
//! Users learn canaveral once, use it everywhere. The framework adapter handles
//! the translation to the underlying tool's CLI, while canaveral owns the workflow.
//!
//! # CI/CD First
//!
//! Designed for headless operation in CI/CD pipelines:
//! - Environment variable configuration
//! - Structured JSON output for parsing
//! - Idempotent operations where possible
//! - Clear exit codes and error messages
//! - Full CLI support for local development

pub mod artifacts;
pub mod capabilities;
pub mod context;
pub mod detection;
pub mod error;
pub mod frameworks;
pub mod orchestration;
pub mod output;
pub mod registry;
pub mod screenshots;
pub mod testing;
pub mod traits;

pub use artifacts::{Artifact, ArtifactKind, ArtifactMetadata};
pub use capabilities::{Capabilities, Capability};
pub use context::{BuildContext, ScreenshotContext, TestContext};
pub use detection::{Detection, FrameworkDetector};
pub use error::{FrameworkError, Result};
pub use orchestration::{BuildOrchestrator, Orchestrator, OrchestratorConfig};
pub use output::{Output, OutputFormat};
pub use registry::FrameworkRegistry;
pub use screenshots::{
    AppStoreScreenSize, DeviceConfig, DeviceManager, FrameConfig, FrameTemplate,
    PlayStoreScreenSize, ScreenConfig, ScreenshotCapture, ScreenshotConfig, ScreenshotFramer,
    ScreenshotResult, ScreenshotSession,
};
pub use testing::{ReportGenerator, TestRunner, TestRunnerConfig};
pub use traits::{
    BuildAdapter, DistributeAdapter, OtaAdapter, ScreenshotAdapter, TestAdapter, TestReport,
    TestCase, TestStatus, TestSuite, VersionAdapter,
};
