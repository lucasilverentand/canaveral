//! Platform detection for the generic tool provider

/// Returns the platform key for the current system, e.g. "darwin-aarch64".
///
/// This key is used to look up OS/arch values in a tool definition's
/// `platforms` map and to select `platform_overrides`.
pub fn current_platform_key() -> &'static str {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "darwin-aarch64"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "darwin-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "linux-x86_64"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "linux-aarch64"
    } else if cfg!(all(target_os = "windows", target_arch = "x86_64")) {
        "windows-x86_64"
    } else if cfg!(all(target_os = "windows", target_arch = "aarch64")) {
        "windows-aarch64"
    } else {
        "unknown"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_platform_key_is_known() {
        let key = current_platform_key();
        assert_ne!(key, "unknown", "running on an unsupported platform");
    }

    #[test]
    fn platform_key_format() {
        let key = current_platform_key();
        // Should contain a hyphen separating OS and arch
        assert!(
            key.contains('-'),
            "platform key should be os-arch, got: {key}"
        );
    }
}
