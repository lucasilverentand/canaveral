//! Integration tests for the embedded tool definition registry, GenericProvider,
//! template expansion, platform resolution, and archive extraction.

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use canaveral_tools::tool_defs::definitions;
use canaveral_tools::tool_defs::schema::{
    FileMapping, PlatformMapping, ToolDefinition, VersionDetect,
};
use canaveral_tools::ToolProvider;

// ---------------------------------------------------------------------------
// SECTION 1: Embedded definition registry – completeness & consistency
// ---------------------------------------------------------------------------

/// All 26 expected tool IDs are present in the registry.
#[test]
fn all_expected_tools_are_registered() {
    let defs = definitions();
    let expected = [
        "ripgrep",
        "fd",
        "bat",
        "delta",
        "just",
        "typos",
        "git-cliff",
        "starship",
        "hyperfine",
        "xh",
        "deno",
        "cargo-nextest",
        "gh",
        "fzf",
        "actionlint",
        "lazygit",
        "k9s",
        "act",
        "yq",
        "jq",
        "direnv",
        "biome",
        "mise",
        "taplo",
        "eza",
        "shellcheck",
        "pnpm",
        "yarn-classic",
        "watchman",
        "gradle",
        "cargo-tauri",
    ];
    for id in &expected {
        assert!(defs.contains_key(*id), "missing tool definition for '{id}'");
    }
}

/// Every definition has at least a `darwin-aarch64` platform mapping,
/// except tools that explicitly don't provide macOS builds (e.g. eza).
#[test]
fn every_definition_has_darwin_aarch64_platform() {
    let defs = definitions();
    // Tools that don't provide macOS builds via GitHub releases
    let no_macos = ["eza", "watchman"];

    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        if no_macos.contains(&id.as_str()) {
            continue;
        }
        assert!(
            def.platforms.contains_key("darwin-aarch64"),
            "tool '{id}' is missing darwin-aarch64 platform"
        );
    }
}

/// Every definition has at least a `linux-x86_64` platform mapping.
#[test]
fn every_definition_has_linux_x86_64_platform() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        assert!(
            def.platforms.contains_key("linux-x86_64"),
            "tool '{id}' is missing linux-x86_64 platform"
        );
    }
}

/// Every definition specifies either an `asset` template or a `url` template.
#[test]
fn every_definition_has_asset_or_url() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        assert!(
            def.asset.is_some() || def.url.is_some(),
            "tool '{id}' has neither asset nor url template"
        );
    }
}

/// Every definition has a non-empty `repo` in `owner/repo` format.
#[test]
fn every_definition_has_valid_repo() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        let parts: Vec<&str> = def.repo.split('/').collect();
        assert_eq!(
            parts.len(),
            2,
            "tool '{id}' repo '{}' should be owner/repo",
            def.repo
        );
        assert!(
            !parts[0].is_empty() && !parts[1].is_empty(),
            "tool '{id}' repo has empty owner or name"
        );
    }
}

/// Every definition has a non-empty binary name.
#[test]
fn every_definition_has_binary() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        assert!(!def.binary.is_empty(), "tool '{id}' has empty binary name");
    }
}

/// The format field is always one of the supported values.
#[test]
fn every_definition_has_valid_format() {
    let defs = definitions();
    let valid_formats = ["tar.gz", "tgz", "zip", "raw", "gz", "tar.xz"];
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        assert!(
            valid_formats.contains(&def.format.as_str()),
            "tool '{id}' has unrecognized format '{}'",
            def.format
        );
    }
}

/// The version_detect regex compiles for every tool.
#[test]
fn every_version_detect_regex_compiles() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        let regex = &def.version_detect.regex;
        let result = regex::Regex::new(regex);
        assert!(
            result.is_ok(),
            "tool '{id}' version_detect regex '{regex}' does not compile: {}",
            result.unwrap_err()
        );
    }
}

/// The version_detect regex has at least one capture group.
#[test]
fn every_version_detect_regex_has_capture_group() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    for (id, def) in &unique {
        let regex = &def.version_detect.regex;
        let re = regex::Regex::new(regex).unwrap();
        assert!(
            re.captures_len() >= 2,
            "tool '{id}' version_detect regex '{regex}' needs at least one capture group"
        );
    }
}

