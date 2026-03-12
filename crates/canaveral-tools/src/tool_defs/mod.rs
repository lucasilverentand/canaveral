//! Embedded tool definition registry
//!
//! Tool definitions are TOML files compiled into the binary via `include_str!`.
//! Each file describes how to download, extract, and detect a single tool.

use std::collections::HashMap;
use std::sync::OnceLock;

pub mod schema;

pub use schema::ToolDefinition;

/// Returns the static map of all embedded tool definitions, keyed by id and aliases.
pub fn definitions() -> &'static HashMap<String, ToolDefinition> {
    static DEFS: OnceLock<HashMap<String, ToolDefinition>> = OnceLock::new();
    DEFS.get_or_init(|| {
        let mut map = HashMap::new();

        // Load all embedded definitions
        load_def(&mut map, include_str!("data/ripgrep.toml"));
        load_def(&mut map, include_str!("data/fd.toml"));
        load_def(&mut map, include_str!("data/bat.toml"));
        load_def(&mut map, include_str!("data/delta.toml"));
        load_def(&mut map, include_str!("data/just.toml"));
        load_def(&mut map, include_str!("data/typos.toml"));
        load_def(&mut map, include_str!("data/git-cliff.toml"));
        load_def(&mut map, include_str!("data/starship.toml"));
        load_def(&mut map, include_str!("data/hyperfine.toml"));
        load_def(&mut map, include_str!("data/xh.toml"));
        load_def(&mut map, include_str!("data/deno.toml"));
        load_def(&mut map, include_str!("data/cargo-nextest.toml"));
        load_def(&mut map, include_str!("data/gh.toml"));
        load_def(&mut map, include_str!("data/fzf.toml"));
        load_def(&mut map, include_str!("data/actionlint.toml"));
        load_def(&mut map, include_str!("data/lazygit.toml"));
        load_def(&mut map, include_str!("data/k9s.toml"));
        load_def(&mut map, include_str!("data/act.toml"));
        load_def(&mut map, include_str!("data/yq.toml"));
        load_def(&mut map, include_str!("data/jq.toml"));
        load_def(&mut map, include_str!("data/direnv.toml"));
        load_def(&mut map, include_str!("data/biome.toml"));
        load_def(&mut map, include_str!("data/mise.toml"));
        load_def(&mut map, include_str!("data/taplo.toml"));
        load_def(&mut map, include_str!("data/eza.toml"));
        load_def(&mut map, include_str!("data/shellcheck.toml"));

        map
    })
}

/// Parse a TOML definition and insert it (plus its aliases) into the map.
fn load_def(map: &mut HashMap<String, ToolDefinition>, toml_str: &str) {
    let def: ToolDefinition =
        toml::from_str(toml_str).expect("embedded tool definition is invalid TOML");
    for alias in &def.aliases {
        map.insert(alias.clone(), def.clone());
    }
    map.insert(def.id.clone(), def);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn definitions_loads_ripgrep() {
        let defs = definitions();
        assert!(defs.contains_key("ripgrep"));
        let rg = &defs["ripgrep"];
        assert_eq!(rg.binary, "rg");
        assert_eq!(rg.repo, "BurntSushi/ripgrep");
    }

    #[test]
    fn alias_lookup_works() {
        let defs = definitions();
        assert!(defs.contains_key("rg"));
        let rg = &defs["rg"];
        assert_eq!(rg.id, "ripgrep");
    }

    #[test]
    fn definition_has_platforms() {
        let defs = definitions();
        let rg = &defs["ripgrep"];
        assert!(rg.platforms.contains_key("darwin-aarch64"));
        assert!(rg.platforms.contains_key("linux-x86_64"));
    }
}
