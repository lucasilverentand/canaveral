//! Signing provider implementations

pub mod gpg;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

pub mod android;

use crate::error::{Result, SigningError};
use crate::provider::SigningProvider;

/// Available signing provider types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    /// macOS codesign
    MacOS,
    /// Windows signtool
    Windows,
    /// Android apksigner
    Android,
    /// GPG
    Gpg,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MacOS => write!(f, "macos"),
            Self::Windows => write!(f, "windows"),
            Self::Android => write!(f, "android"),
            Self::Gpg => write!(f, "gpg"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = SigningError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "macos" | "apple" | "codesign" => Ok(Self::MacOS),
            "windows" | "signtool" | "authenticode" => Ok(Self::Windows),
            "android" | "apksigner" | "jarsigner" => Ok(Self::Android),
            "gpg" | "pgp" => Ok(Self::Gpg),
            _ => Err(SigningError::ConfigError(format!(
                "Unknown signing provider: {}",
                s
            ))),
        }
    }
}

/// Create a signing provider for the given type
pub fn create_provider(provider_type: ProviderType) -> Result<Box<dyn SigningProvider>> {
    match provider_type {
        #[cfg(target_os = "macos")]
        ProviderType::MacOS => Ok(Box::new(macos::MacOSProvider::new())),

        #[cfg(not(target_os = "macos"))]
        ProviderType::MacOS => Err(SigningError::UnsupportedPlatform {
            provider: "macos".to_string(),
        }),

        #[cfg(target_os = "windows")]
        ProviderType::Windows => Ok(Box::new(windows::WindowsProvider::new())),

        #[cfg(not(target_os = "windows"))]
        ProviderType::Windows => Err(SigningError::UnsupportedPlatform {
            provider: "windows".to_string(),
        }),

        ProviderType::Android => Ok(Box::new(android::AndroidProvider::new())),

        ProviderType::Gpg => Ok(Box::new(gpg::GpgProvider::new())),
    }
}

/// Get the default signing provider for the current platform
pub fn default_provider() -> Result<Box<dyn SigningProvider>> {
    #[cfg(target_os = "macos")]
    {
        create_provider(ProviderType::MacOS)
    }

    #[cfg(target_os = "windows")]
    {
        create_provider(ProviderType::Windows)
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        create_provider(ProviderType::Gpg)
    }
}