/// Platform overrides reference platform keys that exist in the platforms map.
/// (This catches typos like `platform_overrides.darwin-aarch64` without a
/// corresponding `platforms.darwin-aarch64` entry.)
#[test]
fn platform_overrides_reference_known_platform_keys() {
    let defs = definitions();
    let unique: Vec<_> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .collect();

    // We allow override keys for platforms that are known even if they aren't
    // in this definition's platforms map (e.g. windows). Just check the key
    // follows the os-arch pattern.
    let known_keys = [
        "darwin-aarch64",
        "darwin-x86_64",
        "linux-x86_64",
        "linux-aarch64",
        "windows-x86_64",
        "windows-aarch64",
    ];

    for (id, def) in &unique {
        for key in def.platform_overrides.keys() {
            assert!(
                known_keys.contains(&key.as_str()),
                "tool '{id}' has platform_override for unknown key '{key}'"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// SECTION 2: Alias resolution
// ---------------------------------------------------------------------------

#[test]
fn alias_rg_resolves_to_ripgrep() {
    let defs = definitions();
    let via_alias = &defs["rg"];
    let via_id = &defs["ripgrep"];
    assert_eq!(via_alias.id, via_id.id);
    assert_eq!(via_alias.binary, "rg");
}

#[test]
fn alias_nextest_resolves_to_cargo_nextest() {
    let defs = definitions();
    // cargo-nextest has aliases if any
    let def = &defs["cargo-nextest"];
    for alias in &def.aliases {
        assert!(
            defs.contains_key(alias),
            "alias '{alias}' for cargo-nextest not in registry"
        );
        assert_eq!(defs[alias].id, "cargo-nextest");
    }
}

#[test]
fn no_alias_collides_with_another_tool_id() {
    let defs = definitions();
    // Collect all primary IDs
    let primary_ids: Vec<&str> = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .map(|(key, _)| key.as_str())
        .collect();

    for (_, def) in defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
    {
        for alias in &def.aliases {
            // An alias should not be the primary id of a different tool
            if primary_ids.contains(&alias.as_str()) {
                assert_eq!(
                    alias.as_str(),
                    def.id.as_str(),
                    "alias '{alias}' of '{}' collides with another tool's primary id",
                    def.id
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SECTION 3: Specific tool definitions – field values
// ---------------------------------------------------------------------------

#[test]
fn ripgrep_fields() {
    let def = &definitions()["ripgrep"];
    assert_eq!(def.binary, "rg");
    assert_eq!(def.repo, "BurntSushi/ripgrep");
    assert_eq!(def.tag_prefix, "v");
    assert_eq!(def.format, "tar.gz");
    assert!(def.aliases.contains(&"rg".to_string()));
    assert!(def.asset.as_ref().unwrap().contains("{version}"));
    assert_eq!(def.files.len(), 1);
    assert_eq!(def.files[0].name, "rg");
}

#[test]
fn jq_fields() {
    let def = &definitions()["jq"];
    assert_eq!(def.binary, "jq");
    assert_eq!(def.repo, "jqlang/jq");
    assert_eq!(def.tag_prefix, "jq-");
    assert_eq!(def.format, "raw");
    assert!(def.url.is_some(), "jq should have a direct url template");
    assert!(def.asset.is_none(), "jq should not have an asset template");
}

#[test]
fn biome_fields() {
    let def = &definitions()["biome"];
    assert_eq!(def.binary, "biome");
    assert_eq!(def.repo, "biomejs/biome");
    assert_eq!(def.tag_prefix, "@biomejs/biome@");
    assert_eq!(def.format, "raw");
    assert!(def.url.is_some());
    // URL should contain the URL-encoded @ symbol for the tag
    assert!(def.url.as_ref().unwrap().contains("%40"));
}

#[test]
fn deno_fields() {
    let def = &definitions()["deno"];
    assert_eq!(def.binary, "deno");
    assert_eq!(def.format, "zip");
    assert_eq!(def.version_detect.regex, r"deno (\d+\.\d+\.\d+)");
}

#[test]
fn gh_has_darwin_zip_override() {
    let def = &definitions()["gh"];
    assert_eq!(def.format, "tar.gz");
    let override_darwin = def
        .platform_overrides
        .get("darwin-aarch64")
        .expect("gh should have darwin-aarch64 override");
    assert_eq!(override_darwin.format.as_deref(), Some("zip"));
    assert!(override_darwin.asset.as_ref().unwrap().ends_with(".zip"));
}

#[test]
fn direnv_uses_custom_version_detect_args() {
    let def = &definitions()["direnv"];
    assert_eq!(def.version_detect.args, vec!["version"]);
}

#[test]
fn git_cliff_has_versioned_src_path() {
    let def = &definitions()["git-cliff"];
    assert_eq!(def.files.len(), 1);
    let src = def.files[0].src.as_ref().unwrap();
    assert!(
        src.contains("{version}"),
        "git-cliff src path should contain {{version}}"
    );
}

#[test]
fn starship_linux_aarch64_uses_gnu() {
    let def = &definitions()["starship"];
    let platform = &def.platforms["linux-aarch64"];
    assert!(
        platform.os.contains("gnu"),
        "starship linux-aarch64 should use gnu, got '{}'",
        platform.os
    );
}

// ---------------------------------------------------------------------------
// SECTION 4: Schema parsing edge cases
// ---------------------------------------------------------------------------

#[test]
fn schema_minimal_definition() {
    let toml_str = r#"
id = "minimal"
name = "Minimal"
binary = "minimal"
repo = "owner/minimal"
"#;
    let def: ToolDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.tag_prefix, "v");
    assert_eq!(def.format, "tar.gz");
    assert!(def.aliases.is_empty());
    assert!(def.files.is_empty());
    assert!(def.platforms.is_empty());
    assert!(def.platform_overrides.is_empty());
    assert!(def.asset.is_none());
    assert!(def.url.is_none());
    assert_eq!(def.version_detect.args, vec!["--version"]);
    assert_eq!(def.version_detect.regex, r"(\d+\.\d+\.\d+)");
}

#[test]
fn schema_with_all_fields() {
    let toml_str = r#"
id = "full"
name = "Full Tool"
binary = "fulltool"
repo = "owner/full"
tag_prefix = "release-"
aliases = ["ft", "full-tool"]
format = "zip"
asset = "full-{version}-{os}-{arch}.zip"
url = "https://example.com/{version}/full.zip"

[version_detect]
args = ["-V"]
regex = 'Full (\d+\.\d+)'

[[files]]
name = "fulltool"
src = "full-{version}/bin/fulltool"

[[files]]
name = "fulltool-helper"
src = "full-{version}/bin/helper"

[platforms.darwin-aarch64]
os = "macos"
arch = "arm64"

[platforms.linux-x86_64]
os = "linux"
arch = "x64"

[platform_overrides.linux-x86_64]
format = "tar.gz"
asset = "full-{version}-linux-x64.tar.gz"
"#;
    let def: ToolDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.tag_prefix, "release-");
    assert_eq!(def.aliases, vec!["ft", "full-tool"]);
    assert_eq!(def.format, "zip");
    assert!(def.asset.is_some());
    assert!(def.url.is_some());
    assert_eq!(def.version_detect.args, vec!["-V"]);
    assert_eq!(def.version_detect.regex, r"Full (\d+\.\d+)");
    assert_eq!(def.files.len(), 2);
    assert_eq!(def.files[1].name, "fulltool-helper");
    assert_eq!(def.platforms.len(), 2);
    assert_eq!(def.platform_overrides.len(), 1);
    let linux_override = &def.platform_overrides["linux-x86_64"];
    assert_eq!(linux_override.format.as_deref(), Some("tar.gz"));
    assert!(linux_override.asset.as_ref().unwrap().contains("tar.gz"));
}

#[test]
fn schema_files_without_src_uses_name() {
    let toml_str = r#"
id = "simple"
name = "Simple"
binary = "simple"
repo = "owner/simple"

[[files]]
name = "simple"
"#;
    let def: ToolDefinition = toml::from_str(toml_str).unwrap();
    assert_eq!(def.files.len(), 1);
    assert_eq!(def.files[0].name, "simple");
    assert!(def.files[0].src.is_none());
}

#[test]
fn schema_empty_aliases_vec() {
    let toml_str = r#"
id = "noalias"
name = "No Alias"
binary = "noalias"
repo = "owner/noalias"
aliases = []
"#;
    let def: ToolDefinition = toml::from_str(toml_str).unwrap();
    assert!(def.aliases.is_empty());
}

#[test]
fn schema_rejects_missing_required_fields() {
    // Missing `binary`
    let toml_str = r#"
id = "broken"
name = "Broken"
repo = "owner/broken"
"#;
    let result = toml::from_str::<ToolDefinition>(toml_str);
    assert!(result.is_err());
}

#[test]
fn schema_rejects_missing_id() {
    let toml_str = r#"
name = "Broken"
binary = "broken"
repo = "owner/broken"
"#;
    let result = toml::from_str::<ToolDefinition>(toml_str);
    assert!(result.is_err());
}

#[test]
fn schema_rejects_missing_repo() {
    let toml_str = r#"
id = "broken"
name = "Broken"
binary = "broken"
"#;
    let result = toml::from_str::<ToolDefinition>(toml_str);
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// SECTION 5: Template expansion
// ---------------------------------------------------------------------------

// We can't call expand() directly from an integration test since it's a
// private module. Instead we test it indirectly through GenericProvider's
// build_download_url. For direct template tests, see the unit tests in
// template.rs. Here we test the end-to-end URL construction.

// ---------------------------------------------------------------------------
// SECTION 6: GenericProvider – URL construction
// ---------------------------------------------------------------------------

use canaveral_tools::providers::GenericProvider;

fn make_definition(overrides: impl FnOnce(&mut ToolDefinition)) -> ToolDefinition {
    let mut platforms = HashMap::new();
    // Add all major platforms so tests work on any CI runner
    platforms.insert(
        "darwin-aarch64".to_string(),
        PlatformMapping {
            os: "apple-darwin".to_string(),
            arch: "aarch64".to_string(),
        },
    );
    platforms.insert(
        "darwin-x86_64".to_string(),
        PlatformMapping {
            os: "apple-darwin".to_string(),
            arch: "x86_64".to_string(),
        },
    );
    platforms.insert(
        "linux-x86_64".to_string(),
        PlatformMapping {
            os: "unknown-linux-musl".to_string(),
            arch: "x86_64".to_string(),
        },
    );
    platforms.insert(
        "linux-aarch64".to_string(),
        PlatformMapping {
            os: "unknown-linux-gnu".to_string(),
            arch: "aarch64".to_string(),
        },
    );

    let mut def = ToolDefinition {
        id: "test-tool".to_string(),
        name: "Test Tool".to_string(),
        binary: "test-tool".to_string(),
        repo: "owner/test-tool".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: Some("{arch}-{os}-{version}.tar.gz".to_string()),
        url: None,
        format: "tar.gz".to_string(),
        files: vec![],
        platforms,
        platform_overrides: HashMap::new(),
    };
    overrides(&mut def);
    def
}

/// Provider id/name/binary come from the definition.
#[test]
fn generic_provider_exposes_definition_identity() {
    let def = make_definition(|d| {
        d.id = "custom-id".to_string();
        d.name = "Custom Name".to_string();
        d.binary = "custom-bin".to_string();
    });
    let provider = GenericProvider::new(def);
    assert_eq!(provider.id(), "custom-id");
    assert_eq!(provider.name(), "Custom Name");
    assert_eq!(provider.binary_name(), "custom-bin");
}

/// from_repo produces sensible defaults.
#[test]
fn generic_provider_from_repo_defaults() {
    let provider = GenericProvider::from_repo("mytool", "myorg/mytool");
    assert_eq!(provider.id(), "mytool");
    assert_eq!(provider.name(), "mytool");
    assert_eq!(provider.binary_name(), "mytool");
}

/// env_vars prepends the install path to PATH.
#[test]
fn generic_provider_env_vars_prepends_path() {
    let def = make_definition(|_| {});
    let provider = GenericProvider::new(def);
    let vars = provider.env_vars(Path::new("/some/bin"));
    assert_eq!(vars.len(), 1);
    assert_eq!(vars[0].0, "PATH");
    assert!(vars[0].1.starts_with("/some/bin"));
}

// ---------------------------------------------------------------------------
// SECTION 7: Download URL construction per-tool
//
// For each embedded tool we verify the constructed URL looks correct for
// a given version and the current platform. This catches template bugs
// and wrong platform mappings.
// ---------------------------------------------------------------------------

/// Helper: parse the real embedded definition and build a GenericProvider,
/// then call build_download_url and return the result.
fn url_for(tool_id: &str, version: &str) -> String {
    let def = definitions()
        .get(tool_id)
        .unwrap_or_else(|| panic!("missing definition for '{tool_id}'"));
    // build_download_url is private, so we reconstruct the expected URL
    // using the definition's templates and the current platform.
    let platform_key = current_platform_key();
    let (os, arch) = if let Some(pm) = def.platforms.get(platform_key) {
        (pm.os.as_str(), pm.arch.as_str())
    } else {
        panic!("tool '{tool_id}' missing platform '{platform_key}'");
    };

    let overrides = def.platform_overrides.get(platform_key);
    let url_template = overrides
        .and_then(|o| o.url.as_deref())
        .or(def.url.as_deref());
    let asset_template = overrides
        .and_then(|o| o.asset.as_deref())
        .or(def.asset.as_deref());

    if let Some(url_tmpl) = url_template {
        return url_tmpl
            .replace("{version}", version)
            .replace("{os}", os)
            .replace("{arch}", arch);
    }

    let asset = asset_template.expect("need asset or url");
    let expanded_asset = asset
        .replace("{version}", version)
        .replace("{os}", os)
        .replace("{arch}", arch);
    let tag = format!("{}{}", def.tag_prefix, version);
    format!(
        "https://github.com/{}/releases/download/{}/{}",
        def.repo, tag, expanded_asset
    )
}

fn current_platform_key() -> &'static str {
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
    } else {
        "unknown"
    }
}

#[test]
fn url_ripgrep() {
    let url = url_for("ripgrep", "14.1.1");
    assert!(url.contains("github.com/BurntSushi/ripgrep"));
    assert!(url.contains("14.1.1"));
    assert!(url.contains("/v14.1.1/"));
}

#[test]
fn url_fd() {
    let url = url_for("fd", "10.2.0");
    assert!(url.contains("github.com/sharkdp/fd"));
    assert!(url.contains("/v10.2.0/"));
    // fd includes "v" in the asset name
    assert!(url.contains("fd-v10.2.0"));
}

#[test]
fn url_bat() {
    let url = url_for("bat", "0.24.0");
    assert!(url.contains("github.com/sharkdp/bat"));
    assert!(url.contains("0.24.0"));
}

#[test]
fn url_delta() {
    let url = url_for("delta", "0.18.2");
    assert!(url.contains("github.com/dandavison/delta"));
    assert!(url.contains("0.18.2"));
}

#[test]
fn url_just() {
    let url = url_for("just", "1.36.0");
    assert!(url.contains("github.com/casey/just"));
    assert!(url.contains("1.36.0"));
}

#[test]
fn url_jq() {
    let url = url_for("jq", "1.7.1");
    assert!(url.contains("github.com/jqlang/jq"));
    // jq uses tag_prefix "jq-" so the tag is "jq-1.7.1"
    assert!(url.contains("/jq-1.7.1/"));
}

#[test]
fn url_biome() {
    let url = url_for("biome", "2.0.0");
    assert!(url.contains("github.com/biomejs/biome"));
    assert!(url.contains("2.0.0"));
    // Uses URL-encoded @ in tag
    assert!(url.contains("%40biomejs%2Fbiome%40"));
}

#[test]
fn url_direnv() {
    let url = url_for("direnv", "2.35.0");
    assert!(url.contains("github.com/direnv/direnv"));
    assert!(url.contains("/v2.35.0/"));
    assert!(url.contains("direnv."));
}

#[test]
fn url_deno() {
    let url = url_for("deno", "2.1.0");
    assert!(url.contains("github.com/denoland/deno"));
    assert!(url.contains("/v2.1.0/"));
    assert!(url.ends_with(".zip"));
}

#[test]
fn url_gh() {
    let url = url_for("gh", "2.63.0");
    assert!(url.contains("github.com/cli/cli"));
    assert!(url.contains("/v2.63.0/"));
    assert!(url.contains("gh_2.63.0_"));
}

#[test]
fn url_starship() {
    let url = url_for("starship", "1.21.1");
    assert!(url.contains("github.com/starship/starship"));
    assert!(url.contains("/v1.21.1/"));
}

#[test]
fn url_git_cliff() {
    let url = url_for("git-cliff", "2.7.0");
    assert!(url.contains("github.com/orhun/git-cliff"));
    assert!(url.contains("/v2.7.0/"));
    assert!(url.contains("git-cliff-2.7.0"));
}

#[test]
fn url_hyperfine() {
    let url = url_for("hyperfine", "1.19.0");
    assert!(url.contains("github.com/sharkdp/hyperfine"));
    assert!(url.contains("1.19.0"));
}

#[test]
fn url_fzf() {
    let url = url_for("fzf", "0.57.0");
    assert!(url.contains("github.com/junegunn/fzf"));
    assert!(url.contains("0.57.0"));
}

#[test]
fn url_lazygit() {
    let url = url_for("lazygit", "0.44.1");
    assert!(url.contains("github.com/jesseduffield/lazygit"));
    assert!(url.contains("0.44.1"));
}

#[test]
fn url_k9s() {
    let url = url_for("k9s", "0.32.7");
    assert!(url.contains("github.com/derailed/k9s"));
    assert!(url.contains("0.32.7"));
}

#[test]
fn url_yq() {
    let url = url_for("yq", "4.44.6");
    assert!(url.contains("github.com/mikefarah/yq"));
    assert!(url.contains("4.44.6"));
}

#[test]
fn url_act() {
    let url = url_for("act", "0.2.70");
    assert!(url.contains("github.com/nektos/act"));
    assert!(url.contains("0.2.70"));
}

#[test]
fn url_actionlint() {
    let url = url_for("actionlint", "1.7.7");
    assert!(url.contains("github.com/rhysd/actionlint"));
    assert!(url.contains("1.7.7"));
}

#[test]
fn url_typos() {
    let url = url_for("typos", "1.28.4");
    assert!(url.contains("github.com/crate-ci/typos"));
    assert!(url.contains("1.28.4"));
}

#[test]
fn url_xh() {
    let url = url_for("xh", "0.23.1");
    assert!(url.contains("github.com/ducaale/xh"));
    assert!(url.contains("0.23.1"));
}

#[test]
fn url_cargo_nextest() {
    let url = url_for("cargo-nextest", "0.9.86");
    assert!(url.contains("github.com/nextest-rs/nextest"));
    assert!(url.contains("0.9.86"));
}

#[test]
fn url_eza() {
    let def = definitions().get("eza").unwrap().clone();
    let platform_key = current_platform_key();
    if !def.platforms.contains_key(platform_key) {
        // eza doesn't provide macOS builds — skip on darwin
        return;
    }
    let url = url_for("eza", "0.20.14");
    assert!(url.contains("github.com/eza-community/eza"));
    assert!(url.contains("0.20.14"));
}

#[test]
fn url_mise() {
    let url = url_for("mise", "2025.1.1");
    assert!(url.contains("github.com/jdx/mise"));
    assert!(url.contains("2025.1.1"));
}

#[test]
fn url_taplo() {
    let url = url_for("taplo", "0.9.3");
    assert!(url.contains("github.com/tamasfe/taplo"));
    assert!(url.contains("0.9.3"));
}

#[test]
fn url_shellcheck() {
    let url = url_for("shellcheck", "0.10.0");
    assert!(url.contains("github.com/koalaman/shellcheck"));
    assert!(url.contains("0.10.0"));
}

// ---------------------------------------------------------------------------
// SECTION 8: GenericProvider – tar.gz extraction
// ---------------------------------------------------------------------------

/// Create a tar.gz archive in memory with the given files.
fn create_tar_gz(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut builder = tar::Builder::new(Vec::new());
    for (path, content) in files {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o755);
        header.set_cksum();
        builder.append_data(&mut header, path, *content).unwrap();
    }
    let tar_bytes = builder.into_inner().unwrap();

    let mut gz_bytes = Vec::new();
    let mut encoder = flate2::write::GzEncoder::new(&mut gz_bytes, flate2::Compression::fast());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap();
    gz_bytes
}

/// Create a zip archive in memory with the given files.
fn create_zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let buf = std::io::Cursor::new(Vec::new());
    let mut writer = zip::ZipWriter::new(buf);
    let options =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (path, content) in files {
        writer.start_file(*path, options).unwrap();
        writer.write_all(content).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

/// Spin up a tiny HTTP server that serves archive bytes, then install via
/// GenericProvider and verify the extracted binary.
#[tokio::test]
async fn extract_tar_gz_with_file_mapping() {
    let binary_content = b"#!/bin/sh\necho hello";
    let archive = create_tar_gz(&[("mytool-1.0.0/bin/mytool", binary_content)]);

    // Start a local HTTP server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let archive_clone = archive.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
            archive_clone.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut socket, &archive_clone)
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "mytool".to_string(),
        name: "My Tool".to_string(),
        binary: "mytool".to_string(),
        repo: "owner/mytool".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/mytool.tar.gz")),
        format: "tar.gz".to_string(),
        files: vec![FileMapping {
            name: "mytool".to_string(),
            src: Some("mytool-{version}/bin/mytool".to_string()),
        }],
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("1.0.0", tmp.path()).await;

    server.abort();

    let result = result.unwrap();
    assert_eq!(result.tool, "mytool");
    assert_eq!(result.version, "1.0.0");

    let binary_path = result.install_path.join("mytool");
    assert!(binary_path.exists(), "binary should be extracted");
    let content = std::fs::read(&binary_path).unwrap();
    assert_eq!(content, binary_content);

    // Verify executable permissions on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&binary_path)
            .unwrap()
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "binary should be executable");
    }
}

#[tokio::test]
async fn extract_zip_archive() {
    let binary_content = b"fake-zip-binary";
    let archive = create_zip(&[("myzip-2.0.0/bin/myzip", binary_content)]);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let archive_clone = archive.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
            archive_clone.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut socket, &archive_clone)
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "myzip".to_string(),
        name: "My Zip".to_string(),
        binary: "myzip".to_string(),
        repo: "owner/myzip".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/myzip.zip")),
        format: "zip".to_string(),
        files: vec![FileMapping {
            name: "myzip".to_string(),
            src: Some("myzip-{version}/bin/myzip".to_string()),
        }],
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("2.0.0", tmp.path()).await;

    server.abort();

    let result = result.unwrap();
    let binary_path = result.install_path.join("myzip");
    assert!(binary_path.exists());
    assert_eq!(std::fs::read(&binary_path).unwrap(), binary_content);
}

#[tokio::test]
async fn extract_raw_binary() {
    let binary_content = b"#!/bin/sh\necho raw";

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let content_clone = binary_content.to_vec();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
            content_clone.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut socket, &content_clone)
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "rawbin".to_string(),
        name: "Raw Binary".to_string(),
        binary: "rawbin".to_string(),
        repo: "owner/rawbin".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/rawbin")),
        format: "raw".to_string(),
        files: vec![],
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("3.0.0", tmp.path()).await;

    server.abort();

    let result = result.unwrap();
    let binary_path = result.install_path.join("rawbin");
    assert!(binary_path.exists());
    assert_eq!(std::fs::read(&binary_path).unwrap(), binary_content);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&binary_path)
            .unwrap()
            .permissions()
            .mode();
        assert!(mode & 0o111 != 0, "raw binary should be executable");
    }
}

