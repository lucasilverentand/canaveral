//! Default configuration values

use super::types::Config;

/// Default configuration file name (TOML — preferred)
pub const DEFAULT_CONFIG_TOML: &str = "canaveral.toml";

/// Default configuration file name (YAML — legacy)
pub const DEFAULT_CONFIG_YAML: &str = "canaveral.yaml";

/// Alternative configuration file names (dotfile variants)
pub const ALT_CONFIG_TOML: &str = ".canaveral.toml";
pub const ALT_CONFIG_YAML: &str = ".canaveral.yaml";

/// Config file names to search for, in priority order.
///
/// At each directory level these names are checked in order, both in the
/// directory itself and inside the `.github/` subdirectory.
pub fn config_file_names() -> Vec<&'static str> {
    vec![
        DEFAULT_CONFIG_TOML,
        DEFAULT_CONFIG_YAML,
        ALT_CONFIG_TOML,
        ALT_CONFIG_YAML,
    ]
}

/// Generate default configuration as TOML
pub fn default_config_toml() -> String {
    let config = Config::default();
    toml::to_string_pretty(&config).unwrap_or_else(|_| DEFAULT_CONFIG_TEMPLATE.to_string())
}

/// Default configuration template (TOML)
pub const DEFAULT_CONFIG_TEMPLATE: &str = r#"# Canaveral Configuration
# See https://github.com/example/canaveral for documentation

[versioning]
strategy = "semver"
tag_format = "v{version}"
independent = false

[git]
remote = "origin"
branch = "main"
require_clean = true
push_tags = true
push_commits = true
commit_message = "chore(release): {version}"

[changelog]
enabled = true
file = "CHANGELOG.md"
format = "markdown"
include_hashes = true
include_authors = false
include_dates = true

[changelog.types.feat]
section = "Features"
hidden = false

[changelog.types.fix]
section = "Bug Fixes"
hidden = false

[changelog.types.docs]
section = "Documentation"
hidden = false

[changelog.types.perf]
section = "Performance"
hidden = false

[changelog.types.refactor]
section = "Refactoring"
hidden = true

[changelog.types.test]
section = "Tests"
hidden = true

[changelog.types.chore]
section = "Chores"
hidden = true

[publish]
enabled = true
dry_run = false

[hooks]
pre_version = []
post_version = []
pre_publish = []
post_publish = []

# App Store and Package Registry Configuration
# [stores.apple]
# api_key_id = "ABC123XYZ"
# api_issuer_id = "12345678-1234-1234-1234-123456789012"
# api_key = "/path/to/AuthKey_ABC123XYZ.p8"
#
# [stores.google_play]
# package_name = "com.example.app"
# service_account_key = "/path/to/service-account.json"
#
# [stores.microsoft]
# tenant_id = "12345678-1234-1234-1234-123456789012"
# client_id = "87654321-4321-4321-4321-210987654321"
# client_secret = "your_client_secret"
# app_id = "9NBLGGH1234"
#
# [stores.npm]
# registry_url = "https://registry.npmjs.org"
# # token: env var NPM_TOKEN or from ~/.npmrc
#
# [stores.crates_io]
# registry_url = "https://crates.io"
# # token: env var CARGO_REGISTRY_TOKEN or from ~/.cargo/credentials.toml
"#;
