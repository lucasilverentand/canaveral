//! Android signing provider using apksigner

use crate::error::{Result, SigningError};
use crate::identity::{SigningIdentity, SigningIdentityType};
use crate::provider::{
    SignOptions, SignatureInfo, SignatureStatus, SignerInfo, SigningProvider, VerifyOptions,
};
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info};

/// Android signing provider using apksigner and keytool
pub struct AndroidProvider {
    /// Path to apksigner (usually in Android SDK build-tools)
    apksigner_path: Option<String>,
    /// Path to keytool (from JDK)
    #[allow(dead_code)]
    keytool_path: String,
}

impl AndroidProvider {
    /// Create a new Android signing provider
    pub fn new() -> Self {
        Self {
            apksigner_path: Self::find_apksigner(),
            keytool_path: "keytool".to_string(),
        }
    }

    /// Find apksigner in common locations
    fn find_apksigner() -> Option<String> {
        // Check ANDROID_HOME/ANDROID_SDK_ROOT
        let sdk_paths = [
            std::env::var("ANDROID_HOME").ok(),
            std::env::var("ANDROID_SDK_ROOT").ok(),
            // Common default locations
            Some("/usr/local/share/android-sdk".to_string()),
            dirs::home_dir().map(|h| h.join("Android/Sdk").to_string_lossy().to_string()),
            dirs::home_dir().map(|h| h.join("Library/Android/sdk").to_string_lossy().to_string()),
        ];

        for sdk_path in sdk_paths.into_iter().flatten() {
            let build_tools = Path::new(&sdk_path).join("build-tools");
            if build_tools.exists() {
                // Find the latest build-tools version
                if let Ok(entries) = std::fs::read_dir(&build_tools) {
                    let mut versions: Vec<_> = entries
                        .filter_map(|e| e.ok())
                        .filter(|e| e.path().is_dir())
                        .collect();

                    versions.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

                    if let Some(latest) = versions.first() {
                        let apksigner = latest.path().join("apksigner");
                        if apksigner.exists() {
                            return Some(apksigner.to_string_lossy().to_string());
                        }
                    }
                }
            }
        }

        // Try PATH
        if std::process::Command::new("apksigner")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return Some("apksigner".to_string());
        }

