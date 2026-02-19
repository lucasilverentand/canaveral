//! Minimal Go-template-subset expander for aqua registry templates

use std::collections::HashMap;

use regex::Regex;

/// Template variables for asset/URL expansion
pub struct TemplateVars {
    pub version: String,
    pub os: String,
    pub arch: String,
    pub format: String,
}

/// Expand a Go-template-style string with the given variables.
///
/// Supports:
/// - `{{.Version}}`, `{{.OS}}`, `{{.Arch}}`, `{{.Format}}`
/// - `{{trimV .Version}}` (strip leading `v`)
pub fn expand(template: &str, vars: &TemplateVars) -> String {
    let re = Regex::new(r"\{\{(.*?)\}\}").unwrap();
    re.replace_all(template, |caps: &regex::Captures| {
        let expr = caps[1].trim();
        match expr {
            ".Version" => vars.version.clone(),
            ".OS" => vars.os.clone(),
            ".Arch" => vars.arch.clone(),
            ".Format" => vars.format.clone(),
            _ if expr.starts_with("trimV ") => {
                let arg = expr.strip_prefix("trimV ").unwrap().trim();
                let val = match arg {
                    ".Version" => &vars.version,
                    ".OS" => &vars.os,
                    ".Arch" => &vars.arch,
                    ".Format" => &vars.format,
                    _ => return caps[0].to_string(),
                };
                val.trim_start_matches('v').to_string()
            }
            _ => caps[0].to_string(),
        }
    })
    .into_owned()
}

/// Apply replacements from the aqua package definition.
///
/// The replacements map is structured as `{ field: { from: to } }` where
/// `field` is typically an OS or Arch value like `"linux"` or `"amd64"`.
///
/// In aqua, replacements keys are the Go-style values (e.g. `"linux"`, `"amd64"`)
/// and the replacement maps `from -> to` remap those values.
pub fn apply_replacements(
    os: &str,
    arch: &str,
    replacements: &HashMap<String, HashMap<String, String>>,
) -> (String, String) {
    let mut result_os = os.to_string();
    let mut result_arch = arch.to_string();

    // Aqua replacements: top-level keys can be OS names or arch names
    // and their values map the default value to a replacement
    for (key, mapping) in replacements {
        match key.as_str() {
            // If the key matches current OS, apply arch replacements
            k if k == os => {
                if let Some(replacement) = mapping.get(arch) {
                    result_arch = replacement.clone();
                }
            }
            // Check if it's an arch-level key
            k if k == arch => {
                if let Some(replacement) = mapping.get(os) {
                    result_os = replacement.clone();
                }
            }
            // Named field replacements
            "os" => {
                if let Some(replacement) = mapping.get(os) {
                    result_os = replacement.clone();
                }
            }
            "arch" => {
                if let Some(replacement) = mapping.get(arch) {
                    result_arch = replacement.clone();
                }
            }
            _ => {}
        }
    }

    (result_os, result_arch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_expansion() {
        let vars = TemplateVars {
            version: "14.0.2".into(),
            os: "linux".into(),
            arch: "amd64".into(),
            format: "tar.gz".into(),
        };
        let result = expand("ripgrep-{{.Version}}-{{.Arch}}-{{.OS}}.tar.gz", &vars);
        assert_eq!(result, "ripgrep-14.0.2-amd64-linux.tar.gz");
    }

    #[test]
    fn trim_v() {
        let vars = TemplateVars {
            version: "v14.0.2".into(),
            os: "linux".into(),
            arch: "amd64".into(),
            format: "tar.gz".into(),
        };
        let result = expand("tool-{{trimV .Version}}.tar.gz", &vars);
        assert_eq!(result, "tool-14.0.2.tar.gz");
    }

    #[test]
    fn trim_v_no_prefix() {
        let vars = TemplateVars {
            version: "14.0.2".into(),
            os: "linux".into(),
            arch: "amd64".into(),
            format: "tar.gz".into(),
        };
        let result = expand("tool-{{trimV .Version}}.tar.gz", &vars);
        assert_eq!(result, "tool-14.0.2.tar.gz");
    }

    #[test]
    fn unknown_template_preserved() {
        let vars = TemplateVars {
            version: "1.0".into(),
            os: "linux".into(),
            arch: "amd64".into(),
            format: "tar.gz".into(),
        };
        let result = expand("{{.Unknown}}", &vars);
        assert_eq!(result, "{{.Unknown}}");
    }

    #[test]
    fn replacements_remap_arch() {
        let mut replacements = HashMap::new();
        let mut linux_map = HashMap::new();
        linux_map.insert("amd64".into(), "x86_64".into());
        replacements.insert("linux".into(), linux_map);

        let (os, arch) = apply_replacements("linux", "amd64", &replacements);
        assert_eq!(os, "linux");
        assert_eq!(arch, "x86_64");
    }

    #[test]
    fn replacements_no_match() {
        let mut replacements = HashMap::new();
        let mut linux_map = HashMap::new();
        linux_map.insert("amd64".into(), "x86_64".into());
        replacements.insert("linux".into(), linux_map);

        let (os, arch) = apply_replacements("darwin", "arm64", &replacements);
        assert_eq!(os, "darwin");
        assert_eq!(arch, "arm64");
    }
}
