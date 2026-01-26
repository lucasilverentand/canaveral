//! Build artifacts - normalized representation of build outputs
//!
//! Regardless of which framework produces the output, artifacts are represented
//! uniformly so downstream operations (signing, uploading, distribution) work
//! the same way.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::traits::Platform;

/// A build artifact
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Path to the artifact file
    pub path: PathBuf,

    /// Kind of artifact
    pub kind: ArtifactKind,

    /// Target platform
    pub platform: Platform,

    /// Size in bytes
    pub size: u64,

    /// SHA256 hash (hex encoded)
    pub sha256: Option<String>,

    /// Metadata about the artifact
    pub metadata: ArtifactMetadata,
}

impl Artifact {
    /// Create a new artifact
    pub fn new(path: impl Into<PathBuf>, kind: ArtifactKind, platform: Platform) -> Self {
        let path = path.into();
        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);

        Self {
            path,
            kind,
            platform,
            size,
            sha256: None,
            metadata: ArtifactMetadata::default(),
        }
    }

    /// Compute and set SHA256 hash
    pub fn with_sha256(mut self) -> Self {
        if let Ok(content) = std::fs::read(&self.path) {
            use sha2::Digest;
            let hash = sha2::Sha256::digest(&content);
            self.sha256 = Some(format!("{:x}", hash));
        }
        self
    }

    /// Set metadata
    pub fn with_metadata(mut self, metadata: ArtifactMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    /// Get file extension
    pub fn extension(&self) -> Option<&str> {
        self.path.extension().and_then(|e| e.to_str())
    }

    /// Get filename
    pub fn filename(&self) -> Option<&str> {
        self.path.file_name().and_then(|n| n.to_str())
    }

    /// Check if artifact is ready for App Store
    pub fn is_app_store_ready(&self) -> bool {
        matches!(
            self.kind,
            ArtifactKind::Ipa | ArtifactKind::Aab | ArtifactKind::Pkg | ArtifactKind::Msix
        )
    }

    /// Check if artifact needs signing
    pub fn needs_signing(&self) -> bool {
        match self.kind {
            ArtifactKind::App | ArtifactKind::Apk | ArtifactKind::Exe => true,
            ArtifactKind::Ipa | ArtifactKind::Aab => {
                // Already signed during build for these formats
                !self.metadata.signed
            }
            _ => false,
        }
    }
}

/// Kind of build artifact
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ArtifactKind {
    // iOS
    /// iOS App Bundle (.app)
    App,
    /// iOS Archive (.xcarchive)
    XcArchive,
    /// iOS IPA for App Store/Ad Hoc
    Ipa,

    // Android
    /// Android APK
    Apk,
    /// Android App Bundle (AAB) for Play Store
    Aab,

    // macOS
    /// macOS Application Bundle (.app)
    MacApp,
    /// macOS Installer Package (.pkg)
    Pkg,
    /// macOS Disk Image (.dmg)
    Dmg,

    // Windows
    /// Windows Executable (.exe)
    Exe,
    /// Windows Installer (.msi)
    Msi,
    /// Windows App Package (.msix/.appx)
    Msix,

    // Linux
    /// Debian package (.deb)
    Deb,
    /// Red Hat package (.rpm)
    Rpm,
    /// AppImage
    AppImage,
    /// Flatpak
    Flatpak,
    /// Snap package
    Snap,
    /// Tarball (.tar.gz)
    Tarball,

    // Web
    /// Static web build directory
    WebBuild,

    // Cross-platform
    /// Tauri bundle (platform-specific)
    TauriBundle,
    /// Electron package
    ElectronPackage,

    // Other
    /// Generic archive
    Archive,
    /// Unknown/custom
    Other,
}