        None
    }

    /// Get apksigner path or return error
    fn get_apksigner(&self) -> Result<&str> {
        self.apksigner_path.as_deref().ok_or_else(|| SigningError::ToolNotFound {
            tool: "apksigner".to_string(),
            hint: "Install Android SDK build-tools or set ANDROID_HOME".to_string(),
        })
    }

    /// List keys in a keystore using keytool
    #[allow(dead_code)]
    async fn list_keystore_keys(&self, keystore_path: &str, password: &str) -> Result<Vec<SigningIdentity>> {
        let output = Command::new(&self.keytool_path)
            .args([
                "-list",
                "-v",
                "-keystore", keystore_path,
                "-storepass", password,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::KeychainError(stderr.to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(Self::parse_keytool_output(&stdout, keystore_path))
    }

    /// Parse keytool -list output
    #[allow(dead_code)]
    fn parse_keytool_output(output: &str, keystore_path: &str) -> Vec<SigningIdentity> {
        let mut identities = Vec::new();
        let mut current_alias: Option<String> = None;
        let mut current_fingerprint: Option<String> = None;

        for line in output.lines() {
            let line = line.trim();

            // Alias line: "Alias name: mykey"
            if line.starts_with("Alias name:") {
                // Save previous entry
                if let Some(alias) = current_alias.take() {
                    let mut identity = SigningIdentity::new(
                        alias.clone(),
                        alias.clone(),
                        SigningIdentityType::AndroidKeystore,
                    );
                    identity.key_alias = Some(alias);
                    identity.keychain = Some(keystore_path.to_string());
                    identity.fingerprint = current_fingerprint.take();
                    identities.push(identity);
                }

                current_alias = Some(line.trim_start_matches("Alias name:").trim().to_string());
            }
            // SHA256 fingerprint: "SHA256: AB:CD:..."
            else if line.starts_with("SHA256:") {
                current_fingerprint = Some(
                    line.trim_start_matches("SHA256:")
                        .trim()
                        .replace(':', "")
                );
            }
        }

        // Don't forget the last entry
        if let Some(alias) = current_alias {
            let mut identity = SigningIdentity::new(
                alias.clone(),
                alias.clone(),
                SigningIdentityType::AndroidKeystore,
            );
            identity.key_alias = Some(alias);
            identity.keychain = Some(keystore_path.to_string());
            identity.fingerprint = current_fingerprint;
            identities.push(identity);
        }

        identities
    }
}

impl Default for AndroidProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SigningProvider for AndroidProvider {
    fn name(&self) -> &str {
        "android"
    }

    fn is_available(&self) -> bool {
        self.apksigner_path.is_some()
    }

    async fn list_identities(&self) -> Result<Vec<SigningIdentity>> {
        // Android keystores are file-based, so we can't list them globally
        // Return empty list - user must specify keystore path
        Ok(vec![])
    }

    async fn find_identity(&self, query: &str) -> Result<SigningIdentity> {
        // For Android, the query should be in format "keystore_path:alias"
        // or just an alias if keystore is configured elsewhere
        let mut identity = SigningIdentity::new(
            query.to_string(),
            query.to_string(),
            SigningIdentityType::AndroidKeystore,
        );

        if query.contains(':') {
            let parts: Vec<&str> = query.splitn(2, ':').collect();
            if parts.len() == 2 {
                identity.keychain = Some(parts[0].to_string());
                identity.key_alias = Some(parts[1].to_string());
                identity.name = parts[1].to_string();
            }
        } else {
            identity.key_alias = Some(query.to_string());
        }

        Ok(identity)
    }

    async fn sign(
        &self,
        artifact: &Path,
        identity: &SigningIdentity,
        options: &SignOptions,
    ) -> Result<()> {
        let apksigner = self.get_apksigner()?;

        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        if options.dry_run {
            info!(
                "Dry run: would sign {} with Android key {}",
                artifact.display(),
                identity.name
            );
            return Ok(());
        }

        let keystore = identity.keychain.as_ref().ok_or_else(|| {
            SigningError::ConfigError("Keystore path not specified".to_string())
        })?;

        let key_alias = identity.key_alias.as_ref().ok_or_else(|| {
            SigningError::ConfigError("Key alias not specified".to_string())
        })?;

        let ks_pass = options.keystore_password.as_ref().ok_or_else(|| {
            SigningError::ConfigError("Keystore password not specified".to_string())
        })?;

        let key_pass = options.key_password.as_ref().unwrap_or(ks_pass);

        let artifact_str = artifact.to_string_lossy();
        let ks_pass_arg = format!("pass:{}", ks_pass);
        let key_pass_arg = format!("pass:{}", key_pass);

        let mut args = vec![
            "sign",
            "--ks", keystore,
            "--ks-key-alias", key_alias,
            "--ks-pass", &ks_pass_arg,
            "--key-pass", &key_pass_arg,
        ];

        if options.verbose {
            args.push("-v");
        }

        args.push(&artifact_str);

        debug!("Running apksigner with args: {:?}", args);

        let output = Command::new(apksigner)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::ToolFailed {
                tool: "apksigner".to_string(),
                reason: stderr.to_string(),
            });
        }

        info!("Signed {} with Android key {}", artifact.display(), key_alias);
        Ok(())
    }

    async fn verify(&self, artifact: &Path, options: &VerifyOptions) -> Result<SignatureInfo> {
        let apksigner = self.get_apksigner()?;

        if !artifact.exists() {
            return Err(SigningError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("Artifact not found: {}", artifact.display()),
            )));
        }

        let artifact_str = artifact.to_string_lossy();

        let mut args = vec!["verify"];

        if options.verbose {
            args.push("-v");
            args.push("--print-certs");
        }

        args.push(&artifact_str);

        let output = Command::new(apksigner)
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        let status = if output.status.success() {
            SignatureStatus::Valid
        } else if stderr.contains("does not contain") || stdout.contains("DOES NOT VERIFY") {
            SignatureStatus::NotSigned
        } else {
            SignatureStatus::Invalid
        };

        // Parse signer info from verbose output
        let signer = if options.verbose && status == SignatureStatus::Valid {
            stdout
                .lines()
                .find(|l| l.contains("Signer #1 certificate DN:"))
                .map(|line| {
                    let dn = line.split("DN:").nth(1).unwrap_or("Unknown").trim();
                    SignerInfo {
                        common_name: dn.to_string(),
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

        let combined_output = format!("{}\n{}", stdout, stderr);

        Ok(SignatureInfo {
            path: artifact.to_string_lossy().to_string(),
            status,
            signer,
            signed_at: None,
            timestamp_authority: None,
            notarized: None,
            stapled: None,
            algorithm: Some("APK Signature Scheme v2/v3".to_string()),
            warnings: vec![],
            details: Some(combined_output),
        })
    }

    fn supported_extensions(&self) -> &[&str] {
        &["apk", "aab"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_keytool_output() {
        let output = r#"
Keystore type: PKCS12
Keystore provider: SUN

Your keystore contains 1 entry

Alias name: release
Creation date: Jan 1, 2024
Entry type: PrivateKeyEntry
Certificate chain length: 1
Certificate[1]:
Owner: CN=My App, O=My Company
Issuer: CN=My App, O=My Company
Serial number: 12345678
Valid from: Mon Jan 01 00:00:00 UTC 2024 until: Fri Jan 01 00:00:00 UTC 2054
Certificate fingerprints:
	 SHA1: AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD
	 SHA256: 11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00
"#;

        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/keystore.jks");
        assert_eq!(identities.len(), 1);

        let id = &identities[0];
        assert_eq!(id.key_alias, Some("release".to_string()));
        assert!(id.fingerprint.is_some());
    }

    #[test]
    fn test_provider_creation() {
        let provider = AndroidProvider::new();
        assert_eq!(provider.name(), "android");
    }
}
