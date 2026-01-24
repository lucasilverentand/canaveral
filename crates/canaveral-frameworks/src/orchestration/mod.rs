//! Workflow orchestration for framework operations
//!
//! The orchestrator coordinates multi-step workflows (build, sign, upload)
//! with proper error handling, hooks, retries, and structured output.
//! Designed for CI/CD first with full CLI support.

mod build;
mod config;

pub use build::BuildOrchestrator;
pub use config::OrchestratorConfig;

use std::path::Path;
use std::time::Instant;

use crate::artifacts::Artifact;
use crate::context::BuildContext;
use crate::error::{FrameworkError, Result};
use crate::output::{Output, OutputFormat};
use crate::registry::FrameworkRegistry;
use crate::traits::PrerequisiteStatus;

/// Main orchestrator for all framework operations
pub struct Orchestrator {
    /// Registry of available adapters
    registry: FrameworkRegistry,

    /// Configuration
    config: OrchestratorConfig,
}

impl Orchestrator {
    /// Create a new orchestrator with default registry
    pub fn new() -> Self {
        Self {
            registry: FrameworkRegistry::with_builtins(),
            config: OrchestratorConfig::default(),
        }
    }

    /// Create with custom registry
    pub fn with_registry(registry: FrameworkRegistry) -> Self {
        Self {
            registry,
            config: OrchestratorConfig::default(),
        }
    }

    /// Create with custom config
    pub fn with_config(config: OrchestratorConfig) -> Self {
        Self {
            registry: FrameworkRegistry::with_builtins(),
            config,
        }
    }

    /// Set configuration
    pub fn config(mut self, config: OrchestratorConfig) -> Self {
        self.config = config;
        self
    }

    /// Get reference to registry
    pub fn registry(&self) -> &FrameworkRegistry {
        &self.registry
    }

    /// Get mutable reference to registry
    pub fn registry_mut(&mut self) -> &mut FrameworkRegistry {
        &mut self.registry
    }

    // -------------------------------------------------------------------------
    // Build Operations
    // -------------------------------------------------------------------------