impl ArtifactKind {
    /// Get typical file extension for this kind
    pub fn extension(&self) -> &'static str {
        match self {
            Self::App => "app",
            Self::XcArchive => "xcarchive",
            Self::Ipa => "ipa",
            Self::Apk => "apk",
            Self::Aab => "aab",
            Self::MacApp => "app",
            Self::Pkg => "pkg",
            Self::Dmg => "dmg",
            Self::Exe => "exe",
            Self::Msi => "msi",
            Self::Msix => "msix",
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::AppImage => "AppImage",
            Self::Flatpak => "flatpak",
            Self::Snap => "snap",
            Self::Tarball => "tar.gz",
            Self::WebBuild => "",
            Self::TauriBundle => "",
            Self::ElectronPackage => "",
            Self::Archive => "zip",
            Self::Other => "",
        }
    }

    /// Infer artifact kind from file path
    pub fn from_path(path: &std::path::Path) -> Self {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        match ext.as_str() {
            "app" => {
                // Could be iOS or macOS - check parent path
                if path.to_string_lossy().contains("Build/Products") {
                    Self::App
                } else {
                    Self::MacApp
                }
            }
            "xcarchive" => Self::XcArchive,
            "ipa" => Self::Ipa,
            "apk" => Self::Apk,
            "aab" => Self::Aab,
            "pkg" => Self::Pkg,
            "dmg" => Self::Dmg,
            "exe" => Self::Exe,
            "msi" => Self::Msi,
            "msix" | "appx" => Self::Msix,
            "deb" => Self::Deb,
            "rpm" => Self::Rpm,
            "appimage" => Self::AppImage,
            "flatpak" => Self::Flatpak,
            "snap" => Self::Snap,
            "gz" if path.to_string_lossy().ends_with(".tar.gz") => Self::Tarball,
            "zip" => Self::Archive,
            _ => Self::Other,
        }
    }
}

/// Metadata about an artifact
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    /// Version string embedded in artifact
    pub version: Option<String>,

    /// Build number
    pub build_number: Option<u64>,

    /// Universal identifier (bundle ID for mobile, package name for libraries, etc.)
    pub identifier: Option<String>,

    /// Minimum OS version
    pub min_os_version: Option<String>,

    /// Target architectures
    pub architectures: Vec<String>,

    /// Whether artifact is signed
    pub signed: bool,

    /// Signing identity used
    pub signing_identity: Option<String>,

    /// Build timestamp
    pub built_at: Option<chrono::DateTime<chrono::Utc>>,

    /// Framework that produced this artifact
    pub framework: Option<String>,

    /// Framework version
    pub framework_version: Option<String>,

    /// Custom metadata
    pub custom: std::collections::HashMap<String, serde_json::Value>,
}

impl ArtifactMetadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    pub fn with_build_number(mut self, build_number: u64) -> Self {
        self.build_number = Some(build_number);
        self
    }

    pub fn with_identifier(mut self, identifier: impl Into<String>) -> Self {
        self.identifier = Some(identifier.into());
        self
    }

    pub fn with_framework(mut self, framework: impl Into<String>) -> Self {
        self.framework = Some(framework.into());
        self
    }

    pub fn with_signed(mut self, signed: bool) -> Self {
        self.signed = signed;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_artifact_kind_from_path() {
        assert_eq!(
            ArtifactKind::from_path(Path::new("app.ipa")),
            ArtifactKind::Ipa
        );
        assert_eq!(
            ArtifactKind::from_path(Path::new("app-release.aab")),
            ArtifactKind::Aab
        );
        assert_eq!(
            ArtifactKind::from_path(Path::new("myapp.dmg")),
            ArtifactKind::Dmg
        );
    }

    #[test]
    fn test_artifact_metadata_builder() {
        let meta = ArtifactMetadata::new()
            .with_version("1.2.3")
            .with_build_number(42)
            .with_identifier("com.example.app")
            .with_framework("flutter")
            .with_signed(true);

        assert_eq!(meta.version, Some("1.2.3".to_string()));
        assert_eq!(meta.build_number, Some(42));
        assert_eq!(meta.identifier, Some("com.example.app".to_string()));
        assert!(meta.signed);
    }

    #[test]
    fn test_artifact_app_store_ready() {
        let ipa = Artifact::new("/tmp/test.ipa", ArtifactKind::Ipa, Platform::Ios);
        assert!(ipa.is_app_store_ready());

        let apk = Artifact::new("/tmp/test.apk", ArtifactKind::Apk, Platform::Android);
        assert!(!apk.is_app_store_ready());

        let aab = Artifact::new("/tmp/test.aab", ArtifactKind::Aab, Platform::Android);
        assert!(aab.is_app_store_ready());
    }
}