#[tokio::test]
async fn extract_tar_gz_without_file_mapping() {
    // When no file mappings are specified, all files in the archive should be
    // extracted to bin/
    let binary_a = b"binary-a-content";
    let binary_b = b"binary-b-content";
    let archive = create_tar_gz(&[("somedir/tool-a", binary_a), ("somedir/tool-b", binary_b)]);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let archive_clone = archive.clone();
    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\n\r\n",
            archive_clone.len()
        );
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
        tokio::io::AsyncWriteExt::write_all(&mut socket, &archive_clone)
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "multi".to_string(),
        name: "Multi".to_string(),
        binary: "tool-a".to_string(),
        repo: "owner/multi".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/multi.tar.gz")),
        format: "tar.gz".to_string(),
        files: vec![], // no file mapping
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("1.0.0", tmp.path()).await;

    server.abort();

    let result = result.unwrap();
    // Both files should be extracted by filename
    assert!(result.install_path.join("tool-a").exists());
    assert!(result.install_path.join("tool-b").exists());
    assert_eq!(
        std::fs::read(result.install_path.join("tool-a")).unwrap(),
        binary_a
    );
    assert_eq!(
        std::fs::read(result.install_path.join("tool-b")).unwrap(),
        binary_b
    );
}

// ---------------------------------------------------------------------------
// SECTION 9: Error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn install_returns_error_on_http_404() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "notfound".to_string(),
        name: "Not Found".to_string(),
        binary: "notfound".to_string(),
        repo: "owner/notfound".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/notfound")),
        format: "raw".to_string(),
        files: vec![],
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("1.0.0", tmp.path()).await;

    server.abort();

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("404"), "error should mention 404: {err}");
}

