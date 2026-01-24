//! Apple notarization using notarytool
//!
//! Provides notarization support for macOS apps using Apple's `notarytool`.

use crate::error::{Result, StoreError};
use crate::types::*;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Apple notarization client using notarytool
pub struct Notarizer {
    /// App Store Connect API Key ID
    api_key_id: String,

    /// API Key Issuer ID
    api_issuer_id: String,

    /// Path to .p8 key file or key contents
    api_key: String,

    /// Team ID (optional, for disambiguation)
    team_id: Option<String>,

    /// Whether to automatically staple after notarization
    auto_staple: bool,
}

impl Notarizer {
    /// Create a new notarizer with API key credentials
    pub fn new(config: &AppleStoreConfig) -> Result<Self> {
        // Verify notarytool is available
        if !Self::is_notarytool_available() {
            return Err(StoreError::ToolNotFound(
                "notarytool (part of Xcode) is required for notarization".to_string(),
            ));
        }

        Ok(Self {
            api_key_id: config.api_key_id.clone(),
            api_issuer_id: config.api_issuer_id.clone(),
            api_key: config.api_key.clone(),
            team_id: config.team_id.clone(),
            auto_staple: config.staple,
        })
    }

    /// Check if notarytool is available
    fn is_notarytool_available() -> bool {
        std::process::Command::new("xcrun")
            .args(["notarytool", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Build base notarytool command with credentials
    fn build_command(&self) -> Command {
        let mut cmd = Command::new("xcrun");
        cmd.arg("notarytool");

        // Add API key credentials
        cmd.args(["--key-id", &self.api_key_id]);
        cmd.args(["--issuer", &self.api_issuer_id]);

        // Key can be a file path or the key contents
        if Path::new(&self.api_key).exists() {
            cmd.args(["--key", &self.api_key]);
        } else {
            // Write key to temp file
            // Note: In production, consider using --keychain-profile instead
            cmd.args(["--key", &self.api_key]);
        }

        if let Some(team_id) = &self.team_id {
            cmd.args(["--team-id", team_id]);
        }

        cmd
    }

    /// Submit a file for notarization
    pub async fn submit(&self, path: &Path) -> Result<String> {
        info!("Submitting {} for notarization", path.display());

        let mut cmd = self.build_command();
        cmd.args(["submit", path.to_str().unwrap()]);
        cmd.args(["--output-format", "json"]);

        let output = cmd
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("notarytool submit failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        debug!("notarytool submit stdout: {}", stdout);
        if !stderr.is_empty() {
            debug!("notarytool submit stderr: {}", stderr);
        }

        if !output.status.success() {
            return Err(StoreError::NotarizationFailed(format!(
                "Submission failed: {}",
                stderr
            )));
        }

        // Parse JSON output for submission ID
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| StoreError::NotarizationFailed(format!("Failed to parse response: {}", e)))?;

        let submission_id = json
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| StoreError::NotarizationFailed("No submission ID in response".to_string()))?;

        info!("Notarization submitted: {}", submission_id);
        Ok(submission_id.to_string())
    }

    /// Check the status of a notarization submission
    pub async fn status(&self, submission_id: &str) -> Result<NotarizationResult> {
        debug!("Checking notarization status for {}", submission_id);

        let mut cmd = self.build_command();
        cmd.args(["info", submission_id]);
        cmd.args(["--output-format", "json"]);

        let output = cmd
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("notarytool info failed: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StoreError::NotarizationFailed(format!(
                "Status check failed: {}",
                stderr
            )));
        }

        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| StoreError::NotarizationFailed(format!("Failed to parse response: {}", e)))?;

        let status_str = json
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let status = match status_str {
            "In Progress" => NotarizationStatus::InProgress,
            "Accepted" => NotarizationStatus::Accepted,
            "Invalid" => NotarizationStatus::Invalid,
            "Rejected" => NotarizationStatus::Rejected,
            _ => NotarizationStatus::InProgress,
        };

        Ok(NotarizationResult {
            submission_id: submission_id.to_string(),
            status,
            log_url: None,
            timestamp: chrono::Utc::now(),
            issues: Vec::new(),
        })
    }

    /// Get the notarization log for a submission
    pub async fn get_log(&self, submission_id: &str) -> Result<String> {
        let mut cmd = self.build_command();
        cmd.args(["log", submission_id]);

        let output = cmd
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("notarytool log failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StoreError::NotarizationFailed(format!(
                "Failed to get log: {}",
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Wait for notarization to complete, polling periodically
    pub async fn wait(
        &self,
        submission_id: &str,
        timeout_secs: Option<u64>,
    ) -> Result<NotarizationResult> {
        info!("Waiting for notarization to complete...");

        let timeout = timeout_secs.unwrap_or(3600); // Default 1 hour
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_secs(30);

        loop {
            if start.elapsed().as_secs() > timeout {
                return Err(StoreError::Timeout(format!(
                    "Notarization timed out after {} seconds",
                    timeout
                )));
            }

            let result = self.status(submission_id).await?;

            match result.status {
                NotarizationStatus::InProgress => {
                    debug!("Still in progress, waiting...");
                    tokio::time::sleep(poll_interval).await;
                }
                NotarizationStatus::Accepted => {
                    info!("Notarization accepted!");
                    return Ok(result);
                }
                NotarizationStatus::Invalid | NotarizationStatus::Rejected => {
                    // Try to get the log for more details
                    if let Ok(log) = self.get_log(submission_id).await {
                        warn!("Notarization failed. Log:\n{}", log);
                    }
                    return Err(StoreError::NotarizationFailed(format!(
                        "Notarization {:?}",
                        result.status
                    )));
                }
            }
        }
    }

    /// Staple the notarization ticket to an artifact
    pub async fn staple(&self, path: &Path) -> Result<()> {
        info!("Stapling notarization ticket to {}", path.display());

        let output = Command::new("xcrun")
            .args(["stapler", "staple", path.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("stapler failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StoreError::NotarizationFailed(format!(
                "Stapling failed: {}",
                stderr
            )));
        }

        info!("Stapling complete");
        Ok(())
    }

    /// Verify that an artifact has a valid notarization staple
    pub async fn verify_staple(&self, path: &Path) -> Result<bool> {
        let output = Command::new("xcrun")
            .args(["stapler", "validate", path.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("stapler validate failed: {}", e)))?;

        Ok(output.status.success())
    }

    /// Full notarization workflow: submit, wait, staple
    pub async fn notarize(
        &self,
        path: &Path,
        timeout_secs: Option<u64>,
    ) -> Result<NotarizationResult> {
        // Submit
        let submission_id = self.submit(path).await?;

        // Wait for completion
        let result = self.wait(&submission_id, timeout_secs).await?;

        // Staple if successful and auto_staple is enabled
        if result.status == NotarizationStatus::Accepted && self.auto_staple {
            self.staple(path).await?;
        }

        Ok(result)
    }

    /// List recent notarization submissions
    pub async fn history(&self, limit: Option<usize>) -> Result<Vec<NotarizationResult>> {
        let mut cmd = self.build_command();
        cmd.arg("history");
        cmd.args(["--output-format", "json"]);

        let output = cmd
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("notarytool history failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(StoreError::NotarizationFailed(format!(
                "History failed: {}",
                stderr
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let json: serde_json::Value = serde_json::from_str(&stdout)
            .map_err(|e| StoreError::NotarizationFailed(format!("Failed to parse response: {}", e)))?;

        let mut results = Vec::new();

        if let Some(submissions) = json.get("submissionHistory").and_then(|v| v.as_array()) {
            for submission in submissions.iter().take(limit.unwrap_or(100)) {
                let id = submission
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                let status_str = submission
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown");

                let status = match status_str {
                    "Accepted" => NotarizationStatus::Accepted,
                    "Invalid" => NotarizationStatus::Invalid,
                    "Rejected" => NotarizationStatus::Rejected,
                    _ => NotarizationStatus::InProgress,
                };

                results.push(NotarizationResult {
                    submission_id: id,
                    status,
                    log_url: None,
                    timestamp: chrono::Utc::now(),
                    issues: Vec::new(),
                });
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notarytool_available_check() {
        // This just tests the check doesn't panic
        let _ = Notarizer::is_notarytool_available();
    }
}
