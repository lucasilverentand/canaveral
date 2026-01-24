//! macOS code signing provider using codesign and related tools

use crate::error::{Result, SigningError};
use crate::identity::{SigningIdentity, SigningIdentityType};
use crate::provider::{
    SignOptions, SignatureInfo, SignatureStatus, SignerInfo, SigningProvider, VerifyOptions,
};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

/// macOS signing provider using codesign, productsign, and notarytool
pub struct MacOSProvider {
    /// Path to codesign binary
    codesign_path: String,
    /// Path to security binary (for keychain operations)
    security_path: String,
    /// Path to productsign binary (for pkg signing)
    productsign_path: String,
}

impl MacOSProvider {
    /// Create a new macOS signing provider
    pub fn new() -> Self {
        Self {
            codesign_path: "/usr/bin/codesign".to_string(),
            security_path: "/usr/bin/security".to_string(),
            productsign_path: "/usr/bin/productsign".to_string(),
        }
    }

    /// Parse identity from `security find-identity` output line
    fn parse_identity_line(line: &str) -> Option<SigningIdentity> {
        // Format: "  1) FINGERPRINT "Name" (TYPE)"
        let line = line.trim();
        if !line.starts_with(char::is_numeric) {
            return None;
        }

        // Extract fingerprint (40 hex chars)
        let parts: Vec<&str> = line.splitn(3, ' ').collect();
        if parts.len() < 3 {
            return None;
        }

        let fingerprint = parts[1].to_string();

        // Extract name from quotes
        let rest = parts[2..].join(" ");
        let name_start = rest.find('"')?;
        let name_end = rest[name_start + 1..].find('"')? + name_start + 1;
        let name = rest[name_start + 1..name_end].to_string();

        // Determine identity type from name
        let identity_type = if name.contains("Developer ID Application") {
            SigningIdentityType::AppleDeveloper
        } else if name.contains("Developer ID Installer") {
            SigningIdentityType::AppleInstaller
        } else if name.contains("Apple Distribution") || name.contains("iPhone Distribution") {
            SigningIdentityType::AppleDistribution
        } else if name.contains("Mac App Store") {
            SigningIdentityType::AppleDistribution
        } else {
            SigningIdentityType::Generic
        };

        // Extract team ID if present (usually in parentheses at end of name)
        let team_id = if let Some(start) = name.rfind('(') {
            if let Some(end) = name.rfind(')') {
                Some(name[start + 1..end].to_string())
            } else {
                None
            }
        } else {
            None
        };

        let mut identity = SigningIdentity::new(fingerprint.clone(), name, identity_type);
        identity.fingerprint = Some(fingerprint);
        identity.team_id = team_id;

        Some(identity)
    }

    /// Get detailed certificate info using security command
    #[allow(dead_code)]
    async fn get_certificate_details(&self, fingerprint: &str) -> Result<SigningIdentity> {
        let output = Command::new(&self.security_path)
            .args(["find-certificate", "-c", fingerprint, "-p"])
            .output()
            .await?;

        // For now, just return basic identity - full cert parsing would need x509 crate
        let mut identity =
            SigningIdentity::new(fingerprint, fingerprint, SigningIdentityType::Generic);
        identity.fingerprint = Some(fingerprint.to_string());

        if output.status.success() {
            // Certificate found, mark as valid
            identity.is_valid = true;
        }

        Ok(identity)
    }

    /// Run codesign command
    async fn run_codesign(&self, args: &[&str]) -> Result<String> {
        debug!("Running codesign with args: {:?}", args);

        let output = Command::new(&self.codesign_path)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if !output.status.success() {
            return Err(SigningError::ToolFailed {
                tool: "codesign".to_string(),
                reason: if stderr.is_empty() { stdout } else { stderr },
            });
        }

        Ok(if stdout.is_empty() { stderr } else { stdout })
    }

    /// Check if file is a pkg installer
    fn is_pkg(&self, path: &Path) -> bool {
        path.extension()
            .map(|e| e.eq_ignore_ascii_case("pkg"))
            .unwrap_or(false)
    }

    /// Sign a pkg file using productsign
    async fn sign_pkg(
        &self,
        artifact: &Path,
        identity: &SigningIdentity,
        options: &SignOptions,
    ) -> Result<()> {
        let mut args = vec!["--sign", &identity.name];

        if options.timestamp {
            args.push("--timestamp");
        }

        // productsign requires output to a different file, then we move it
        let temp_path = artifact.with_extension("pkg.signed");
        let artifact_str = artifact.to_string_lossy();
        let temp_str = temp_path.to_string_lossy();

        args.push(&artifact_str);
        args.push(&temp_str);

        debug!("Running productsign with args: {:?}", args);

        let output = Command::new(&self.productsign_path)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::ToolFailed {
                tool: "productsign".to_string(),
                reason: stderr.to_string(),
            });
        }

        // Move signed file back to original location
        std::fs::rename(&temp_path, artifact)?;

        Ok(())
    }
}

impl Default for MacOSProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SigningProvider for MacOSProvider {
    fn name(&self) -> &str {
        "macos"
    }

    fn is_available(&self) -> bool {
        Path::new(&self.codesign_path).exists()
    }

