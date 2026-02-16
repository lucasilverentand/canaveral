//! Build orchestration
//!
//! Specialized orchestrator for build workflows with hooks, signing integration,
//! and artifact management.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tracing::{info, instrument};

use crate::artifacts::Artifact;
use crate::context::BuildContext;
use crate::error::{FrameworkError, Result};
use crate::output::Output;
use crate::registry::FrameworkRegistry;
use crate::traits::{BuildAdapter, Platform};

use super::config::OrchestratorConfig;

/// Build-specific orchestrator
pub struct BuildOrchestrator {
    registry: Arc<FrameworkRegistry>,
    config: OrchestratorConfig,
    hooks: BuildHooks,
}

impl BuildOrchestrator {
    /// Create a new build orchestrator
    pub fn new(registry: Arc<FrameworkRegistry>) -> Self {
        Self {
            registry,
            config: OrchestratorConfig::from_env(),
            hooks: BuildHooks::default(),
        }
    }

    /// Set configuration
    pub fn with_config(mut self, config: OrchestratorConfig) -> Self {
        self.config = config;
        self
    }

    /// Set hooks
    pub fn with_hooks(mut self, hooks: BuildHooks) -> Self {
        self.hooks = hooks;
        self
    }

    /// Build for a single platform
    #[instrument(skip(self, ctx), fields(path = %ctx.path.display(), platform = %ctx.platform.as_str()))]
    pub async fn build(&self, ctx: &BuildContext) -> Result<BuildOutput> {
        let start = Instant::now();

        // Resolve adapter
        let adapter = self.resolve_adapter(&ctx.path, None)?;

        // Pre-build hook
        self.hooks.run_pre_build(ctx).await?;

        // Check prerequisites
        if self.config.check_prerequisites {
            self.check_prerequisites(&adapter).await?;
        }

        // Execute build
        let artifacts = if ctx.dry_run {
            vec![]
        } else {
            adapter.build(ctx).await?
        };

        // Post-build hook
        self.hooks.run_post_build(ctx, &artifacts).await?;

        let duration_ms = start.elapsed().as_millis() as u64;

        info!(
            adapter = adapter.id(),
            artifact_count = artifacts.len(),
            duration_ms,
            dry_run = ctx.dry_run,
            "build completed"
        );

        Ok(BuildOutput {
            success: true,
            platform: ctx.platform,
            artifacts,
            duration_ms,
            adapter_id: adapter.id().to_string(),
            dry_run: ctx.dry_run,
        })
    }