#[tokio::test]
async fn install_returns_error_for_unsupported_format() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = tokio::io::AsyncReadExt::read(&mut socket, &mut buf).await;
        let response =
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\nContent-Type: application/octet-stream\r\n\r\ndata";
        tokio::io::AsyncWriteExt::write_all(&mut socket, response.as_bytes())
            .await
            .unwrap();
    });

    let def = ToolDefinition {
        id: "badformat".to_string(),
        name: "Bad Format".to_string(),
        binary: "badformat".to_string(),
        repo: "owner/badformat".to_string(),
        tag_prefix: "v".to_string(),
        aliases: vec![],
        version_detect: VersionDetect::default(),
        asset: None,
        url: Some(format!("http://{addr}/badformat")),
        format: "7z".to_string(), // unsupported
        files: vec![],
        platforms: {
            let mut m = HashMap::new();
            let key = current_platform_key().to_string();
            m.insert(
                key,
                PlatformMapping {
                    os: "test-os".to_string(),
                    arch: "test-arch".to_string(),
                },
            );
            m
        },
        platform_overrides: HashMap::new(),
    };

    let provider = GenericProvider::new(def);
    let tmp = tempfile::TempDir::new().unwrap();
    let result = provider.install_to_cache("1.0.0", tmp.path()).await;

    server.abort();

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("unsupported"),
        "error should mention unsupported format: {err}"
    );
}

