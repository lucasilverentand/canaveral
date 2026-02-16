//! Windows code signing provider using signtool
//!
//! This module is only compiled on Windows.

use crate::error::{Result, SigningError};
use crate::identity::{SigningIdentity, SigningIdentityType};
use crate::provider::{
    SignOptions, SignatureInfo, SignatureStatus, SignerInfo, SigningProvider, VerifyOptions,
};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, instrument};

/// Windows signing provider using signtool.exe
pub struct WindowsProvider {
    /// Path to signtool.exe
    signtool_path: Option<String>,
}

impl WindowsProvider {
    /// Create a new Windows signing provider
    pub fn new() -> Self {
        Self {
            signtool_path: Self::find_signtool(),
        }
    }

    /// Find signtool.exe in Windows SDK locations
    fn find_signtool() -> Option<String> {
        // Common Windows SDK locations
        let sdk_paths = [
            r"C:\Program Files (x86)\Windows Kits\10\bin",
            r"C:\Program Files\Windows Kits\10\bin",
            r"C:\Program Files (x86)\Windows Kits\8.1\bin",
        ];

        for sdk_path in sdk_paths {
            let sdk_dir = Path::new(sdk_path);
            if sdk_dir.exists() {
                // Find the latest version directory
                if let Ok(entries) = std::fs::read_dir(sdk_dir) {
                    let mut versions: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .collect();

                    versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

                    for version in versions {
                        // Try x64 first, then x86
                        for arch in ["x64", "x86"] {
                            let signtool = version.path().join(arch).join("signtool.exe");
                            if signtool.exists() {
                                return Some(signtool.to_string_lossy().to_string());
                            }
                        }
                    }
                }
            }
        }

        // Try PATH
        if std::process::Command::new("signtool")
            .arg("/?")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some("signtool".to_string());
        }

