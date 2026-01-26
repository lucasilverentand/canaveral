//! Apple App Store and notarization support
//!
//! This module provides integration with:
//! - macOS notarization via `notarytool`
//! - App Store Connect API for uploads
//!
//! ## Notarization
//!
//! ```ignore
//! use canaveral_stores::apple::Notarizer;
//!
//! let notarizer = Notarizer::new(config)?;
//! let result = notarizer.notarize(&artifact_path, None).await?;
//! ```

mod connect;
mod notarize;
mod testflight;

pub use connect::AppStoreConnect;
pub use notarize::Notarizer;
pub use testflight::{
    TestFlight, TestFlightBuild, BuildProcessingState, BuildAudienceType,
    BetaGroup, BetaTester, TesterInviteType, BetaAppReviewSubmission, BetaReviewState,
};

use crate::error::{Result, StoreError};
use crate::types::AppInfo;
use std::path::Path;
use std::process::Stdio;
use tokio::process::Command;

/// Check if a tool is available on the system
#[allow(dead_code)]
pub(crate) async fn check_tool(tool: &str) -> bool {
    Command::new("which")
        .arg(tool)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Extract app info from a macOS app bundle or package
pub async fn extract_app_info(path: &Path) -> Result<AppInfo> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "app" => extract_app_bundle_info(path).await,
        "pkg" => extract_pkg_info(path).await,
        "dmg" => extract_dmg_info(path).await,
        "zip" => extract_zip_info(path).await,
        _ => Err(StoreError::InvalidArtifact(format!(
            "Unsupported file type: {}",
            ext
        ))),
    }
}

/// Extract info from an .app bundle
async fn extract_app_bundle_info(path: &Path) -> Result<AppInfo> {
    let info_plist = path.join("Contents/Info.plist");

    if !info_plist.exists() {
        return Err(StoreError::InvalidArtifact(
            "Missing Info.plist in app bundle".to_string(),
        ));
    }

    let plist: plist::Value = plist::from_file(&info_plist)
        .map_err(|e| StoreError::InvalidArtifact(format!("Failed to read Info.plist: {}", e)))?;

    let dict = plist
        .as_dictionary()
        .ok_or_else(|| StoreError::InvalidArtifact("Info.plist is not a dictionary".to_string()))?;

    let bundle_id = dict
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .ok_or_else(|| StoreError::InvalidArtifact("Missing CFBundleIdentifier".to_string()))?
        .to_string();

    let version = dict
        .get("CFBundleShortVersionString")
        .and_then(|v| v.as_string())
        .unwrap_or("0.0.0")
        .to_string();

    let build_number = dict
        .get("CFBundleVersion")
        .and_then(|v| v.as_string())
        .unwrap_or("1")
        .to_string();

    let name = dict
        .get("CFBundleName")
        .or_else(|| dict.get("CFBundleDisplayName"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let min_os_version = dict
        .get("LSMinimumSystemVersion")
        .or_else(|| dict.get("MinimumOSVersion"))
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    // Determine platforms
    let mut platforms = Vec::new();
    if dict.contains_key("LSMinimumSystemVersion") {
        platforms.push("macOS".to_string());
    }
    if dict.contains_key("MinimumOSVersion") {
        platforms.push("iOS".to_string());
    }
    if dict.contains_key("UIDeviceFamily") {
        if !platforms.contains(&"iOS".to_string()) {
            platforms.push("iOS".to_string());
        }
    }
    if platforms.is_empty() {
        platforms.push("macOS".to_string());
    }

    // Calculate size
    let size = calculate_dir_size(path).await?;

    Ok(AppInfo {
        identifier: bundle_id,
        version,
        build_number,
        name,
        min_os_version,
        platforms,
        size,
        sha256: None,
    })
}

/// Extract info from a .pkg installer
async fn extract_pkg_info(path: &Path) -> Result<AppInfo> {
    // Use pkgutil to get info
    let output = Command::new("pkgutil")
        .args(["--payload-files", path.to_str().unwrap()])
        .output()
        .await
        .map_err(|e| StoreError::CommandFailed(format!("pkgutil failed: {}", e)))?;

    if !output.status.success() {
        // Try xar for flat packages
        let output = Command::new("xar")
            .args(["-tf", path.to_str().unwrap()])
            .output()
            .await
            .map_err(|e| StoreError::CommandFailed(format!("xar failed: {}", e)))?;

        if !output.status.success() {
            return Err(StoreError::InvalidArtifact(
                "Failed to read package contents".to_string(),
            ));
        }
    }

    // For packages, we extract basic info from the filename if we can't read the package
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unknown");

    let size = std::fs::metadata(path)
        .map(|m| m.len())
        .unwrap_or(0);

    Ok(AppInfo {
        identifier: format!("pkg.{}", filename.replace(' ', ".")),
        version: "1.0.0".to_string(),
        build_number: "1".to_string(),
        name: Some(filename.to_string()),
        min_os_version: None,
        platforms: vec!["macOS".to_string()],
        size,
        sha256: None,
    })
}

/// Extract info from a .dmg disk image
async fn extract_dmg_info(path: &Path) -> Result<AppInfo> {
    // Mount the DMG temporarily to inspect contents
    let temp_mount = tempfile::tempdir()
        .map_err(|e| StoreError::Io(e))?;

    let mount_output = Command::new("hdiutil")
        .args([
            "attach",
            path.to_str().unwrap(),
            "-mountpoint",
            temp_mount.path().to_str().unwrap(),
            "-nobrowse",
            "-quiet",
        ])
        .output()
        .await
        .map_err(|e| StoreError::CommandFailed(format!("hdiutil attach failed: {}", e)))?;

    if !mount_output.status.success() {
        let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

        return Ok(AppInfo {
            identifier: format!("dmg.{}", filename.replace(' ', ".")),
            version: "1.0.0".to_string(),
            build_number: "1".to_string(),
            name: Some(filename.to_string()),
            min_os_version: None,
            platforms: vec!["macOS".to_string()],
            size,
            sha256: None,
        });
    }

    // Look for .app bundles in the mounted DMG
    let mut app_info = None;
    if let Ok(entries) = std::fs::read_dir(temp_mount.path()) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.extension().and_then(|e| e.to_str()) == Some("app") {
                if let Ok(info) = extract_app_bundle_info(&entry_path).await {
                    app_info = Some(info);
                    break;
                }
            }
        }
    }

    // Unmount
    let _ = Command::new("hdiutil")
        .args(["detach", temp_mount.path().to_str().unwrap(), "-quiet"])
        .output()
        .await;

    match app_info {
        Some(mut info) => {
            // Update size to DMG size
            info.size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
            Ok(info)
        }
        None => {
            let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            Ok(AppInfo {
                identifier: format!("dmg.{}", filename.replace(' ', ".")),
                version: "1.0.0".to_string(),
                build_number: "1".to_string(),
                name: Some(filename.to_string()),
                min_os_version: None,
                platforms: vec!["macOS".to_string()],
                size,
                sha256: None,
            })
        }
    }
}