    /// Build a project
    pub async fn build(&self, ctx: &BuildContext) -> Result<BuildResult> {
        let start = Instant::now();

        // Resolve adapter
        let adapter = self.registry.resolve_build(&ctx.path, None)?;

        self.log_info(&format!(
            "Building {} for {} using {}",
            ctx.path.display(),
            ctx.platform.as_str(),
            adapter.name()
        ));

        // Check prerequisites
        if self.config.check_prerequisites {
            let status = adapter.check_prerequisites().await?;
            if !status.satisfied {
                return Err(self.prerequisites_error(&status));
            }
        }

        // Dry run check
        if ctx.dry_run {
            self.log_info("Dry run mode - skipping actual build");
            return Ok(BuildResult {
                success: true,
                artifacts: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
                adapter_id: adapter.id().to_string(),
                adapter_name: adapter.name().to_string(),
                warnings: vec!["Dry run - no artifacts produced".to_string()],
            });
        }

        // Execute build with retry logic
        let artifacts = self.execute_with_retry(
            || async { adapter.build(ctx).await },
            self.config.max_retries,
        ).await?;

        // Verify artifacts exist
        for artifact in &artifacts {
            if !artifact.path.exists() {
                return Err(FrameworkError::ArtifactNotFound {
                    expected_path: artifact.path.clone(),
                });
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.log_success(&format!(
            "Build completed in {}ms - {} artifact(s) produced",
            duration_ms,
            artifacts.len()
        ));

        Ok(BuildResult {
            success: true,
            artifacts,
            duration_ms,
            adapter_id: adapter.id().to_string(),
            adapter_name: adapter.name().to_string(),
            warnings: vec![],
        })
    }

    /// Build and return structured output
    pub async fn build_with_output(
        &self,
        ctx: &BuildContext,
        format: OutputFormat,
    ) -> (Output, i32) {
        match self.build(ctx).await {
            Ok(result) => {
                let mut output = Output::success("build", "Build completed successfully")
                    .with_duration(result.duration_ms)
                    .with_artifacts(result.artifacts.clone())
                    .with_output("adapter", &result.adapter_id)
                    .with_output("platform", ctx.platform.as_str());

                // Add artifact paths as outputs for CI
                for (i, artifact) in result.artifacts.iter().enumerate() {
                    let key = if i == 0 {
                        "artifact_path".to_string()
                    } else {
                        format!("artifact_path_{}", i)
                    };
                    output = output.with_output(key, artifact.path.to_string_lossy());
                }

                for warning in result.warnings {
                    output = output.with_warning(warning);
                }

                output.print(format);
                (output, 0)
            }
            Err(e) => {
                let output = Output::failure("build", e.to_string());
                output.print(format);
                (output, e.exit_code())
            }
        }
    }

    // -------------------------------------------------------------------------
    // Detect Operations
    // -------------------------------------------------------------------------

    /// Detect frameworks in a project
    pub fn detect(&self, path: &Path) -> DetectResult {
        let build = self.registry.detect_build(path);
        let test = self.registry.detect_test(path);
        let ota = self.registry.detect_ota(path);

        DetectResult { build, test, ota }
    }

    /// Detect and return structured output
    pub fn detect_with_output(&self, path: &Path, format: OutputFormat) -> Output {
        let result = self.detect(path);

        let mut output = Output::success("detect", "Framework detection completed");

        if let Some(best) = result.build.first() {
            output = output
                .with_output("framework", &best.adapter_id)
                .with_output("framework_name", &best.adapter_name)
                .with_output("confidence", best.detection.confidence().to_string());
        }

        // Add all detected frameworks as metadata
        let frameworks: Vec<_> = result
            .build
            .iter()
            .map(|d| serde_json::json!({
                "id": d.adapter_id,
                "name": d.adapter_name,
                "confidence": d.detection.confidence()
            }))
            .collect();

        output = output.with_metadata("frameworks", serde_json::json!(frameworks));

        output.print(format);
        output
    }

    // -------------------------------------------------------------------------
    // Helper Methods
    // -------------------------------------------------------------------------

    /// Execute an async operation with retry logic
    async fn execute_with_retry<F, Fut, T>(&self, f: F, max_retries: u32) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_error = None;

        for attempt in 0..=max_retries {
            match f().await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    if e.is_retryable() && attempt < max_retries {
                        let delay = self.config.retry_delay_ms * (attempt + 1) as u64;
                        self.log_warn(&format!(
                            "Attempt {} failed ({}), retrying in {}ms...",
                            attempt + 1,
                            e,
                            delay
                        ));
                        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                        last_error = Some(e);
                    } else {
                        return Err(e);
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    fn prerequisites_error(&self, status: &PrerequisiteStatus) -> FrameworkError {
        let missing: Vec<_> = status
            .tools
            .iter()
            .filter(|t| !t.available)
            .map(|t| format!("{}: {}", t.name, t.install_hint))
            .collect();

        FrameworkError::Context {
            context: "prerequisites check".to_string(),
            message: format!("Missing tools:\n  {}", missing.join("\n  ")),
        }
    }

    fn log_info(&self, msg: &str) {
        if !self.config.quiet {
            tracing::info!("{}", msg);
            if !self.config.json_output {
                eprintln!("ℹ {}", msg);
            }
        }
    }

    fn log_success(&self, msg: &str) {
        if !self.config.quiet {
            tracing::info!("{}", msg);
            if !self.config.json_output {
                eprintln!("✓ {}", msg);
            }
        }
    }

    fn log_warn(&self, msg: &str) {
        tracing::warn!("{}", msg);
        if !self.config.quiet && !self.config.json_output {
            eprintln!("⚠ {}", msg);
        }
    }
}

impl Default for Orchestrator {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of a build operation
#[derive(Debug, Clone)]
pub struct BuildResult {
    pub success: bool,
    pub artifacts: Vec<Artifact>,
    pub duration_ms: u64,
    pub adapter_id: String,
    pub adapter_name: String,
    pub warnings: Vec<String>,
}

/// Result of framework detection
#[derive(Debug)]
pub struct DetectResult {
    pub build: Vec<crate::detection::DetectionResult>,
    pub test: Vec<crate::detection::DetectionResult>,
    pub ota: Vec<crate::detection::DetectionResult>,
}

impl DetectResult {
    /// Get the best build framework
    pub fn best_build(&self) -> Option<&crate::detection::DetectionResult> {
        self.build.first()
    }

    /// Check if any framework was detected
    pub fn has_detections(&self) -> bool {
        !self.build.is_empty() || !self.test.is_empty() || !self.ota.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orchestrator_creation() {
        let orchestrator = Orchestrator::new();
        assert!(orchestrator.registry().build_adapter_ids().is_empty());
    }

    #[test]
    fn test_detect_result() {
        let result = DetectResult {
            build: vec![],
            test: vec![],
            ota: vec![],
        };

        assert!(result.best_build().is_none());
        assert!(!result.has_detections());
    }
}
