//! GPG signing provider

use crate::error::{Result, SigningError};
use crate::identity::{SigningIdentity, SigningIdentityType};
use crate::provider::{
    SignOptions, SignatureInfo, SignatureStatus, SignerInfo, SigningProvider, VerifyOptions,
};
use chrono::{DateTime, TimeZone, Utc};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, instrument};

/// GPG signing provider
pub struct GpgProvider {
    /// Path to gpg binary
    gpg_path: String,
}

impl GpgProvider {
    /// Create a new GPG signing provider
    pub fn new() -> Self {
        Self {
            gpg_path: "gpg".to_string(),
        }
    }

    /// Create with custom gpg path
    pub fn with_path(gpg_path: impl Into<String>) -> Self {
        Self {
            gpg_path: gpg_path.into(),
        }
    }

    /// Run gpg command and return output
    async fn run_gpg(&self, args: &[&str]) -> Result<String> {
        debug!("Running gpg with args: {:?}", args);

        let output = Command::new(&self.gpg_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(SigningError::ToolFailed {
                tool: "gpg".to_string(),
                reason: if stderr.is_empty() { stdout } else { stderr },
            });
        }

        Ok(stdout)
    }

    /// Parse GPG key listing output
    fn parse_key_listing(output: &str) -> Vec<SigningIdentity> {
        let mut identities = Vec::new();
        let mut current_fingerprint: Option<String> = None;
        let mut current_name: Option<String> = None;
        let mut current_email: Option<String> = None;
        let mut created_at: Option<DateTime<Utc>> = None;
        let mut expires_at: Option<DateTime<Utc>> = None;

        for line in output.lines() {
            let line = line.trim();

            // Secret key line: "sec   rsa4096/KEYID 2021-01-01 [SC] [expires: 2024-01-01]"
            if line.starts_with("sec") || line.starts_with("pub") {
                // Save previous key if exists
                if let (Some(fp), Some(name)) = (&current_fingerprint, &current_name) {
                    let mut identity =
                        SigningIdentity::new(fp.clone(), name.clone(), SigningIdentityType::Gpg);
                    identity.fingerprint = Some(fp.clone());
                    identity.subject = current_email.clone();
                    identity.created_at = created_at;
                    identity.expires_at = expires_at;
                    identity.is_valid = expires_at.map(|e| e > Utc::now()).unwrap_or(true);
                    identities.push(identity);
                }

                // Parse dates from the line
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    // Parse creation date
                    if let Ok(date) = chrono::NaiveDate::parse_from_str(parts[2], "%Y-%m-%d") {
                        created_at =
                            Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
                    }
                }

                // Parse expiration if present
                if let Some(exp_str) = line.split("[expires: ").nth(1) {
                    if let Some(date_str) = exp_str.split(']').next() {
                        if let Ok(date) = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                            expires_at =
                                Some(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()));
                        }
                    }
                } else {
                    expires_at = None;
                }

                current_fingerprint = None;
                current_name = None;
                current_email = None;
            }
            // Fingerprint line (40 hex chars with optional spaces)
            else if line.len() >= 40
                && line
                    .chars()
                    .all(|c| c.is_ascii_hexdigit() || c.is_whitespace())
            {
                current_fingerprint = Some(line.replace(' ', ""));
            }
            // User ID line: "uid           [ultimate] Name <email@example.com>"
            else if line.starts_with("uid") && current_name.is_none() {
                // Extract name and email
                if let Some(bracket_pos) = line.find(']') {
                    let user_part = line[bracket_pos + 1..].trim();
                    if let Some(email_start) = user_part.find('<') {
                        current_name = Some(user_part[..email_start].trim().to_string());
                        if let Some(email_end) = user_part.find('>') {
                            current_email = Some(user_part[email_start + 1..email_end].to_string());
                        }
                    } else {
                        current_name = Some(user_part.to_string());
                    }
                }
            }
        }

        // Don't forget the last key
        if let (Some(fp), Some(name)) = (current_fingerprint, current_name) {
            let mut identity = SigningIdentity::new(fp.clone(), name, SigningIdentityType::Gpg);
            identity.fingerprint = Some(fp);
            identity.subject = current_email;
            identity.created_at = created_at;
            identity.expires_at = expires_at;
            identity.is_valid = expires_at.map(|e| e > Utc::now()).unwrap_or(true);
            identities.push(identity);
        }

        identities
    }
}

impl Default for GpgProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SigningProvider for GpgProvider {
    fn name(&self) -> &str {
        "gpg"
    }