    async fn list_identities(&self) -> Result<Vec<SigningIdentity>> {
        let output = Command::new(&self.security_path)
            .args(["find-identity", "-v", "-p", "codesigning"])
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::KeychainError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let identities: Vec<SigningIdentity> = stdout
            .lines()
            .filter_map(Self::parse_identity_line)
            .collect();

        Ok(identities)
    }

    async fn find_identity(&self, query: &str) -> Result<SigningIdentity> {
        let identities = self.list_identities().await?;

        let matches: Vec<_> = identities
            .into_iter()
            .filter(|id| {
                id.name.contains(query)
                    || id.fingerprint.as_ref().map(|f| f.contains(query)).unwrap_or(false)
                    || id.team_id.as_ref().map(|t| t == query).unwrap_or(false)
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
                "Dry run: would sign {} with {}",
                artifact.display(),
                identity.name
            );
            return Ok(());
        }

        // Use productsign for pkg files
        if self.is_pkg(artifact) {
            return self.sign_pkg(artifact, identity, options).await;
        }

        let mut args = vec!["-s", &identity.name];

        // Force re-signing
        if options.force {
            args.push("-f");
        }

        // Deep signing
        if options.deep {
            args.push("--deep");
        }

        // Hardened runtime
        if options.hardened_runtime {
            args.push("--options");
            args.push("runtime");
        }

        // Timestamp
        if options.timestamp {
            args.push("--timestamp");
        }

        // Entitlements
        let entitlements_str;
        if let Some(entitlements) = &options.entitlements {
            args.push("--entitlements");
            entitlements_str = entitlements.clone();
            args.push(&entitlements_str);
        }

        // Preserve metadata
        if options.preserve_metadata {
            args.push("--preserve-metadata=identifier,entitlements");
        }

        // Verbose
        if options.verbose {
            args.push("-v");
        }

        let artifact_str = artifact.to_string_lossy();
        args.push(&artifact_str);

        info!("Signing {} with {}", artifact.display(), identity.name);
        self.run_codesign(&args).await?;

        Ok(())
    }

    async fn verify(&self, artifact: &Path, options: &VerifyOptions) -> Result<SignatureInfo> {
        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        let mut args = vec!["-v"];

        if options.deep {
            args.push("--deep");
        }

        if options.strict {
            args.push("--strict");
        }

        let artifact_str = artifact.to_string_lossy();
        args.push(&artifact_str);

        let result = self.run_codesign(&args).await;

        let (status, details) = match result {
            Ok(output) => (SignatureStatus::Valid, Some(output)),
            Err(SigningError::ToolFailed { reason, .. }) => {
                if reason.contains("not signed") {
                    (SignatureStatus::NotSigned, Some(reason))
                } else if reason.contains("invalid signature") {
                    (SignatureStatus::Invalid, Some(reason))
                } else {
                    (SignatureStatus::Unknown, Some(reason))
                }
            }
            Err(e) => return Err(e),
        };

        // Get signer info if signed
        let signer = if status == SignatureStatus::Valid {
            let display_output = Command::new(&self.codesign_path)
                .args(["-d", "-v", &artifact_str])
                .output()
                .await?;

            let stderr = String::from_utf8_lossy(&display_output.stderr);

            // Parse Authority line for signer info
            let common_name = stderr
                .lines()
                .find(|l| l.starts_with("Authority="))
                .map(|l| l.trim_start_matches("Authority=").to_string());

            let team_id = stderr
                .lines()
                .find(|l| l.starts_with("TeamIdentifier="))
                .map(|l| l.trim_start_matches("TeamIdentifier=").to_string());

            common_name.map(|cn| SignerInfo {
                common_name: cn,
                organization: None,
                team_id,
                fingerprint: None,
                serial_number: None,
                expires_at: None,
                certificate_valid: true,
            })
        } else {
            None
        };

        // Check notarization if requested
        let notarized = if options.check_notarization && status == SignatureStatus::Valid {
            let spctl_output = Command::new("/usr/sbin/spctl")
                .args(["--assess", "-v", &artifact_str])
                .output()
                .await?;

            Some(spctl_output.status.success())
        } else {
            None
        };

        Ok(SignatureInfo {
            path: artifact.to_string_lossy().to_string(),
            status,
            signer,
            signed_at: None, // Would need to parse from certificate
            timestamp_authority: None,
            notarized,
            stapled: None,
            algorithm: None,
            warnings: vec![],
            details,
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &[
            "app", "framework", "dylib", "bundle", "kext", "xpc", "pkg", "dmg", "",
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identity_line() {
        let line = r#"  1) ABC123DEF456 "Developer ID Application: My Company (TEAMID123)""#;
        let identity = MacOSProvider::parse_identity_line(line);

        assert!(identity.is_some());
        let id = identity.unwrap();
        assert_eq!(id.fingerprint, Some("ABC123DEF456".to_string()));
        assert!(id.name.contains("Developer ID Application"));
    }

    #[test]
    fn test_is_pkg() {
        let provider = MacOSProvider::new();
        assert!(provider.is_pkg(Path::new("test.pkg")));
        assert!(provider.is_pkg(Path::new("test.PKG")));
        assert!(!provider.is_pkg(Path::new("test.app")));
    }
}
