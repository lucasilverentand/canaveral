//! Default configuration values

use super::types::Config;

/// Default configuration file name (YAML)
pub const DEFAULT_CONFIG_YAML: &str = "canaveral.yaml";

/// Default configuration file name (TOML)
pub const DEFAULT_CONFIG_TOML: &str = "canaveral.toml";

/// Alternative configuration file name
pub const ALT_CONFIG_FILE: &str = ".canaveral.yaml";

/// Get list of config file names to search for
pub fn config_file_names() -> Vec<&'static str> {
    vec![
        DEFAULT_CONFIG_YAML,
        DEFAULT_CONFIG_TOML,
        ALT_CONFIG_FILE,
        ".canaveral.toml",
    ]
}

/// Generate default configuration YAML
pub fn default_config_yaml() -> String {
    let config = Config::default();
    serde_yaml::to_string(&config).unwrap_or_else(|_| DEFAULT_CONFIG_TEMPLATE.to_string())
}

/// Default configuration template
pub const DEFAULT_CONFIG_TEMPLATE: &str = r#"# Canaveral Configuration
# See https://github.com/example/canaveral for documentation

versioning:
  strategy: semver
  tag_format: "v{version}"
  independent: false

git:
  remote: origin
  branch: main
  require_clean: true
  push_tags: true
  push_commits: true
  commit_message: "chore(release): {version}"

changelog:
  enabled: true
  file: CHANGELOG.md
  format: markdown
  include_hashes: true
  include_authors: false
  include_dates: true
  types:
    feat:
      section: Features
      hidden: false
    fix:
      section: Bug Fixes
      hidden: false
    docs:
      section: Documentation
      hidden: false
    perf:
      section: Performance
      hidden: false
    refactor:
      section: Refactoring
      hidden: true
    test:
      section: Tests
      hidden: true
    chore:
      section: Chores
      hidden: true

publish:
  enabled: true
  dry_run: false

hooks:
  pre_version: []
  post_version: []
  pre_publish: []
  post_publish: []

# App Store and Package Registry Configuration
# stores:
#   # Apple App Store (iOS, macOS, tvOS, watchOS)
#   apple:
#     api_key_id: "ABC123XYZ"
#     api_issuer_id: "12345678-1234-1234-1234-123456789012"
#     api_key: "/path/to/AuthKey_ABC123XYZ.p8"
#
#   # Google Play Store (Android)
#   google_play:
#     package_name: "com.example.app"
#     service_account_key: "/path/to/service-account.json"
#
#   # Microsoft Store (Windows, Xbox)
#   microsoft:
#     tenant_id: "12345678-1234-1234-1234-123456789012"
#     client_id: "87654321-4321-4321-4321-210987654321"
#     client_secret: "your_client_secret"
#     app_id: "9NBLGGH1234"
#
#   # NPM Registry (JavaScript/TypeScript packages)
#   npm:
#     registry_url: "https://registry.npmjs.org"
#     # token: env var NPM_TOKEN or from ~/.npmrc
#
#   # Crates.io Registry (Rust packages)
#   crates_io:
#     registry_url: "https://crates.io"
#     # token: env var CARGO_REGISTRY_TOKEN or from ~/.cargo/credentials.toml
"#;