    fn is_available(&self) -> bool {
        std::process::Command::new(&self.gpg_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    #[instrument(skip(self), fields(provider = "gpg"))]
    async fn list_identities(&self) -> Result<Vec<SigningIdentity>> {
        let output = self
            .run_gpg(&["--list-secret-keys", "--keyid-format=long"])
            .await?;
        let identities = Self::parse_key_listing(&output);
        info!(count = identities.len(), "Found GPG signing identities");
        Ok(identities)
    }

    async fn find_identity(&self, query: &str) -> Result<SigningIdentity> {
        let identities = self.list_identities().await?;

        let matches: Vec<_> = identities
            .into_iter()
            .filter(|id| {
                id.name.to_lowercase().contains(&query.to_lowercase())
                    || id
                        .fingerprint
                        .as_ref()
                        .map(|f| f.contains(query))
                        .unwrap_or(false)
                    || id
                        .subject
                        .as_ref()
                        .map(|s| s.to_lowercase().contains(&query.to_lowercase()))
                        .unwrap_or(false)
                    || id.id.contains(query)
            })
            .collect();

        match matches.len() {
            0 => Err(SigningError::IdentityNotFound(query.to_string())),
            1 => Ok(matches.into_iter().next().unwrap()),
            _ => Err(SigningError::AmbiguousIdentity {
                query: query.to_string(),
            }),
        }
    }

    #[instrument(skip(self, identity, options), fields(provider = "gpg", path = %artifact.display()))]
    async fn sign(
        &self,
        artifact: &Path,
        identity: &SigningIdentity,
        options: &SignOptions,
    ) -> Result<()> {
        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        if options.dry_run {
            info!(
                "Dry run: would sign {} with GPG key {}",
                artifact.display(),
                identity.name
            );
            return Ok(());
        }

        let key_id = identity.fingerprint.as_ref().unwrap_or(&identity.id);
        let artifact_str = artifact.to_string_lossy();

        let mut args = vec!["--local-user", key_id];

        // Detached signature
        if options.detached {
            args.push("--detach-sign");
        } else {
            args.push("--sign");
        }

        // ASCII armor
        if options.armor {
            args.push("--armor");
        }

        // Passphrase handling
        if let Some(passphrase) = &options.passphrase {
            args.push("--pinentry-mode");
            args.push("loopback");
            args.push("--passphrase");
            args.push(passphrase);
        }

        // Output file (for detached, use .sig or .asc extension)
        let output_path;
        if options.detached {
            let ext = if options.armor { "asc" } else { "sig" };
            output_path = format!("{}.{}", artifact_str, ext);
            args.push("--output");
            args.push(&output_path);
        }

        args.push(&artifact_str);

        info!(
            "Signing {} with GPG key {}",
            artifact.display(),
            identity.name
        );
        self.run_gpg(&args).await?;

        Ok(())
    }

    #[instrument(skip(self, _options), fields(provider = "gpg", path = %artifact.display()))]
    async fn verify(&self, artifact: &Path, _options: &VerifyOptions) -> Result<SignatureInfo> {
        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        let artifact_str = artifact.to_string_lossy();

        // Check for detached signature first
        let sig_path_asc = format!("{}.asc", artifact_str);
        let sig_path_sig = format!("{}.sig", artifact_str);

        let args = if Path::new(&sig_path_asc).exists() {
            vec!["--verify", &sig_path_asc, &artifact_str]
        } else if Path::new(&sig_path_sig).exists() {
            vec!["--verify", &sig_path_sig, &artifact_str]
        } else {
            vec!["--verify", &artifact_str]
        };

        let output = Command::new(&self.gpg_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let status = if output.status.success() {
            SignatureStatus::Valid
        } else if stderr.contains("No signature") || stderr.contains("not a detached signature") {
            SignatureStatus::NotSigned
        } else if stderr.contains("BAD signature") {
            SignatureStatus::Invalid
        } else if stderr.contains("expired") {
            SignatureStatus::Expired
        } else if stderr.contains("revoked") {
            SignatureStatus::Revoked
        } else {
            SignatureStatus::Unknown
        };

        // Parse signer info from output
        let signer = if status == SignatureStatus::Valid {
            // Look for "Good signature from" line
            stderr
                .lines()
                .find(|l| l.contains("Good signature from"))
                .map(|line| {
                    let name = line.split('"').nth(1).unwrap_or("Unknown").to_string();

                    SignerInfo {
                        common_name: name,
                        organization: None,
                        team_id: None,
                        fingerprint: None,
                        serial_number: None,
                        expires_at: None,
                        certificate_valid: true,
                    }
                })
        } else {
            None
        };

        // Parse timestamp if available
        let signed_at = stderr
            .lines()
            .find(|l| l.contains("Signature made"))
            .and(None);

        Ok(SignatureInfo {
            path: artifact.to_string_lossy().to_string(),
            status,
            signer,
            signed_at,
            timestamp_authority: None,
            notarized: None,
            stapled: None,
            algorithm: Some("GPG".to_string()),
            warnings: vec![],
            details: Some(stderr),
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        // GPG can sign any file
        &["*"]
    }

    fn supports_file(&self, _path: &Path) -> bool {
        // GPG can sign any file
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_listing() {
        let output = r#"
sec   rsa4096/ABCD1234EFGH5678 2021-01-15 [SC] [expires: 2025-01-15]
      1234567890ABCDEF1234567890ABCDEF12345678
uid           [ultimate] Test User <test@example.com>
ssb   rsa4096/IJKL9012MNOP3456 2021-01-15 [E] [expires: 2025-01-15]
"#;

        let identities = GpgProvider::parse_key_listing(output);
        assert_eq!(identities.len(), 1);

        let id = &identities[0];
        assert_eq!(id.name, "Test User");
        assert_eq!(id.subject, Some("test@example.com".to_string()));
        assert!(id.fingerprint.is_some());
    }

    #[test]
    fn test_provider_creation() {
        let provider = GpgProvider::new();
        assert_eq!(provider.name(), "gpg");
    }
}
