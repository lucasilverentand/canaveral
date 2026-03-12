//! Provisioning profile parsing
//!
//! Extracts profile data from `.mobileprovision` files. On macOS, uses
//! `security cms -D` to strip the CMS envelope and expose the embedded plist.
//! Also supports parsing raw plist XML directly (for testing or pre-extracted data).

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use chrono::{DateTime, Utc};
use tracing::{debug, instrument};

use crate::error::{Result, SigningError};

use super::ProvisioningProfile;

/// Parse a `.mobileprovision` file and extract profile metadata.
///
/// On macOS, this shells out to `security cms -D -i <path>` to extract
/// the XML plist from the CMS (PKCS#7) envelope. The plist is then parsed
/// to extract UUID, name, team, entitlements, devices, and dates.
#[instrument(skip_all, fields(path = %path.display()))]
pub fn parse_mobileprovision(path: &Path) -> Result<ProvisioningProfile> {
    if !path.exists() {
        return Err(SigningError::ProvisioningProfileError(format!(
            "File not found: {}",
            path.display()
        )));
    }

    // Extract the plist XML from the CMS envelope
    let plist_xml = extract_plist_xml(path)?;

    // Parse the XML plist
    parse_plist_xml(&plist_xml, Some(path))
}

/// Extract plist XML from a .mobileprovision file using `security cms`.
fn extract_plist_xml(path: &Path) -> Result<String> {
    debug!("Extracting plist from {}", path.display());

    let output = Command::new("/usr/bin/security")
        .args(["cms", "-D", "-i"])
        .arg(path)
        .output()
        .map_err(|e| SigningError::ToolFailed {
            tool: "security".to_string(),
            reason: format!("Failed to run security cms: {}", e),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(SigningError::ToolFailed {
            tool: "security".to_string(),
            reason: format!("security cms failed: {}", stderr),
        });
    }

    String::from_utf8(output.stdout).map_err(|e| {
        SigningError::ProvisioningProfileError(format!("Invalid UTF-8 in plist output: {}", e))
    })
}

/// Parse a plist XML string into a ProvisioningProfile.
///
/// This is the core parsing logic, separated from the CMS extraction
/// so it can be tested independently with synthetic plist data.
pub fn parse_plist_xml(xml: &str, source_path: Option<&Path>) -> Result<ProvisioningProfile> {
    // Use simple XML tag extraction since we don't have a plist crate
    let uuid = extract_string_value(xml, "UUID").ok_or_else(|| {
        SigningError::ProvisioningProfileError("Missing UUID in profile".to_string())
    })?;

    let name = extract_string_value(xml, "Name").ok_or_else(|| {
        SigningError::ProvisioningProfileError("Missing Name in profile".to_string())
    })?;

    let team_id = extract_array_strings(xml, "TeamIdentifier")
        .into_iter()
        .next()
        .ok_or_else(|| {
            SigningError::ProvisioningProfileError("Missing TeamIdentifier in profile".to_string())
        })?;

    let bundle_id = extract_entitlement_bundle_id(xml).unwrap_or_default();

    let devices = extract_array_strings(xml, "ProvisionedDevices");

    let creation_date = extract_date_value(xml, "CreationDate").unwrap_or_else(Utc::now);

    let expiration_date = extract_date_value(xml, "ExpirationDate").unwrap_or_else(Utc::now);

    // Extract entitlements as a simplified map
    let entitlements = extract_entitlements(xml);

    // Determine profile type
    let has_get_task_allow = entitlements
        .get("get-task-allow")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    // Check for enterprise (ProvisionsAllDevices key)
    let is_enterprise = xml.contains("<key>ProvisionsAllDevices</key>");

    let profile_type = ProvisioningProfile::infer_profile_type(
        !devices.is_empty(),
        has_get_task_allow,
        is_enterprise,
    );

    // Extract certificate fingerprints from DeveloperCertificates
    let certificates = extract_certificate_fingerprints(xml);

    debug!(
        uuid = %uuid,
        name = %name,
        bundle_id = %bundle_id,
        profile_type = %profile_type,
        devices = devices.len(),
        certificates = certificates.len(),
        "Parsed provisioning profile"
    );

    Ok(ProvisioningProfile {
        uuid,
        name,
        team_id,
        bundle_id,
        profile_type,
        certificates,
        devices,
        entitlements,
        creation_date,
        expiration_date,
        path: source_path.map(|p| p.to_path_buf()),
    })
}

/// Extract a string value for a given key from plist XML.
///
/// Looks for patterns like:
/// ```xml
/// <key>KeyName</key>
/// <string>value</string>
/// ```
fn extract_string_value(xml: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{}</key>", key);
    let key_pos = xml.find(&key_tag)?;
    let after_key = &xml[key_pos + key_tag.len()..];

    // Skip whitespace
    let trimmed = after_key.trim_start();

    // Extract string value
    if trimmed.starts_with("<string>") {
        let start = "<string>".len();
        let end = trimmed.find("</string>")?;
        Some(trimmed[start..end].to_string())
    } else {
        None
    }
}

/// Extract a date value for a given key from plist XML.
fn extract_date_value(xml: &str, key: &str) -> Option<DateTime<Utc>> {
    let key_tag = format!("<key>{}</key>", key);
    let key_pos = xml.find(&key_tag)?;
    let after_key = &xml[key_pos + key_tag.len()..];
    let trimmed = after_key.trim_start();

    if trimmed.starts_with("<date>") {
        let start = "<date>".len();
        let end = trimmed.find("</date>")?;
        let date_str = &trimmed[start..end];
        // Apple plist dates are ISO 8601
        date_str.parse::<DateTime<Utc>>().ok()
    } else {
        None
    }
}

/// Extract an array of strings for a given key from plist XML.
fn extract_array_strings(xml: &str, key: &str) -> Vec<String> {
    let key_tag = format!("<key>{}</key>", key);
    let key_pos = match xml.find(&key_tag) {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let after_key = &xml[key_pos + key_tag.len()..];
    let trimmed = after_key.trim_start();

    if !trimmed.starts_with("<array>") {
        return Vec::new();
    }

    let array_end = match trimmed.find("</array>") {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let array_content = &trimmed["<array>".len()..array_end];

    let mut strings = Vec::new();
    let mut remaining = array_content;
    while let Some(start) = remaining.find("<string>") {
        let value_start = start + "<string>".len();
        if let Some(end) = remaining[value_start..].find("</string>") {
            strings.push(remaining[value_start..value_start + end].trim().to_string());
            remaining = &remaining[value_start + end + "</string>".len()..];
        } else {
            break;
        }
    }

    strings
}

/// Extract the bundle ID from the Entitlements section.
fn extract_entitlement_bundle_id(xml: &str) -> Option<String> {
    // Look for application-identifier in entitlements
    // The value is usually "TEAMID.com.example.app"
    let entitlements_start = xml.find("<key>Entitlements</key>")?;
    let entitlements_section = &xml[entitlements_start..];

    // Find the dict that follows
    let dict_start = entitlements_section.find("<dict>")?;
    let dict_end = entitlements_section[dict_start..].find("</dict>")?;
    let dict_content = &entitlements_section[dict_start..dict_start + dict_end + "</dict>".len()];

    // Try application-identifier first (iOS)
    if let Some(app_id) = extract_string_value(dict_content, "application-identifier") {
        // Strip team ID prefix (format: "TEAMID.com.example.app")
        if let Some(dot_pos) = app_id.find('.') {
            return Some(app_id[dot_pos + 1..].to_string());
        }
        return Some(app_id);
    }

    // Try com.apple.application-identifier (macOS)
    if let Some(app_id) = extract_string_value(dict_content, "com.apple.application-identifier") {
        if let Some(dot_pos) = app_id.find('.') {
            return Some(app_id[dot_pos + 1..].to_string());
        }
        return Some(app_id);
    }

    None
}

/// Extract entitlements as a simplified JSON map.
fn extract_entitlements(xml: &str) -> HashMap<String, serde_json::Value> {
    let mut map = HashMap::new();

    let entitlements_start = match xml.find("<key>Entitlements</key>") {
        Some(pos) => pos,
        None => return map,
    };
    let entitlements_section = &xml[entitlements_start..];

    let dict_start = match entitlements_section.find("<dict>") {
        Some(pos) => pos,
        None => return map,
    };
    let dict_end = match entitlements_section[dict_start..].find("</dict>") {
        Some(pos) => pos,
        None => return map,
    };
    let dict_content = &entitlements_section[dict_start + "<dict>".len()..dict_start + dict_end];

    // Extract key-value pairs from the entitlements dict
    let mut remaining = dict_content;
    while let Some(key_start) = remaining.find("<key>") {
        let key_value_start = key_start + "<key>".len();
        let key_end = match remaining[key_value_start..].find("</key>") {
            Some(pos) => key_value_start + pos,
            None => break,
        };
        let key = remaining[key_value_start..key_end].trim().to_string();
        remaining = &remaining[key_end + "</key>".len()..];

        let trimmed = remaining.trim_start();

        if let Some(rest) = trimmed.strip_prefix("<true/>") {
            map.insert(key, serde_json::Value::Bool(true));
            remaining = rest;
        } else if let Some(rest) = trimmed.strip_prefix("<false/>") {
            map.insert(key, serde_json::Value::Bool(false));
            remaining = rest;
        } else if trimmed.starts_with("<string>") {
            let start = "<string>".len();
            if let Some(end) = trimmed[start..].find("</string>") {
                let value = trimmed[start..start + end].to_string();
                map.insert(key, serde_json::Value::String(value));
                remaining = &trimmed[start + end + "</string>".len()..];
            }
        } else if trimmed.starts_with("<array>") {
            // Store arrays as JSON arrays of strings
            if let Some(array_end) = trimmed.find("</array>") {
                let array_content = &trimmed["<array>".len()..array_end];
                let mut values = Vec::new();
                let mut arr_remaining = array_content;
                while let Some(s_start) = arr_remaining.find("<string>") {
                    let s_value_start = s_start + "<string>".len();
                    if let Some(s_end) = arr_remaining[s_value_start..].find("</string>") {
                        values.push(serde_json::Value::String(
                            arr_remaining[s_value_start..s_value_start + s_end]
                                .trim()
                                .to_string(),
                        ));
                        arr_remaining = &arr_remaining[s_value_start + s_end + "</string>".len()..];
                    } else {
                        break;
                    }
                }
                map.insert(key, serde_json::Value::Array(values));
                remaining = &trimmed[array_end + "</array>".len()..];
            }
        }
    }

    map
}

/// Extract certificate SHA-1 fingerprints from DeveloperCertificates.
///
/// DeveloperCertificates contains base64-encoded DER certificates.
/// We compute the SHA-1 hash of each to get the fingerprint.
fn extract_certificate_fingerprints(xml: &str) -> Vec<String> {
    let key_tag = "<key>DeveloperCertificates</key>";
    let key_pos = match xml.find(key_tag) {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let after_key = &xml[key_pos + key_tag.len()..];
    let trimmed = after_key.trim_start();

    if !trimmed.starts_with("<array>") {
        return Vec::new();
    }

    let array_end = match trimmed.find("</array>") {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let array_content = &trimmed["<array>".len()..array_end];

    let mut fingerprints = Vec::new();
    let mut remaining = array_content;
    while let Some(data_start) = remaining.find("<data>") {
        let value_start = data_start + "<data>".len();
        if let Some(data_end) = remaining[value_start..].find("</data>") {
            let b64_data = remaining[value_start..value_start + data_end]
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>();

            if let Ok(der_bytes) =
                base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &b64_data)
            {
                // Compute SHA-1 fingerprint
                use sha2::Digest;
                let hash = sha2::Sha256::digest(&der_bytes);
                let fingerprint = hash
                    .iter()
                    .map(|b| format!("{:02X}", b))
                    .collect::<Vec<_>>()
                    .join(":");
                fingerprints.push(fingerprint);
            }

            remaining = &remaining[value_start + data_end + "</data>".len()..];
        } else {
            break;
        }
    }

    fingerprints
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::ProfileType;
    use chrono::Datelike;

    const SAMPLE_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>AppIDName</key>
    <string>Example App</string>
    <key>CreationDate</key>
    <date>2025-01-15T10:30:00Z</date>
    <key>DeveloperCertificates</key>
    <array>
        <data>
        MIIB0DCCAXag
        </data>
    </array>
    <key>Entitlements</key>
    <dict>
        <key>application-identifier</key>
        <string>TEAM123.com.example.myapp</string>
        <key>get-task-allow</key>
        <true/>
        <key>keychain-access-groups</key>
        <array>
            <string>TEAM123.*</string>
        </array>
        <key>aps-environment</key>
        <string>development</string>
    </dict>
    <key>ExpirationDate</key>
    <date>2026-01-15T10:30:00Z</date>
    <key>Name</key>
    <string>iOS Team Provisioning Profile: com.example.myapp</string>
    <key>ProvisionedDevices</key>
    <array>
        <string>00008110-001A2B3C4D5E6F00</string>
        <string>00008120-002B3C4D5E6F7800</string>
    </array>
    <key>TeamIdentifier</key>
    <array>
        <string>TEAM123</string>
    </array>
    <key>TeamName</key>
    <string>Example Team</string>
    <key>UUID</key>
    <string>12345678-ABCD-EF01-2345-6789ABCDEF01</string>
    <key>Version</key>
    <integer>1</integer>
</dict>
</plist>"#;

    const APPSTORE_PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CreationDate</key>
    <date>2025-06-01T12:00:00Z</date>
    <key>Entitlements</key>
    <dict>
        <key>application-identifier</key>
        <string>TEAM456.com.example.production</string>
        <key>get-task-allow</key>
        <false/>
    </dict>
    <key>ExpirationDate</key>
    <date>2026-06-01T12:00:00Z</date>
    <key>Name</key>
    <string>App Store Profile</string>
    <key>TeamIdentifier</key>
    <array>
        <string>TEAM456</string>
    </array>
    <key>UUID</key>
    <string>AABBCCDD-1122-3344-5566-778899001122</string>
</dict>
</plist>"#;

    #[test]
    fn test_parse_development_profile() {
        let profile = parse_plist_xml(SAMPLE_PLIST, None).unwrap();

        assert_eq!(profile.uuid, "12345678-ABCD-EF01-2345-6789ABCDEF01");
        assert_eq!(
            profile.name,
            "iOS Team Provisioning Profile: com.example.myapp"
        );
        assert_eq!(profile.team_id, "TEAM123");
        assert_eq!(profile.bundle_id, "com.example.myapp");
        assert_eq!(profile.devices.len(), 2);
        assert_eq!(profile.devices[0], "00008110-001A2B3C4D5E6F00");
        assert!(matches!(profile.profile_type, ProfileType::Development));
    }

    #[test]
    fn test_parse_appstore_profile() {
        let profile = parse_plist_xml(APPSTORE_PLIST, None).unwrap();

        assert_eq!(profile.uuid, "AABBCCDD-1122-3344-5566-778899001122");
        assert_eq!(profile.name, "App Store Profile");
        assert_eq!(profile.team_id, "TEAM456");
        assert_eq!(profile.bundle_id, "com.example.production");
        assert!(profile.devices.is_empty());
        assert!(matches!(profile.profile_type, ProfileType::AppStore));
    }

    #[test]
    fn test_parse_entitlements() {
        let profile = parse_plist_xml(SAMPLE_PLIST, None).unwrap();

        assert_eq!(
            profile.entitlements.get("get-task-allow"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            profile.entitlements.get("aps-environment"),
            Some(&serde_json::Value::String("development".to_string()))
        );

        // Verify keychain-access-groups array
        let keychain = profile.entitlements.get("keychain-access-groups").unwrap();
        assert!(keychain.is_array());
        let arr = keychain.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0], serde_json::Value::String("TEAM123.*".to_string()));
    }

    #[test]
    fn test_extract_string_value() {
        let xml = "<key>Name</key>\n<string>Test</string>";
        assert_eq!(extract_string_value(xml, "Name"), Some("Test".to_string()));
        assert_eq!(extract_string_value(xml, "Missing"), None);
    }

    #[test]
    fn test_extract_date_value() {
        let xml = "<key>ExpirationDate</key>\n<date>2026-01-15T10:30:00Z</date>";
        let date = extract_date_value(xml, "ExpirationDate").unwrap();
        assert_eq!(date.year(), 2026);
        assert_eq!(date.month(), 1);
    }

    #[test]
    fn test_extract_array_strings() {
        let xml = r#"<key>Devices</key>
        <array>
            <string>UDID1</string>
            <string>UDID2</string>
        </array>"#;
        let values = extract_array_strings(xml, "Devices");
        assert_eq!(values, vec!["UDID1", "UDID2"]);
    }

    #[test]
    fn test_parse_missing_uuid() {
        let xml = r#"<?xml version="1.0"?>
<plist version="1.0">
<dict>
    <key>Name</key>
    <string>Test</string>
</dict>
</plist>"#;
        let result = parse_plist_xml(xml, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_enterprise_profile() {
        let xml = r#"<?xml version="1.0"?>
<plist version="1.0">
<dict>
    <key>UUID</key>
    <string>ENTERPRISE-UUID</string>
    <key>Name</key>
    <string>Enterprise Profile</string>
    <key>TeamIdentifier</key>
    <array><string>ENT999</string></array>
    <key>CreationDate</key>
    <date>2025-01-01T00:00:00Z</date>
    <key>ExpirationDate</key>
    <date>2026-01-01T00:00:00Z</date>
    <key>ProvisionsAllDevices</key>
    <true/>
    <key>Entitlements</key>
    <dict>
        <key>application-identifier</key>
        <string>ENT999.com.enterprise.app</string>
    </dict>
</dict>
</plist>"#;

        let profile = parse_plist_xml(xml, None).unwrap();
        assert!(matches!(profile.profile_type, ProfileType::Enterprise));
        assert_eq!(profile.bundle_id, "com.enterprise.app");
    }
}
