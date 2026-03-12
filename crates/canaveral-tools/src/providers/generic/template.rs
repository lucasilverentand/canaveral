//! Simple placeholder expansion for tool definition templates

/// Expand `{version}`, `{os}`, and `{arch}` placeholders in a template string.
///
/// Placeholders that don't match any of the three known names are left as-is.
pub fn expand(template: &str, version: &str, os: &str, arch: &str) -> String {
    template
        .replace("{version}", version)
        .replace("{os}", os)
        .replace("{arch}", arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_expansion() {
        let result = expand(
            "ripgrep-{version}-{arch}-{os}.tar.gz",
            "14.1.1",
            "apple-darwin",
            "aarch64",
        );
        assert_eq!(result, "ripgrep-14.1.1-aarch64-apple-darwin.tar.gz");
    }

    #[test]
    fn missing_placeholders_left_as_is() {
        let result = expand(
            "tool-{version}-{unknown}.tar.gz",
            "1.0.0",
            "linux",
            "x86_64",
        );
        assert_eq!(result, "tool-1.0.0-{unknown}.tar.gz");
    }

    #[test]
    fn no_placeholders() {
        let result = expand("static-filename.tar.gz", "1.0.0", "linux", "x86_64");
        assert_eq!(result, "static-filename.tar.gz");
    }

    #[test]
    fn all_three_placeholders() {
        let result = expand("{os}-{arch}-{version}", "2.0", "darwin", "arm64");
        assert_eq!(result, "darwin-arm64-2.0");
    }

    #[test]
    fn repeated_placeholders() {
        let result = expand("{version}-{version}", "3.0", "linux", "amd64");
        assert_eq!(result, "3.0-3.0");
    }

    #[test]
    fn empty_template() {
        let result = expand("", "1.0", "linux", "x86_64");
        assert_eq!(result, "");
    }
}
