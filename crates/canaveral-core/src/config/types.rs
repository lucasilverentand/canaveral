//! Config types — split into domain-specific modules.
//! This file is kept only for backward compatibility of tests.

#[cfg(test)]
mod tests {
    use crate::config::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.versioning.strategy, "semver");
        assert_eq!(config.git.remote, "origin");
        assert!(config.changelog.enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let yaml = serde_yaml::to_string(&config).unwrap();
        assert!(yaml.contains("strategy: semver"));
    }

    #[test]
    fn test_tools_config_simple_versions() {
        let toml = r#"
[tools]
bun = "1.2"
node = "22"
rust = "1.75"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(matches!(
            config.tools.tools.get("bun"),
            Some(ToolVersionSpec::Version(v)) if v == "1.2"
        ));
        assert!(matches!(
            config.tools.tools.get("node"),
            Some(ToolVersionSpec::Version(v)) if v == "22"
        ));
        assert!(matches!(
            config.tools.tools.get("rust"),
            Some(ToolVersionSpec::Version(v)) if v == "1.75"
        ));
    }

    #[test]
    fn test_tools_config_detailed_spec() {
        let toml = r#"
[tools]
rust = { version = "1.75", install_method = "rustup" }
node = { version = "22" }
"#;
        let config: Config = toml::from_str(toml).unwrap();
        match config.tools.tools.get("rust") {
            Some(ToolVersionSpec::Detailed(spec)) => {
                assert_eq!(spec.version, "1.75");
                assert_eq!(spec.install_method.as_deref(), Some("rustup"));
            }
            other => panic!("expected Detailed, got {:?}", other),
        }
        match config.tools.tools.get("node") {
            Some(ToolVersionSpec::Detailed(spec)) => {
                assert_eq!(spec.version, "22");
                assert!(spec.install_method.is_none());
            }
            other => panic!("expected Detailed, got {:?}", other),
        }
    }

    #[test]
    fn test_tools_config_mixed() {
        let toml = r#"
[tools]
bun = "1.2"
rust = { version = "1.75", install_method = "rustup" }
go = "1.22"
python = "3.12"
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert_eq!(config.tools.tools.len(), 4);
        assert!(matches!(
            config.tools.tools.get("bun"),
            Some(ToolVersionSpec::Version(_))
        ));
        assert!(matches!(
            config.tools.tools.get("rust"),
            Some(ToolVersionSpec::Detailed(_))
        ));
    }

    #[test]
    fn test_tools_config_default_is_empty() {
        let config = Config::default();
        assert!(config.tools.tools.is_empty());
    }

    #[test]
    fn test_tools_config_absent_section() {
        let toml = r#"name = "my-project""#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.tools.tools.is_empty());
    }
}