    /// Build for multiple platforms
    #[instrument(skip(self), fields(path = %path.display(), platform_count = platforms.len()))]
    pub async fn build_all(&self, path: &Path, platforms: &[Platform]) -> Result<MultiBuildOutput> {
        let start = Instant::now();
        let mut outputs = Vec::new();
        let mut all_artifacts = Vec::new();
        let mut errors = Vec::new();

        for platform in platforms {
            let ctx = BuildContext::from_env(path, *platform);

            match self.build(&ctx).await {
                Ok(output) => {
                    all_artifacts.extend(output.artifacts.clone());
                    outputs.push(output);
                }
                Err(e) => {
                    errors.push((*platform, e.to_string()));
                    if !self.config.ci {
                        // In local mode, stop on first error
                        break;
                    }
                }
            }
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(MultiBuildOutput {
            success: errors.is_empty(),
            outputs,
            all_artifacts,
            errors,
            duration_ms,
        })
    }

    /// Build and output results
    pub async fn build_with_output(&self, ctx: &BuildContext) -> (Output, i32) {
        let format = self.config.effective_output_format();

        match self.build(ctx).await {
            Ok(result) => {
                let mut output = Output::success(
                    "build",
                    format!(
                        "Built {} for {}",
                        result.adapter_id,
                        result.platform.as_str()
                    ),
                )
                .with_duration(result.duration_ms)
                .with_artifacts(result.artifacts.clone())
                .with_output("platform", result.platform.as_str())
                .with_output("adapter", &result.adapter_id);

                // Primary artifact path for CI
                if let Some(artifact) = result.artifacts.first() {
                    output = output
                        .with_output("artifact_path", artifact.path.to_string_lossy())
                        .with_output("artifact_kind", format!("{:?}", artifact.kind).to_lowercase());

                    if let Some(ref sha) = artifact.sha256 {
                        output = output.with_output("artifact_sha256", sha);
                    }
                }

                if result.dry_run {
                    output = output.with_warning("Dry run - no artifacts produced");
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

    fn resolve_adapter(
        &self,
        path: &Path,
        adapter_id: Option<&str>,
    ) -> Result<Arc<dyn BuildAdapter>> {
        self.registry.resolve_build(path, adapter_id)
    }

    async fn check_prerequisites(&self, adapter: &Arc<dyn BuildAdapter>) -> Result<()> {
        let status = adapter.check_prerequisites().await?;

        if !status.satisfied {
            let missing: Vec<_> = status
                .tools
                .iter()
                .filter(|t| !t.available)
                .collect();

            if !missing.is_empty() {
                let details: Vec<_> = missing
                    .iter()
                    .map(|t| format!("  - {}: {}", t.name, t.install_hint))
                    .collect();

                return Err(FrameworkError::Context {
                    context: "prerequisites".to_string(),
                    message: format!(
                        "Missing required tools:\n{}",
                        details.join("\n")
                    ),
                });
            }
        }

        // Log warnings
        for warning in &status.warnings {
            tracing::warn!("{}", warning);
        }

        Ok(())
    }
}

/// Build output for a single platform
#[derive(Debug, Clone)]
pub struct BuildOutput {
    pub success: bool,
    pub platform: Platform,
    pub artifacts: Vec<Artifact>,
    pub duration_ms: u64,
    pub adapter_id: String,
    pub dry_run: bool,
}

/// Build output for multiple platforms
#[derive(Debug, Clone)]
pub struct MultiBuildOutput {
    pub success: bool,
    pub outputs: Vec<BuildOutput>,
    pub all_artifacts: Vec<Artifact>,
    pub errors: Vec<(Platform, String)>,
    pub duration_ms: u64,
}

/// Build hooks for extensibility
#[derive(Default)]
pub struct BuildHooks {
    pre_build: Option<Box<dyn Fn(&BuildContext) -> Result<()> + Send + Sync>>,
    post_build: Option<Box<dyn Fn(&BuildContext, &[Artifact]) -> Result<()> + Send + Sync>>,
}

impl BuildHooks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_pre_build<F>(mut self, f: F) -> Self
    where
        F: Fn(&BuildContext) -> Result<()> + Send + Sync + 'static,
    {
        self.pre_build = Some(Box::new(f));
        self
    }

    pub fn on_post_build<F>(mut self, f: F) -> Self
    where
        F: Fn(&BuildContext, &[Artifact]) -> Result<()> + Send + Sync + 'static,
    {
        self.post_build = Some(Box::new(f));
        self
    }

    async fn run_pre_build(&self, ctx: &BuildContext) -> Result<()> {
        if let Some(ref hook) = self.pre_build {
            hook(ctx)?;
        }
        Ok(())
    }

    async fn run_post_build(&self, ctx: &BuildContext, artifacts: &[Artifact]) -> Result<()> {
        if let Some(ref hook) = self.post_build {
            hook(ctx, artifacts)?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for BuildHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BuildHooks")
            .field("pre_build", &self.pre_build.is_some())
            .field("post_build", &self.post_build.is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_hooks() {
        let hooks = BuildHooks::new()
            .on_pre_build(|_ctx| Ok(()))
            .on_post_build(|_ctx, _artifacts| Ok(()));

        assert!(hooks.pre_build.is_some());
        assert!(hooks.post_build.is_some());
    }
}
