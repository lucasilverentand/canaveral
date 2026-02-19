//! Platform detection and mapping for aqua registry

use crate::providers::aqua::schema::{AquaOverride, AquaPackage};

/// Returns the Go-style `(goos, goarch)` for the current platform
pub fn current_platform() -> (&'static str, &'static str) {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "freebsd") {
        "freebsd"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "amd64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else if cfg!(target_arch = "x86") {
        "386"
    } else if cfg!(target_arch = "arm") {
        "armv6l"
    } else {
        "unknown"
    };

    (os, arch)
}

/// Check if the current platform is in the package's `supported_envs` list.
/// An empty list means all platforms are supported.
pub fn is_supported(supported_envs: &[String]) -> bool {
    if supported_envs.is_empty() {
        return true;
    }

    let (os, arch) = current_platform();
    let platform = format!("{os}/{arch}");

    supported_envs.iter().any(|env| {
        env == "all"
            || env == &platform
            || env == os
            || (env.ends_with("/all") && env.starts_with(os))
            || (env.starts_with("all/") && env.ends_with(arch))
    })
}

/// Find the best matching platform override from the package's overrides list.
/// Returns `None` if no override matches the current platform.
pub fn find_override(overrides: &[AquaOverride]) -> Option<&AquaOverride> {
    let (os, arch) = current_platform();

    // Prefer exact (os+arch) match, then os-only match
    let exact = overrides
        .iter()
        .find(|o| o.goos.as_deref() == Some(os) && o.goarch.as_deref() == Some(arch));
    if exact.is_some() {
        return exact;
    }

    overrides
        .iter()
        .find(|o| o.goos.as_deref() == Some(os) && o.goarch.is_none())
}

/// Apply platform overrides to a package, returning the effective asset, format,
/// files, and replacements for the current platform.
pub fn apply_overrides(pkg: &AquaPackage) -> AquaPackage {
    let mut result = pkg.clone();

    if let Some(ov) = find_override(&pkg.overrides) {
        if let Some(ref asset) = ov.asset {
            result.asset = Some(asset.clone());
        }
        if let Some(ref format) = ov.format {
            result.format = Some(format.clone());
        }
        if let Some(ref files) = ov.files {
            result.files = files.clone();
        }
        if let Some(ref replacements) = ov.replacements {
            result.replacements = replacements.clone();
        }
        if let Some(ref url) = ov.url {
            result.url = Some(url.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_is_known() {
        let (os, arch) = current_platform();
        assert_ne!(os, "unknown");
        assert_ne!(arch, "unknown");
    }

    #[test]
    fn empty_supported_envs_means_all() {
        assert!(is_supported(&[]));
    }

    #[test]
    fn supported_envs_all() {
        assert!(is_supported(&["all".to_string()]));
    }

    #[test]
    fn find_override_exact_match() {
        let (os, arch) = current_platform();
        let overrides = vec![
            AquaOverride {
                goos: Some("other".into()),
                goarch: None,
                asset: Some("other-asset".into()),
                format: None,
                replacements: None,
                files: None,
                url: None,
            },
            AquaOverride {
                goos: Some(os.into()),
                goarch: Some(arch.into()),
                asset: Some("exact-asset".into()),
                format: None,
                replacements: None,
                files: None,
                url: None,
            },
        ];
        let found = find_override(&overrides).unwrap();
        assert_eq!(found.asset.as_deref(), Some("exact-asset"));
    }

    #[test]
    fn find_override_os_only_fallback() {
        let (os, _) = current_platform();
        let overrides = vec![AquaOverride {
            goos: Some(os.into()),
            goarch: None,
            asset: Some("os-only-asset".into()),
            format: None,
            replacements: None,
            files: None,
            url: None,
        }];
        let found = find_override(&overrides).unwrap();
        assert_eq!(found.asset.as_deref(), Some("os-only-asset"));
    }

    #[test]
    fn find_override_no_match() {
        let overrides = vec![AquaOverride {
            goos: Some("nonexistent".into()),
            goarch: None,
            asset: Some("nope".into()),
            format: None,
            replacements: None,
            files: None,
            url: None,
        }];
        assert!(find_override(&overrides).is_none());
    }
}