// ---------------------------------------------------------------------------
// SECTION 10: Registry integration
// ---------------------------------------------------------------------------

use canaveral_tools::registry::ToolRegistry;

#[test]
fn registry_finds_embedded_definitions() {
    let registry = ToolRegistry::empty();
    // Even with an empty registry (no builtins), the fallback to embedded
    // definitions should work.
    let provider = registry.get("ripgrep");
    assert!(
        provider.is_some(),
        "empty registry should fall back to embedded defs"
    );
    assert_eq!(provider.unwrap().id(), "ripgrep");
}

#[test]
fn registry_finds_tool_by_alias() {
    let registry = ToolRegistry::empty();
    let provider = registry.get("rg");
    assert!(provider.is_some(), "should find ripgrep via 'rg' alias");
    assert_eq!(provider.unwrap().id(), "ripgrep");
}

#[test]
fn registry_builtins_override_embedded() {
    let registry = ToolRegistry::with_builtins();
    // node is both a builtin and potentially an embedded definition
    // The builtin should win
    let provider = registry.get("node");
    assert!(provider.is_some());
    assert_eq!(provider.unwrap().id(), "node");
}

#[test]
fn registry_get_with_source_creates_generic_provider() {
    let registry = ToolRegistry::empty();
    let provider = registry.get_with_source("custom-tool", "myorg/custom-tool");
    assert_eq!(provider.id(), "custom-tool");
    assert_eq!(provider.binary_name(), "custom-tool");
}

