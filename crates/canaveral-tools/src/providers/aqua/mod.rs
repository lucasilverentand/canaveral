//! Aqua registry-backed tool provider
//!
//! Parses package definitions from the aqua-registry to download pre-built
//! binaries from GitHub releases or HTTP URLs.

pub mod platform;
pub mod provider;
pub mod schema;
pub mod template;

pub use provider::AquaProvider;

use std::collections::HashMap;
use std::sync::OnceLock;

/// Parsed shortnames index: tool name → "owner/repo"
fn shortnames() -> &'static HashMap<String, String> {
    static SHORTNAMES: OnceLock<HashMap<String, String>> = OnceLock::new();
    SHORTNAMES.get_or_init(|| {
        let toml_str = include_str!("shortnames.toml");

        #[derive(serde::Deserialize)]
        struct ShortNamesFile {
            tools: HashMap<String, String>,
        }

        let parsed: ShortNamesFile = toml::from_str(toml_str).expect("shortnames.toml is invalid");
        parsed.tools
    })
}