/// Extract info from a .zip archive
async fn extract_zip_info(path: &Path) -> Result<AppInfo> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| StoreError::InvalidArtifact(format!("Invalid zip file: {}", e)))?;

    // First pass: find the Info.plist index
    let mut plist_index: Option<usize> = None;
    for i in 0..archive.len() {
        let file = archive.by_index(i)
            .map_err(|e| StoreError::InvalidArtifact(format!("Failed to read zip entry: {}", e)))?;

        let name = file.name().to_string();
        drop(file); // Explicitly drop the borrow

        if name.ends_with("/Contents/Info.plist") || name.ends_with(".app/Contents/Info.plist") {
            plist_index = Some(i);
            break;
        }
    }

    // Second pass: extract and parse the Info.plist if found
    if let Some(i) = plist_index {
        let mut file = archive.by_index(i)
            .map_err(|e| StoreError::InvalidArtifact(format!("Failed to read zip entry: {}", e)))?;

        let mut contents = Vec::new();
        std::io::Read::read_to_end(&mut file, &mut contents)?;
        drop(file); // Explicitly drop the borrow

        let plist: plist::Value = plist::from_reader(std::io::Cursor::new(&contents))
            .map_err(|e| StoreError::InvalidArtifact(format!("Failed to parse Info.plist: {}", e)))?;

        if let Some(dict) = plist.as_dictionary() {
            let bundle_id = dict
                .get("CFBundleIdentifier")
                .and_then(|v| v.as_string())
                .unwrap_or("unknown")
                .to_string();

            let version = dict
                .get("CFBundleShortVersionString")
                .and_then(|v| v.as_string())
                .unwrap_or("0.0.0")
                .to_string();

            let build_number = dict
                .get("CFBundleVersion")
                .and_then(|v| v.as_string())
                .unwrap_or("1")
                .to_string();

            let name = dict
                .get("CFBundleName")
                .and_then(|v| v.as_string())
                .map(|s| s.to_string());

            let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

            return Ok(AppInfo {
                identifier: bundle_id,
                version,
                build_number,
                name,
                min_os_version: None,
                platforms: vec!["macOS".to_string()],
                size,
                sha256: None,
            });
        }
    }

    // Fallback
    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown");
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    Ok(AppInfo {
        identifier: format!("zip.{}", filename.replace(' ', ".")),
        version: "1.0.0".to_string(),
        build_number: "1".to_string(),
        name: Some(filename.to_string()),
        min_os_version: None,
        platforms: vec!["macOS".to_string()],
        size,
        sha256: None,
    })
}

/// Calculate directory size recursively
async fn calculate_dir_size(path: &Path) -> Result<u64> {
    let mut size = 0u64;

    if path.is_file() {
        return Ok(std::fs::metadata(path).map(|m| m.len()).unwrap_or(0));
    }

    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    stack.push(entry_path);
                } else if let Ok(metadata) = entry.metadata() {
                    size += metadata.len();
                }
            }
        }
    }

    Ok(size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_check_tool() {
        // 'ls' should always exist on macOS/Linux
        #[cfg(unix)]
        assert!(check_tool("ls").await);

        // Non-existent tool should return false
        assert!(!check_tool("definitely-not-a-real-tool-xyz").await);
    }
}
