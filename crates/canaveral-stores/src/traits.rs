//! Store adapter traits

use crate::error::Result;
use crate::types::*;
use std::path::Path;

/// Trait for app store adapters
///
/// Implementations provide upload and management capabilities
/// for specific app stores.
#[async_trait::async_trait]
pub trait StoreAdapter: Send + Sync {
    /// Get the store name
    fn name(&self) -> &str;

    /// Get the store type
    fn store_type(&self) -> StoreType;

    /// Check if the adapter is properly configured and available
    fn is_available(&self) -> bool;

    /// Validate an artifact before upload
    ///
    /// Checks that the artifact is valid for this store and extracts
    /// app information.
    async fn validate_artifact(&self, path: &Path) -> Result<ValidationResult>;

    /// Upload an artifact to the store
    ///
    /// Returns upload result with build ID and status.
    async fn upload(&self, path: &Path, options: &UploadOptions) -> Result<UploadResult>;

    /// Get the status of a specific build
    async fn get_build_status(&self, build_id: &str) -> Result<BuildStatus>;

    /// List recent builds for an app
    async fn list_builds(&self, limit: Option<usize>) -> Result<Vec<Build>>;

    /// Get supported file extensions for this store
    fn supported_extensions(&self) -> &[&str];
}

/// Trait for stores that support notarization (Apple)
#[async_trait::async_trait]
pub trait NotarizationSupport: StoreAdapter {
    /// Submit an artifact for notarization
    async fn submit_for_notarization(&self, path: &Path) -> Result<String>;

    /// Check notarization status
    async fn check_notarization_status(&self, submission_id: &str) -> Result<NotarizationResult>;

    /// Wait for notarization to complete
    async fn wait_for_notarization(
        &self,
        submission_id: &str,
        timeout_secs: Option<u64>,
    ) -> Result<NotarizationResult>;

    /// Staple notarization ticket to artifact
    async fn staple(&self, path: &Path) -> Result<()>;

    /// Full notarization workflow: submit, wait, staple
    async fn notarize(&self, path: &Path, timeout_secs: Option<u64>) -> Result<NotarizationResult> {
        let submission_id = self.submit_for_notarization(path).await?;
        let result = self
            .wait_for_notarization(&submission_id, timeout_secs)
            .await?;

        if result.status == NotarizationStatus::Accepted {
            self.staple(path).await?;
        }

        Ok(result)
    }
}

/// Trait for stores that support staged rollouts
#[async_trait::async_trait]
pub trait StagedRolloutSupport: StoreAdapter {
    /// Update rollout percentage for a build
    async fn update_rollout(&self, build_id: &str, percentage: f64) -> Result<()>;

    /// Halt a staged rollout
    async fn halt_rollout(&self, build_id: &str) -> Result<()>;

    /// Complete a staged rollout (100%)
    async fn complete_rollout(&self, build_id: &str) -> Result<()>;
}

/// Trait for stores that support release tracks/channels
#[async_trait::async_trait]
pub trait TrackSupport: StoreAdapter {
    /// List available tracks/channels
    async fn list_tracks(&self) -> Result<Vec<String>>;

    /// Promote a build from one track to another
    async fn promote_build(&self, build_id: &str, from_track: &str, to_track: &str) -> Result<()>;
}
