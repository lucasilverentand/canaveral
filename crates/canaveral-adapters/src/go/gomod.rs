//! go.mod file parsing

use std::path::Path;

use canaveral_core::error::{AdapterError, Result};

/// Parsed go.mod file
#[derive(Debug, Clone)]
pub struct GoMod {
    /// Module path
    pub module: String,
    /// Go version
    pub go_version: Option<String>,
    /// Direct dependencies
    pub require: Vec<Dependency>,
    /// Replacements
    pub replace: Vec<Replace>,
    /// Exclusions
    pub exclude: Vec<Dependency>,
    /// Retractions
    pub retract: Vec<String>,
}

/// A dependency in go.mod
#[derive(Debug, Clone)]
pub struct Dependency {
    /// Module path
    pub path: String,
    /// Version
    pub version: String,
    /// Whether this is an indirect dependency
    pub indirect: bool,
}

/// A replace directive
#[derive(Debug, Clone)]
pub struct Replace {
    /// Original module path
    pub old_path: String,
    /// Original version (optional)
    pub old_version: Option<String>,
    /// Replacement path
    pub new_path: String,
    /// Replacement version (optional)
    pub new_version: Option<String>,
}

impl GoMod {
    /// Load a go.mod file
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            AdapterError::ManifestParseError(format!("Failed to read go.mod: {}", e))
        })?;

        Self::parse(&content)
    }

    /// Parse go.mod content
    pub fn parse(content: &str) -> Result<Self> {
        let mut module = String::new();
        let mut go_version = None;
        let mut require = Vec::new();
        let mut replace = Vec::new();
        let mut exclude = Vec::new();
        let mut retract = Vec::new();

        let mut in_block: Option<&str> = None;

        for line in content.lines() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with("//") {
                continue;
            }

            // Handle block start/end
            if line.ends_with('(') {
                let directive = line.trim_end_matches('(').trim();
                in_block = Some(match directive {
                    "require" => "require",
                    "replace" => "replace",
                    "exclude" => "exclude",
                    "retract" => "retract",
                    _ => continue,
                });
                continue;
            }

            if line == ")" {
                in_block = None;
                continue;
            }

            // Parse directives
            if let Some(block) = in_block {
                match block {
                    "require" => {
                        if let Some(dep) = Self::parse_require_line(line) {
                            require.push(dep);
                        }
                    }
                    "replace" => {
                        if let Some(rep) = Self::parse_replace_line(line) {
                            replace.push(rep);
                        }
                    }
                    "exclude" => {
                        if let Some(dep) = Self::parse_require_line(line) {
                            exclude.push(dep);
                        }
                    }
                    "retract" => {
                        retract.push(line.to_string());
                    }
                    _ => {}
                }
            } else if let Some(rest) = line.strip_prefix("module ") {
                module = rest.trim().to_string();
            } else if let Some(rest) = line.strip_prefix("go ") {
                go_version = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("require ") {
                if let Some(dep) = Self::parse_require_line(rest) {
                    require.push(dep);
                }
            } else if let Some(rest) = line.strip_prefix("replace ") {
                if let Some(rep) = Self::parse_replace_line(rest) {
                    replace.push(rep);
                }
            } else if let Some(rest) = line.strip_prefix("exclude ") {
                if let Some(dep) = Self::parse_require_line(rest) {
                    exclude.push(dep);
                }
            } else if let Some(rest) = line.strip_prefix("retract ") {
                retract.push(rest.to_string());
            }
        }

        if module.is_empty() {
            return Err(AdapterError::ManifestParseError(
                "No module directive found in go.mod".to_string(),
            )
            .into());
        }

        Ok(Self {
            module,
            go_version,
            require,
            replace,
            exclude,
            retract,
        })
    }

    /// Parse a require line: `path version [// indirect]`
    fn parse_require_line(line: &str) -> Option<Dependency> {
        let line = line.trim();
        let indirect = line.contains("// indirect");
        let line = line.split("//").next()?.trim();

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            Some(Dependency {
                path: parts[0].to_string(),
                version: parts[1].to_string(),
                indirect,
            })
        } else {
            None
        }
    }

    /// Parse a replace line: `old [version] => new [version]`
    fn parse_replace_line(line: &str) -> Option<Replace> {
        let parts: Vec<&str> = line.split("=>").collect();
        if parts.len() != 2 {
            return None;
        }

        let old_parts: Vec<&str> = parts[0].split_whitespace().collect();
        let new_parts: Vec<&str> = parts[1].split_whitespace().collect();

        if old_parts.is_empty() || new_parts.is_empty() {
            return None;
        }

        Some(Replace {
            old_path: old_parts[0].to_string(),
            old_version: old_parts.get(1).map(|s| s.to_string()),
            new_path: new_parts[0].to_string(),
            new_version: new_parts.get(1).map(|s| s.to_string()),
        })
    }

    /// Get the major version from the module path (for v2+ modules)
    pub fn major_version(&self) -> Option<u64> {
        // Check if module path ends with /vN
        if let Some(last) = self.module.rsplit('/').next() {
            if let Some(v) = last.strip_prefix('v') {
                return v.parse().ok();
            }
        }
        None
    }

    /// Check if this is a v2+ module
    pub fn is_v2_plus(&self) -> bool {
        self.major_version().map(|v| v >= 2).unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let content = r#"
module github.com/example/test

go 1.21
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert_eq!(gomod.module, "github.com/example/test");
        assert_eq!(gomod.go_version, Some("1.21".to_string()));
        assert!(gomod.require.is_empty());
    }

    #[test]
    fn test_parse_with_require() {
        let content = r#"
module github.com/example/test

go 1.21

require (
    github.com/pkg/errors v0.9.1
    golang.org/x/sys v0.5.0 // indirect
)
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert_eq!(gomod.require.len(), 2);

        let errors = gomod
            .require
            .iter()
            .find(|d| d.path.contains("errors"))
            .unwrap();
        assert_eq!(errors.version, "v0.9.1");
        assert!(!errors.indirect);

        let sys = gomod
            .require
            .iter()
            .find(|d| d.path.contains("sys"))
            .unwrap();
        assert!(sys.indirect);
    }

    #[test]
    fn test_parse_inline_require() {
        let content = r#"
module github.com/example/test

go 1.21

require github.com/pkg/errors v0.9.1
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert_eq!(gomod.require.len(), 1);
        assert_eq!(gomod.require[0].path, "github.com/pkg/errors");
    }

    #[test]
    fn test_parse_replace() {
        let content = r#"
module github.com/example/test

go 1.21

replace github.com/foo/bar => ../bar
replace github.com/old/module v1.0.0 => github.com/new/module v2.0.0
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert_eq!(gomod.replace.len(), 2);

        let local = &gomod.replace[0];
        assert_eq!(local.old_path, "github.com/foo/bar");
        assert!(local.old_version.is_none());
        assert_eq!(local.new_path, "../bar");

        let versioned = &gomod.replace[1];
        assert_eq!(versioned.old_version, Some("v1.0.0".to_string()));
        assert_eq!(versioned.new_version, Some("v2.0.0".to_string()));
    }

    #[test]
    fn test_v2_module() {
        let content = r#"
module github.com/example/test/v2

go 1.21
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert!(gomod.is_v2_plus());
        assert_eq!(gomod.major_version(), Some(2));
    }

    #[test]
    fn test_v1_module() {
        let content = r#"
module github.com/example/test

go 1.21
"#;

        let gomod = GoMod::parse(content).unwrap();
        assert!(!gomod.is_v2_plus());
        assert_eq!(gomod.major_version(), None);
    }
}