        None
    }

    /// Get signtool path or return error
    fn get_signtool(&self) -> Result<&str> {
        self.signtool_path.as_deref().ok_or_else(|| SigningError::ToolNotFound {
            tool: "signtool.exe".to_string(),
            hint: "Install Windows SDK or add signtool.exe to PATH".to_string(),
        })
    }

    /// Parse signtool verify output for signature info
    fn parse_verify_output(output: &str) -> (SignatureStatus, Option<SignerInfo>) {
        let status = if output.contains("Successfully verified") {
            SignatureStatus::Valid
        } else if output.contains("No signature found") {
            SignatureStatus::NotSigned
        } else if output.contains("The signature is invalid") {
            SignatureStatus::Invalid
        } else if output.contains("expired") {
            SignatureStatus::Expired
        } else {
            SignatureStatus::Unknown
        };

        let signer = if status == SignatureStatus::Valid {
            // Parse "Issued to:" line
            output
                .lines()
                .find(|l| l.trim().starts_with("Issued to:"))
                .map(|line| {
                    let name = line.trim_start_matches("Issued to:").trim().to_string();
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

        (status, signer)
    }
}

impl Default for WindowsProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SigningProvider for WindowsProvider {
    fn name(&self) -> &str {
        "windows"
    }

    fn is_available(&self) -> bool {
        self.signtool_path.is_some()
    }

    #[instrument(skip(self), fields(provider = "windows"))]
    async fn list_identities(&self) -> Result<Vec<SigningIdentity>> {
        // Use certutil to list certificates in the Windows certificate store
        let output = Command::new("certutil")
            .args(["-store", "My"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::KeychainError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut identities = Vec::new();
        let mut current_name: Option<String> = None;
        let mut current_thumbprint: Option<String> = None;
        let mut is_code_signing = false;

        for line in stdout.lines() {
            let line = line.trim();

            // Subject line contains the certificate name
            if line.starts_with("Subject:") {
                // Extract CN from Subject
                if let Some(cn_start) = line.find("CN=") {
                    let cn_part = &line[cn_start + 3..];
                    let cn_end = cn_part.find(',').unwrap_or(cn_part.len());
                    current_name = Some(cn_part[..cn_end].to_string());
                }
            }
            // Cert Hash (sha1) is the thumbprint
            else if line.starts_with("Cert Hash(sha1):") {
                current_thumbprint = Some(
                    line.trim_start_matches("Cert Hash(sha1):")
                        .trim()
                        .replace(' ', "")
                );
            }
            // Check for Code Signing EKU
            else if line.contains("Code Signing") {
                is_code_signing = true;
            }
            // End of certificate entry
            else if line.starts_with("===============") {
                if let (Some(name), Some(thumbprint)) = (current_name.take(), current_thumbprint.take()) {
                    if is_code_signing {
                        let identity_type = if name.contains("EV ") {
                            SigningIdentityType::WindowsEV
                        } else {
                            SigningIdentityType::WindowsAuthenticode
                        };

                        let mut identity = SigningIdentity::new(
                            thumbprint.clone(),
                            name,
                            identity_type,
                        );
                        identity.fingerprint = Some(thumbprint);
                        identities.push(identity);
                    }
                }
                is_code_signing = false;
            }
        }

        info!(count = identities.len(), "Found Windows signing identities");
        Ok(identities)
    }

    async fn find_identity(&self, query: &str) -> Result<SigningIdentity> {
        let identities = self.list_identities().await?;

        let matches: Vec<_> = identities
            .into_iter()
            .filter(|id| {
                id.name.to_lowercase().contains(&query.to_lowercase())
                    || id.fingerprint.as_ref().map(|f| f.to_lowercase().contains(&query.to_lowercase())).unwrap_or(false)
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

    #[instrument(skip(self, identity, options), fields(provider = "windows", path = %artifact.display()))]
    async fn sign(
        &self,
        artifact: &Path,
        identity: &SigningIdentity,
        options: &SignOptions,
    ) -> Result<()> {
        let signtool = self.get_signtool()?;

        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        if options.dry_run {
            info!(
                "Dry run: would sign {} with Windows certificate {}",
                artifact.display(),
                identity.name
            );
            return Ok(());
        }

        let artifact_str = artifact.to_string_lossy();

        let mut args = vec!["sign"];

        // Use SHA256 by default
        let algorithm = options.algorithm.as_deref().unwrap_or("sha256");
        args.push("/fd");
        args.push(algorithm);

        // Certificate selection - by thumbprint if available
        if let Some(thumbprint) = &identity.fingerprint {
            args.push("/sha1");
            args.push(thumbprint);
        } else {
            // Fall back to subject name
            args.push("/n");
            args.push(&identity.name);
        }

        // Timestamp
        if options.timestamp {
            let timestamp_url = options.timestamp_url.as_deref()
                .unwrap_or("http://timestamp.digicert.com");
            args.push("/tr");
            args.push(timestamp_url);
            args.push("/td");
            args.push(algorithm);
        }

        // Description
        if let Some(desc) = &options.description {
            args.push("/d");
            args.push(desc);
        }

        // Description URL
        if let Some(url) = &options.description_url {
            args.push("/du");
            args.push(url);
        }

        // Verbose
        if options.verbose {
            args.push("/v");
        }

        args.push(&artifact_str);

        debug!("Running signtool with args: {:?}", args);

        let output = Command::new(signtool)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::ToolFailed {
                tool: "signtool".to_string(),
                reason: format!("{}\n{}", stdout, stderr),
            });
        }

        info!("Signed {} with Windows certificate {}", artifact.display(), identity.name);
        Ok(())
    }

    #[instrument(skip(self, options), fields(provider = "windows", path = %artifact.display()))]
    async fn verify(&self, artifact: &Path, options: &VerifyOptions) -> Result<SignatureInfo> {
        let signtool = self.get_signtool()?;

        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        let artifact_str = artifact.to_string_lossy();

        let mut args = vec!["verify", "/pa"];

        if options.verbose {
            args.push("/v");
        }

        args.push(&artifact_str);

        let output = Command::new(signtool)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("{}\n{}", stdout, stderr);

        let (status, signer) = Self::parse_verify_output(&combined);

        Ok(SignatureInfo {
            path: artifact.to_string_lossy().to_string(),
            status,
            signer,
            signed_at: None,
            timestamp_authority: None,
            notarized: None,
            stapled: None,
            algorithm: Some("Authenticode".to_string()),
            warnings: vec![],
            details: Some(combined),
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["exe", "dll", "sys", "msi", "msix", "appx", "cab", "cat", "ocx"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_verify_output_valid() {
        let output = r#"
Verifying: test.exe

Signature Index: 0 (Primary Signature)
Hash of file (sha256): ABC123...

Signing Certificate Chain:
    Issued to: My Company
    Issued by: DigiCert
    Expires:   2025-01-01

Successfully verified: test.exe
"#;

        let (status, signer) = WindowsProvider::parse_verify_output(output);
        assert_eq!(status, SignatureStatus::Valid);
        assert!(signer.is_some());
        assert_eq!(signer.unwrap().common_name, "My Company");
    }

    #[test]
    fn test_parse_verify_output_not_signed() {
        let output = "SignTool Error: No signature found.";
        let (status, signer) = WindowsProvider::parse_verify_output(output);
        assert_eq!(status, SignatureStatus::NotSigned);
        assert!(signer.is_none());
    }

    #[test]
    fn test_provider_creation() {
        let provider = WindowsProvider::new();
        assert_eq!(provider.name(), "windows");
    }
}