#[test]
fn registry_all_tools_from_embedded_are_accessible() {
    let registry = ToolRegistry::empty();
    let all_ids = [
        "ripgrep",
        "fd",
        "bat",
        "delta",
        "just",
        "typos",
        "git-cliff",
        "starship",
        "hyperfine",
        "xh",
        "deno",
        "cargo-nextest",
        "gh",
        "fzf",
        "actionlint",
        "lazygit",
        "k9s",
        "act",
        "yq",
        "jq",
        "direnv",
        "biome",
        "mise",
        "taplo",
        "eza",
        "shellcheck",
        "pnpm",
        "yarn-classic",
        "watchman",
        "gradle",
        "cargo-tauri",
    ];
    for id in &all_ids {
        assert!(
            registry.get(id).is_some(),
            "registry.get('{id}') should return a provider"
        );
    }
}

// ---------------------------------------------------------------------------
// SECTION 11: Platform override precedence
// ---------------------------------------------------------------------------

#[test]
fn platform_override_changes_format_and_asset() {
    let toml_str = r#"
id = "overridden"
name = "Overridden"
binary = "overridden"
repo = "owner/overridden"
asset = "overridden-{version}-{os}-{arch}.tar.gz"
format = "tar.gz"

[platforms.darwin-aarch64]
os = "macos"
arch = "arm64"

[platforms.darwin-x86_64]
os = "macos"
arch = "amd64"

[platforms.linux-x86_64]
os = "linux"
arch = "amd64"

[platforms.linux-aarch64]
os = "linux"
arch = "arm64"

[platform_overrides.darwin-aarch64]
format = "zip"
asset = "overridden-{version}-macos-arm64.zip"

[platform_overrides.darwin-x86_64]
format = "zip"
asset = "overridden-{version}-macos-amd64.zip"

[platform_overrides.linux-x86_64]
url = "https://custom.example.com/{version}/overridden-linux"
format = "raw"
"#;
    let def: ToolDefinition = toml::from_str(toml_str).unwrap();
    let platform_key = current_platform_key();

    if let Some(ov) = def.platform_overrides.get(platform_key) {
        // Verify the override is applied
        if platform_key.starts_with("darwin") {
            assert_eq!(ov.format.as_deref(), Some("zip"));
            assert!(ov.asset.as_ref().unwrap().ends_with(".zip"));
        } else if platform_key == "linux-x86_64" {
            assert_eq!(ov.format.as_deref(), Some("raw"));
            assert!(ov.url.is_some());
        }
    }
    // If no override for current platform, that's fine too — the defaults apply
}

