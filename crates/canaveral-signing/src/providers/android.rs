//! Android signing provider using apksigner

use crate::error::{Result, SigningError};
use crate::identity::{SigningIdentity, SigningIdentityType};
use crate::provider::{
    SignOptions, SignatureInfo, SignatureStatus, SignerInfo, SigningProvider, VerifyOptions,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use std::cmp::Reverse;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;
use tracing::{debug, info, instrument, warn};

/// Android signing provider using apksigner and keytool
pub struct AndroidProvider {
    /// Path to apksigner (usually in Android SDK build-tools)
    apksigner_path: Option<String>,
    /// Path to keytool (from JDK)
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

                    versions.sort_by_key(|v| Reverse(v.file_name()));

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
        self.apksigner_path
            .as_deref()
            .ok_or_else(|| SigningError::ToolNotFound {
                tool: "apksigner".to_string(),
                hint: "Install Android SDK build-tools or set ANDROID_HOME".to_string(),
            })
    }

    /// List keys in a keystore using keytool
    pub async fn list_keystore_keys(
        &self,
        keystore_path: &str,
        password: &str,
    ) -> Result<Vec<SigningIdentity>> {
        let output = Command::new(&self.keytool_path)
            .args([
                "-list",
                "-v",
                "-keystore",
                keystore_path,
                "-storepass",
                password,
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
    fn parse_keytool_output(output: &str, keystore_path: &str) -> Vec<SigningIdentity> {
        let mut identities = Vec::new();
        let mut current_alias: Option<String> = None;
        let mut current_fingerprint: Option<String> = None;
        let mut current_subject: Option<String> = None;
        let mut current_issuer: Option<String> = None;
        let mut current_serial: Option<String> = None;
        let mut current_created_at: Option<chrono::DateTime<Utc>> = None;
        let mut current_expires_at: Option<chrono::DateTime<Utc>> = None;

        for line in output.lines() {
            let line = line.trim();

            // Alias line: "Alias name: mykey"
            if line.starts_with("Alias name:") {
                // Save previous entry
                if let Some(alias) = current_alias.take() {
                    let is_valid = Self::check_cert_validity(
                        current_created_at.as_ref(),
                        current_expires_at.as_ref(),
                    );
                    let mut identity = SigningIdentity::new(
                        alias.clone(),
                        alias.clone(),
                        SigningIdentityType::AndroidKeystore,
                    );
                    identity.key_alias = Some(alias);
                    identity.keychain = Some(keystore_path.to_string());
                    identity.fingerprint = current_fingerprint.take();
                    identity.subject = current_subject.take();
                    identity.issuer = current_issuer.take();
                    identity.serial_number = current_serial.take();
                    identity.created_at = current_created_at.take();
                    identity.expires_at = current_expires_at.take();
                    identity.is_valid = is_valid;
                    identities.push(identity);
                }

                current_alias = Some(line.trim_start_matches("Alias name:").trim().to_string());
            }
            // Owner line: "Owner: CN=My App, O=My Company"
            else if line.starts_with("Owner:") {
                current_subject = Some(line.trim_start_matches("Owner:").trim().to_string());
            }
            // Issuer line: "Issuer: CN=My App, O=My Company"
            else if line.starts_with("Issuer:") {
                current_issuer = Some(line.trim_start_matches("Issuer:").trim().to_string());
            }
            // Serial number: "Serial number: 12345678"
            else if line.starts_with("Serial number:") {
                current_serial = Some(line.trim_start_matches("Serial number:").trim().to_string());
            }
            // Valid from line: "Valid from: Mon Jan 01 00:00:00 UTC 2024 until: Fri Jan 01 00:00:00 UTC 2054"
            else if line.starts_with("Valid from:") {
                let rest = line.trim_start_matches("Valid from:").trim();
                if let Some((from_part, until_part)) = rest.split_once("until:") {
                    let from_str = from_part.trim();
                    let until_str = until_part.trim();

                    if let Some(dt) = Self::parse_keytool_date(from_str) {
                        current_created_at = Some(dt);
                    }
                    if let Some(dt) = Self::parse_keytool_date(until_str) {
                        current_expires_at = Some(dt);
                    }
                }
            }
            // SHA256 fingerprint: "SHA256: AB:CD:..."
            else if line.starts_with("SHA256:") {
                current_fingerprint =
                    Some(line.trim_start_matches("SHA256:").trim().replace(':', ""));
            }
        }

        // Don't forget the last entry
        if let Some(alias) = current_alias {
            let is_valid =
                Self::check_cert_validity(current_created_at.as_ref(), current_expires_at.as_ref());
            let mut identity = SigningIdentity::new(
                alias.clone(),
                alias.clone(),
                SigningIdentityType::AndroidKeystore,
            );
            identity.key_alias = Some(alias);
            identity.keychain = Some(keystore_path.to_string());
            identity.fingerprint = current_fingerprint;
            identity.subject = current_subject;
            identity.issuer = current_issuer;
            identity.serial_number = current_serial;
            identity.created_at = current_created_at;
            identity.expires_at = current_expires_at;
            identity.is_valid = is_valid;
            identities.push(identity);
        }

        identities
    }

    /// Parse a date string from keytool output (e.g. "Mon Jan 01 00:00:00 UTC 2024")
    fn parse_keytool_date(date_str: &str) -> Option<chrono::DateTime<Utc>> {
        // keytool outputs dates like: "Mon Jan 01 00:00:00 UTC 2024"
        // We skip the day-of-week and timezone abbreviation, parsing only
        // the date/time components to avoid chrono's strict weekday validation.
        let parts: Vec<&str> = date_str.split_whitespace().collect();
        if parts.len() == 6 {
            // Skip parts[0] (day-of-week) and parts[4] (timezone), assume UTC
            let date_only = format!("{} {} {} {}", parts[1], parts[2], parts[3], parts[5]);
            if let Ok(naive) = NaiveDateTime::parse_from_str(&date_only, "%b %d %H:%M:%S %Y") {
                return Some(Utc.from_utc_datetime(&naive));
            }
        }
        None
    }

    /// Check whether a certificate is currently valid based on its dates
    fn check_cert_validity(
        created_at: Option<&chrono::DateTime<Utc>>,
        expires_at: Option<&chrono::DateTime<Utc>>,
    ) -> bool {
        let now = Utc::now();
        if let Some(exp) = expires_at {
            if *exp < now {
                return false;
            }
        }
        if let Some(start) = created_at {
            if *start > now {
                return false;
            }
        }
        true
    }

    /// Build apksigner signing scheme flags from SignOptions
    fn build_signing_scheme_args(options: &SignOptions) -> Vec<&'static str> {
        let mut args = Vec::new();
        if let Some(v1) = options.v1_signing {
            args.push(if v1 {
                "--v1-signing-enabled=true"
            } else {
                "--v1-signing-enabled=false"
            });
        }
        if let Some(v2) = options.v2_signing {
            args.push(if v2 {
                "--v2-signing-enabled=true"
            } else {
                "--v2-signing-enabled=false"
            });
        }
        if let Some(v3) = options.v3_signing {
            args.push(if v3 {
                "--v3-signing-enabled=true"
            } else {
                "--v3-signing-enabled=false"
            });
        }
        if let Some(v4) = options.v4_signing {
            args.push(if v4 {
                "--v4-signing-enabled=true"
            } else {
                "--v4-signing-enabled=false"
            });
        }
        args
    }

    /// Parse signing schemes from apksigner verify output
    fn parse_verify_signing_schemes(output: &str) -> String {
        let mut schemes = Vec::new();

        for line in output.lines() {
            let line = line.trim();
            if line.starts_with("Verified using v1 scheme") && line.ends_with("true") {
                schemes.push("JAR signing");
            } else if line.starts_with("Verified using v2 scheme") && line.ends_with("true") {
                schemes.push("APK Signature Scheme v2");
            } else if line.starts_with("Verified using v3 scheme") && line.ends_with("true") {
                schemes.push("APK Signature Scheme v3");
            } else if line.starts_with("Verified using v4 scheme") && line.ends_with("true") {
                schemes.push("APK Signature Scheme v4");
            }
        }

        if schemes.is_empty() {
            "Unknown".to_string()
        } else {
            schemes.join(", ")
        }
    }

    /// Parse certificate expiration from apksigner verify verbose output
    fn parse_verify_cert_expiration(output: &str) -> Option<chrono::DateTime<Utc>> {
        // apksigner verify --print-certs may include lines like:
        // "Signer #1 certificate validity: ..."
        // or date info in DN. We'll look for "not after:" patterns.
        for line in output.lines() {
            let line = line.trim();
            // Look for "Signer #1 certificate validity not after:" style
            if line.contains("not after:") {
                if let Some(date_str) = line.split("not after:").nth(1) {
                    if let Some(dt) = Self::parse_keytool_date(date_str.trim()) {
                        return Some(dt);
                    }
                }
            }
        }
        None
    }

    /// Generate a new Android keystore
    #[allow(clippy::too_many_arguments)]
    pub async fn generate_keystore(
        &self,
        keystore_path: &Path,
        alias: &str,
        password: &str,
        validity_days: u32,
        distinguished_name: &str,
        key_algorithm: &str,
        key_size: u32,
    ) -> Result<()> {
        if keystore_path.exists() {
            return Err(SigningError::ConfigError(format!(
                "Keystore already exists at: {}",
                keystore_path.display()
            )));
        }

        let keystore_str = keystore_path.to_string_lossy();
        let validity_str = validity_days.to_string();
        let key_size_str = key_size.to_string();

        let output = Command::new(&self.keytool_path)
            .args([
                "-genkeypair",
                "-v",
                "-keystore",
                &keystore_str,
                "-alias",
                alias,
                "-keyalg",
                key_algorithm,
                "-keysize",
                &key_size_str,
                "-validity",
                &validity_str,
                "-storepass",
                password,
                "-keypass",
                password,
                "-dname",
                distinguished_name,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SigningError::ToolFailed {
                tool: "keytool".to_string(),
                reason: stderr.to_string(),
            });
        }

        info!(
            "Generated keystore at {} with alias '{}'",
            keystore_path.display(),
            alias
        );
        Ok(())
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
        // Check for ANDROID_KEYSTORE_PATH env var
        let keystore_path = std::env::var("ANDROID_KEYSTORE_PATH").ok();
        let keystore_password = std::env::var("ANDROID_KEYSTORE_PASSWORD").ok();

        if let (Some(path), Some(password)) = (keystore_path, keystore_password) {
            debug!("Listing identities from keystore: {}", path);
            return self.list_keystore_keys(&path, &password).await;
        }

        // Android keystores are file-based, so we can't list them globally
        // Return empty list - user must specify keystore path or set env vars
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

    #[instrument(skip(self, identity, options), fields(provider = "android", path = %artifact.display()))]
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

        let keystore = identity
            .keychain
            .as_ref()
            .ok_or_else(|| SigningError::ConfigError("Keystore path not specified".to_string()))?;

        let key_alias = identity
            .key_alias
            .as_ref()
            .ok_or_else(|| SigningError::ConfigError("Key alias not specified".to_string()))?;

        let ks_pass = options.keystore_password.as_ref().ok_or_else(|| {
            SigningError::ConfigError("Keystore password not specified".to_string())
        })?;

        let key_pass = options.key_password.as_ref().unwrap_or(ks_pass);

        let artifact_str = artifact.to_string_lossy();
        let ks_pass_arg = format!("pass:{}", ks_pass);
        let key_pass_arg = format!("pass:{}", key_pass);

        let mut args = vec![
            "sign",
            "--ks",
            keystore,
            "--ks-key-alias",
            key_alias,
            "--ks-pass",
            &ks_pass_arg,
            "--key-pass",
            &key_pass_arg,
        ];

        // Add signing scheme flags
        let scheme_args = Self::build_signing_scheme_args(options);
        for arg in &scheme_args {
            args.push(arg);
        }

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

        info!(
            "Signed {} with Android key {}",
            artifact.display(),
            key_alias
        );
        Ok(())
    }

    #[instrument(skip(self, options), fields(provider = "android", path = %artifact.display()))]
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

        // Parse signing schemes from verbose output
        let algorithm = if options.verbose && status == SignatureStatus::Valid {
            let detected = Self::parse_verify_signing_schemes(&stdout);
            if detected == "Unknown" {
                Some("APK Signature Scheme v2/v3".to_string())
            } else {
                Some(detected)
            }
        } else {
            Some("APK Signature Scheme v2/v3".to_string())
        };

        // Parse cert expiration from verify output
        let cert_expiration = if options.verbose {
            Self::parse_verify_cert_expiration(&stdout)
        } else {
            None
        };

        // Parse signer info from verbose output
        let signer = if options.verbose && status == SignatureStatus::Valid {
            stdout
                .lines()
                .find(|l| l.contains("Signer #1 certificate DN:"))
                .map(|line| {
                    let dn = line.split("DN:").nth(1).unwrap_or("Unknown").trim();

                    let certificate_valid = if let Some(exp) = cert_expiration {
                        exp > Utc::now()
                    } else {
                        true
                    };

                    if !certificate_valid {
                        warn!("Signer certificate has expired");
                    }

                    SignerInfo {
                        common_name: dn.to_string(),
                        organization: None,
                        team_id: None,
                        fingerprint: None,
                        serial_number: None,
                        expires_at: cert_expiration,
                        certificate_valid,
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
            algorithm,
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
    use chrono::Datelike;

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
    fn test_parse_keytool_output_multiple_aliases() {
        let output = r#"
Keystore type: PKCS12
Keystore provider: SUN

Your keystore contains 2 entries

Alias name: release
Creation date: Jan 1, 2024
Entry type: PrivateKeyEntry
Certificate chain length: 1
Certificate[1]:
Owner: CN=Release Key, O=My Company
Issuer: CN=Release Key, O=My Company
Serial number: 11111111
Valid from: Mon Jan 01 00:00:00 UTC 2024 until: Fri Jan 01 00:00:00 UTC 2054
Certificate fingerprints:
	 SHA256: AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99

Alias name: debug
Creation date: Jun 15, 2023
Entry type: PrivateKeyEntry
Certificate chain length: 1
Certificate[1]:
Owner: CN=Debug Key, O=My Company
Issuer: CN=Debug Key, O=My Company
Serial number: 22222222
Valid from: Thu Jun 15 00:00:00 UTC 2023 until: Sun Jun 15 00:00:00 UTC 2053
Certificate fingerprints:
	 SHA256: 11:22:33:44:55:66:77:88:99:00:AA:BB:CC:DD:EE:FF:11:22:33:44:55:66:77:88:99:00:AA:BB:CC:DD:EE:FF
"#;

        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/keystore.jks");
        assert_eq!(identities.len(), 2);

        assert_eq!(identities[0].key_alias, Some("release".to_string()));
        assert_eq!(identities[0].name, "release");
        assert_eq!(
            identities[0].keychain,
            Some("/path/to/keystore.jks".to_string())
        );

        assert_eq!(identities[1].key_alias, Some("debug".to_string()));
        assert_eq!(identities[1].name, "debug");
        assert_eq!(
            identities[1].keychain,
            Some("/path/to/keystore.jks".to_string())
        );
    }

    #[test]
    fn test_parse_keytool_output_with_validity_dates() {
        let output = r#"
Alias name: release
Creation date: Jan 1, 2024
Entry type: PrivateKeyEntry
Owner: CN=My App, O=My Company
Issuer: CN=My App, O=My Company
Serial number: 12345678
Valid from: Mon Jan 01 00:00:00 UTC 2024 until: Fri Jan 01 00:00:00 UTC 2054
Certificate fingerprints:
	 SHA256: 11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00
"#;

        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/ks.jks");
        assert_eq!(identities.len(), 1);

        let id = &identities[0];
        assert!(id.created_at.is_some(), "created_at should be parsed");
        assert!(id.expires_at.is_some(), "expires_at should be parsed");

        let created = id.created_at.unwrap();
        assert_eq!(created.year(), 2024);
        assert_eq!(created.month(), 1);
        assert_eq!(created.day(), 1);

        let expires = id.expires_at.unwrap();
        assert_eq!(expires.year(), 2054);
        assert_eq!(expires.month(), 1);
        assert_eq!(expires.day(), 1);

        // Cert valid from 2024 to 2054, should be valid now
        assert!(id.is_valid);
    }

    #[test]
    fn test_parse_keytool_output_with_owner_issuer() {
        let output = r#"
Alias name: mykey
Creation date: Jan 1, 2024
Entry type: PrivateKeyEntry
Owner: CN=My App, O=My Company, C=US
Issuer: CN=My CA, O=Certificate Authority, C=US
Serial number: abcdef01
Valid from: Mon Jan 01 00:00:00 UTC 2024 until: Fri Jan 01 00:00:00 UTC 2054
Certificate fingerprints:
	 SHA256: AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99
"#;

        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/ks.jks");
        assert_eq!(identities.len(), 1);

        let id = &identities[0];
        assert_eq!(
            id.subject,
            Some("CN=My App, O=My Company, C=US".to_string())
        );
        assert_eq!(
            id.issuer,
            Some("CN=My CA, O=Certificate Authority, C=US".to_string())
        );
        assert_eq!(id.serial_number, Some("abcdef01".to_string()));
    }

    #[test]
    fn test_parse_keytool_output_empty() {
        let output = "";
        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/ks.jks");
        assert!(identities.is_empty());
    }

    #[tokio::test]
    async fn test_find_identity_with_keystore_alias() {
        let provider = AndroidProvider::default();
        let identity = provider.find_identity("path/to/ks:myalias").await.unwrap();

        assert_eq!(identity.keychain, Some("path/to/ks".to_string()));
        assert_eq!(identity.key_alias, Some("myalias".to_string()));
        assert_eq!(identity.name, "myalias");
    }

    #[tokio::test]
    async fn test_find_identity_plain_alias() {
        let provider = AndroidProvider::default();
        let identity = provider.find_identity("myalias").await.unwrap();

        assert_eq!(identity.keychain, None);
        assert_eq!(identity.key_alias, Some("myalias".to_string()));
        assert_eq!(identity.name, "myalias");
    }

    #[tokio::test]
    async fn test_sign_requires_keystore() {
        let provider = AndroidProvider {
            apksigner_path: Some("apksigner".to_string()),
            keytool_path: "keytool".to_string(),
        };

        // Identity without keychain
        let identity = SigningIdentity::new("test", "test", SigningIdentityType::AndroidKeystore);

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let options = SignOptions {
            keystore_password: Some("pass".to_string()),
            ..Default::default()
        };

        let result = provider.sign(tmp.path(), &identity, &options).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Keystore path not specified"),
            "Expected keystore error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_sign_requires_key_alias() {
        let provider = AndroidProvider {
            apksigner_path: Some("apksigner".to_string()),
            keytool_path: "keytool".to_string(),
        };

        let mut identity =
            SigningIdentity::new("test", "test", SigningIdentityType::AndroidKeystore);
        identity.keychain = Some("/path/to/keystore.jks".to_string());
        // key_alias is None

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let options = SignOptions {
            keystore_password: Some("pass".to_string()),
            ..Default::default()
        };

        let result = provider.sign(tmp.path(), &identity, &options).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Key alias not specified"),
            "Expected alias error, got: {}",
            err
        );
    }

    #[tokio::test]
    async fn test_sign_requires_password() {
        let provider = AndroidProvider {
            apksigner_path: Some("apksigner".to_string()),
            keytool_path: "keytool".to_string(),
        };

        let mut identity =
            SigningIdentity::new("test", "test", SigningIdentityType::AndroidKeystore);
        identity.keychain = Some("/path/to/keystore.jks".to_string());
        identity.key_alias = Some("release".to_string());

        let tmp = tempfile::NamedTempFile::new().unwrap();
        let options = SignOptions::default(); // No password

        let result = provider.sign(tmp.path(), &identity, &options).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("Keystore password not specified"),
            "Expected password error, got: {}",
            err
        );
    }

    #[test]
    fn test_supported_extensions() {
        let provider = AndroidProvider::default();
        let exts = provider.supported_extensions();
        assert_eq!(exts, &["apk", "aab"]);
    }

    #[test]
    fn test_provider_name() {
        let provider = AndroidProvider::default();
        assert_eq!(provider.name(), "android");
    }

    #[test]
    fn test_default_provider_creation() {
        let provider = AndroidProvider::default();
        assert_eq!(provider.name(), "android");
        assert_eq!(provider.keytool_path, "keytool");
        // apksigner_path depends on the system, so we just check it doesn't panic
    }

    #[test]
    fn test_signing_scheme_flag_construction() {
        // No flags set
        let options = SignOptions::default();
        let args = AndroidProvider::build_signing_scheme_args(&options);
        assert!(args.is_empty());

        // All enabled
        let options = SignOptions {
            v1_signing: Some(true),
            v2_signing: Some(true),
            v3_signing: Some(true),
            v4_signing: Some(true),
            ..Default::default()
        };
        let args = AndroidProvider::build_signing_scheme_args(&options);
        assert_eq!(args.len(), 4);
        assert_eq!(args[0], "--v1-signing-enabled=true");
        assert_eq!(args[1], "--v2-signing-enabled=true");
        assert_eq!(args[2], "--v3-signing-enabled=true");
        assert_eq!(args[3], "--v4-signing-enabled=true");

        // Mixed
        let options = SignOptions {
            v1_signing: Some(false),
            v2_signing: Some(true),
            v3_signing: None,
            v4_signing: Some(false),
            ..Default::default()
        };
        let args = AndroidProvider::build_signing_scheme_args(&options);
        assert_eq!(args.len(), 3);
        assert_eq!(args[0], "--v1-signing-enabled=false");
        assert_eq!(args[1], "--v2-signing-enabled=true");
        assert_eq!(args[2], "--v4-signing-enabled=false");

        // Only v3 disabled
        let options = SignOptions {
            v3_signing: Some(false),
            ..Default::default()
        };
        let args = AndroidProvider::build_signing_scheme_args(&options);
        assert_eq!(args.len(), 1);
        assert_eq!(args[0], "--v3-signing-enabled=false");
    }

    #[test]
    fn test_verify_parses_signing_schemes() {
        let output = r#"Verifies
Verified using v1 scheme (JAR signing): true
Verified using v2 scheme (APK Signature Scheme v2): true
Verified using v3 scheme (APK Signature Scheme v3): true
Verified using v4 scheme (APK Signature Scheme v4): false
"#;

        let result = AndroidProvider::parse_verify_signing_schemes(output);
        assert_eq!(
            result,
            "JAR signing, APK Signature Scheme v2, APK Signature Scheme v3"
        );

        // Only v2
        let output2 = r#"Verifies
Verified using v1 scheme (JAR signing): false
Verified using v2 scheme (APK Signature Scheme v2): true
Verified using v3 scheme (APK Signature Scheme v3): false
"#;
        let result2 = AndroidProvider::parse_verify_signing_schemes(output2);
        assert_eq!(result2, "APK Signature Scheme v2");

        // No schemes detected
        let output3 = "Verifies\n";
        let result3 = AndroidProvider::parse_verify_signing_schemes(output3);
        assert_eq!(result3, "Unknown");
    }

    #[test]
    fn test_cert_expiration_detected() {
        // Expired cert: expires_at in the past
        let output = r#"
Alias name: expired
Creation date: Jan 1, 2020
Entry type: PrivateKeyEntry
Owner: CN=Expired, O=Test
Issuer: CN=Expired, O=Test
Serial number: deadbeef
Valid from: Wed Jan 01 00:00:00 UTC 2020 until: Thu Jan 01 00:00:00 UTC 2021
Certificate fingerprints:
	 SHA256: AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99:AA:BB:CC:DD:EE:FF:00:11:22:33:44:55:66:77:88:99
"#;

        let identities = AndroidProvider::parse_keytool_output(output, "/path/to/ks.jks");
        assert_eq!(identities.len(), 1);

        let id = &identities[0];
        assert!(id.expires_at.is_some());

        let expires = id.expires_at.unwrap();
        assert_eq!(expires.year(), 2021);

        // Certificate expired in 2021, so is_valid should be false
        assert!(!id.is_valid, "Expired certificate should not be valid");
    }

    #[test]
    fn test_provider_creation() {
        let provider = AndroidProvider::new();
        assert_eq!(provider.name(), "android");
    }

    #[test]
    fn test_parse_keytool_date() {
        let date_str = "Mon Jan 01 00:00:00 UTC 2024";
        let result = AndroidProvider::parse_keytool_date(date_str);
        assert!(result.is_some());
        let dt = result.unwrap();
        assert_eq!(dt.year(), 2024);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 1);

        // Invalid date
        let bad = AndroidProvider::parse_keytool_date("not a date");
        assert!(bad.is_none());
    }

    #[test]
    fn test_check_cert_validity() {
        let now = Utc::now();

        // Valid cert
        let past = now - chrono::Duration::days(30);
        let future = now + chrono::Duration::days(365);
        assert!(AndroidProvider::check_cert_validity(
            Some(&past),
            Some(&future)
        ));

        // Expired cert
        let expired = now - chrono::Duration::days(1);
        assert!(!AndroidProvider::check_cert_validity(
            Some(&past),
            Some(&expired)
        ));

        // Not yet valid
        let future_start = now + chrono::Duration::days(1);
        let far_future = now + chrono::Duration::days(365);
        assert!(!AndroidProvider::check_cert_validity(
            Some(&future_start),
            Some(&far_future)
        ));

        // No dates
        assert!(AndroidProvider::check_cert_validity(None, None));
    }
}