// ---------------------------------------------------------------------------
// SECTION 12: Definition count sanity check
// ---------------------------------------------------------------------------

#[test]
fn definitions_map_has_expected_entry_count() {
    let defs = definitions();
    // 26 tools + aliases. Count unique tool IDs.
    let unique_count = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .count();
    assert_eq!(
        unique_count, 31,
        "expected 31 unique tool definitions, got {unique_count}"
    );
}

#[test]
fn definitions_total_entries_includes_aliases() {
    let defs = definitions();
    let unique_count = defs
        .iter()
        .filter(|(key, def)| key.as_str() == def.id.as_str())
        .count();
    // Total entries should be > unique count if any tools have aliases
    assert!(
        defs.len() >= unique_count,
        "total map entries ({}) should be >= unique tools ({unique_count})",
        defs.len()
    );
}

// ---------------------------------------------------------------------------
// SECTION 13: Version detect regex matches real-world output
// ---------------------------------------------------------------------------

#[test]
fn version_detect_regex_matches_ripgrep_output() {
    let def = &definitions()["ripgrep"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "ripgrep 14.1.1";
    let caps = re.captures(output).expect("should match ripgrep output");
    assert_eq!(&caps[1], "14.1.1");
}

#[test]
fn version_detect_regex_matches_jq_output() {
    let def = &definitions()["jq"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "jq-1.7.1";
    let caps = re.captures(output).expect("should match jq output");
    assert_eq!(&caps[1], "1.7.1");
}

#[test]
fn version_detect_regex_matches_deno_output() {
    let def = &definitions()["deno"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "deno 2.1.0 (stable, release, aarch64-apple-darwin)";
    let caps = re.captures(output).expect("should match deno output");
    assert_eq!(&caps[1], "2.1.0");
}

#[test]
fn version_detect_regex_matches_bat_output() {
    let def = &definitions()["bat"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "bat 0.24.0 (871abd2)";
    let caps = re.captures(output).expect("should match bat output");
    assert_eq!(&caps[1], "0.24.0");
}

#[test]
fn version_detect_regex_matches_fd_output() {
    let def = &definitions()["fd"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "fd 10.2.0";
    let caps = re.captures(output).expect("should match fd output");
    assert_eq!(&caps[1], "10.2.0");
}

#[test]
fn version_detect_regex_matches_just_output() {
    let def = &definitions()["just"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "just 1.36.0";
    let caps = re.captures(output).expect("should match just output");
    assert_eq!(&caps[1], "1.36.0");
}

#[test]
fn version_detect_regex_matches_starship_output() {
    let def = &definitions()["starship"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "starship 1.21.1";
    let caps = re.captures(output).expect("should match starship output");
    assert_eq!(&caps[1], "1.21.1");
}

#[test]
fn version_detect_regex_matches_gh_output() {
    let def = &definitions()["gh"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "gh version 2.63.0 (2024-12-04)";
    let caps = re.captures(output).expect("should match gh output");
    assert_eq!(&caps[1], "2.63.0");
}

#[test]
fn version_detect_regex_matches_biome_output() {
    let def = &definitions()["biome"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "Version: 2.0.0";
    let caps = re.captures(output).expect("should match biome output");
    assert_eq!(&caps[1], "2.0.0");
}

#[test]
fn version_detect_regex_matches_fzf_output() {
    let def = &definitions()["fzf"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "0.57.0 (brew)";
    let caps = re.captures(output).expect("should match fzf output");
    assert_eq!(&caps[1], "0.57.0");
}

#[test]
fn version_detect_regex_matches_lazygit_output() {
    let def = &definitions()["lazygit"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    // lazygit outputs to stderr: "commit=xxx, build date=xxx, build source=xxx, version=0.44.1, os=darwin, arch=arm64"
    let output = "commit=abc123, build date=2024-01-01, version=0.44.1, os=darwin, arch=arm64";
    let caps = re.captures(output).expect("should match lazygit output");
    assert_eq!(&caps[1], "0.44.1");
}

#[test]
fn version_detect_regex_matches_k9s_output() {
    let def = &definitions()["k9s"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = " ____  __.________       \n|    |/ _/   __   \\______\nVersion:    v0.32.7";
    let caps = re.captures(output).expect("should match k9s output");
    assert_eq!(&caps[1], "0.32.7");
}

#[test]
fn version_detect_regex_matches_direnv_output() {
    let def = &definitions()["direnv"];
    let re = regex::Regex::new(&def.version_detect.regex).unwrap();
    let output = "2.35.0";
    let caps = re.captures(output).expect("should match direnv output");
    assert_eq!(&caps[1], "2.35.0");
}
