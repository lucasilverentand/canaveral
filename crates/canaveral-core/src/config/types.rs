//! Configuration types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Main configuration for Canaveral
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Version of the config schema
    #[serde(rename = "$schema")]
    pub schema: Option<String>,

    /// Project name
    pub name: Option<String>,

    /// Versioning configuration
    pub versioning: VersioningConfig,

    /// Git configuration
    pub git: GitConfig,

    /// Changelog configuration
    pub changelog: ChangelogConfig,

    /// Package configurations
    #[serde(default)]
    pub packages: Vec<PackageConfig>,

    /// Hooks configuration
    #[serde(default)]
    pub hooks: HooksConfig,

    /// Publishing configuration
    pub publish: PublishConfig,

    /// Code signing configuration
    #[serde(default)]
    pub signing: SigningConfig,

    /// App store configurations
    #[serde(default)]
    pub stores: StoresConfig,

    /// Metadata management configuration
    #[serde(default)]
    pub metadata: MetadataConfig,

    /// Task orchestration configuration
    #[serde(default)]
    pub tasks: TasksConfig,

    /// CI/CD configuration
    #[serde(default)]
    pub ci: CIConfig,

    /// PR validation configuration
    #[serde(default)]
    pub pr: PrConfig,

    /// Release notes configuration
    #[serde(default)]
    pub release_notes: ReleaseNotesConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            schema: None,
            name: None,
            versioning: VersioningConfig::default(),
            git: GitConfig::default(),
            changelog: ChangelogConfig::default(),
            packages: Vec::new(),
            hooks: HooksConfig::default(),
            publish: PublishConfig::default(),
            signing: SigningConfig::default(),
            stores: StoresConfig::default(),
            metadata: MetadataConfig::default(),
            tasks: TasksConfig::default(),
            ci: CIConfig::default(),
            pr: PrConfig::default(),
            release_notes: ReleaseNotesConfig::default(),
        }
    }
}

/// Versioning configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VersioningConfig {
    /// Version strategy (semver, calver, etc.)
    pub strategy: String,

    /// Tag format (e.g., "v{version}")
    pub tag_format: String,

    /// Whether to use independent versioning in monorepos
    pub independent: bool,

    /// Pre-release identifier
    pub prerelease_identifier: Option<String>,

    /// Build metadata
    pub build_metadata: Option<String>,
}

impl Default for VersioningConfig {
    fn default() -> Self {
        Self {
            strategy: "semver".to_string(),
            tag_format: "v{version}".to_string(),
            independent: false,
            prerelease_identifier: None,
            build_metadata: None,
        }
    }
}

/// Git configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    /// Remote name
    pub remote: String,

    /// Branch to release from
    pub branch: String,

    /// Whether to require clean working directory
    pub require_clean: bool,

    /// Whether to push tags
    pub push_tags: bool,

    /// Whether to push commits
    pub push_commits: bool,

    /// Commit message template
    pub commit_message: String,

    /// Whether to sign commits
    pub sign_commits: bool,

    /// Whether to sign tags
    pub sign_tags: bool,
}

impl Default for GitConfig {
    fn default() -> Self {
        Self {
            remote: "origin".to_string(),
            branch: "main".to_string(),
            require_clean: true,
            push_tags: true,
            push_commits: true,
            commit_message: "chore(release): {version}".to_string(),
            sign_commits: false,
            sign_tags: false,
        }
    }
}

/// Changelog configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChangelogConfig {
    /// Whether to generate changelog
    pub enabled: bool,

    /// Changelog file path
    pub file: PathBuf,

    /// Changelog format (markdown, etc.)
    pub format: String,

    /// Commit types to include
    #[serde(default)]
    pub types: HashMap<String, CommitTypeConfig>,

    /// Header template
    pub header: Option<String>,

    /// Whether to include commit hashes
    pub include_hashes: bool,

    /// Whether to include authors
    pub include_authors: bool,

    /// Whether to include dates
    pub include_dates: bool,
}

impl Default for ChangelogConfig {
    fn default() -> Self {
        let mut types = HashMap::new();
        types.insert(
            "feat".to_string(),
            CommitTypeConfig {
                section: "Features".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "fix".to_string(),
            CommitTypeConfig {
                section: "Bug Fixes".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "docs".to_string(),
            CommitTypeConfig {
                section: "Documentation".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "perf".to_string(),
            CommitTypeConfig {
                section: "Performance".to_string(),
                hidden: false,
            },
        );
        types.insert(
            "refactor".to_string(),
            CommitTypeConfig {
                section: "Refactoring".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "test".to_string(),
            CommitTypeConfig {
                section: "Tests".to_string(),
                hidden: true,
            },
        );
        types.insert(
            "chore".to_string(),
            CommitTypeConfig {
                section: "Chores".to_string(),
                hidden: true,
            },
        );

        Self {
            enabled: true,
            file: PathBuf::from("CHANGELOG.md"),
            format: "markdown".to_string(),
            types,
            header: None,
            include_hashes: true,
            include_authors: false,
            include_dates: true,
        }
    }
}

/// Configuration for a commit type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitTypeConfig {
    /// Section header in changelog
    pub section: String,
    /// Whether to hide this type from changelog
    pub hidden: bool,
}

/// Package-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    /// Package name
    pub name: String,

    /// Path to package (relative to repo root)
    pub path: PathBuf,

    /// Package type (npm, cargo, python, etc.)
    #[serde(rename = "type")]
    pub package_type: String,

    /// Whether to publish this package
    #[serde(default = "default_true")]
    pub publish: bool,

    /// Custom registry URL
    pub registry: Option<String>,

    /// Package-specific tag format
    pub tag_format: Option<String>,

    /// Files to update with version
    #[serde(default)]
    pub version_files: Vec<PathBuf>,
}

fn default_true() -> bool {
    true
}

/// Hooks configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct HooksConfig {
    /// Commands to run before version bump
    #[serde(default)]
    pub pre_version: Vec<String>,

    /// Commands to run after version bump
    #[serde(default)]
    pub post_version: Vec<String>,

    /// Commands to run before changelog generation
    #[serde(default)]
    pub pre_changelog: Vec<String>,

    /// Commands to run after changelog generation
    #[serde(default)]
    pub post_changelog: Vec<String>,

    /// Commands to run before publishing
    #[serde(default)]
    pub pre_publish: Vec<String>,

    /// Commands to run after publishing
    #[serde(default)]
    pub post_publish: Vec<String>,

    /// Commands to run before git operations
    #[serde(default)]
    pub pre_git: Vec<String>,

    /// Commands to run after git operations
    #[serde(default)]
    pub post_git: Vec<String>,
}

/// Publishing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PublishConfig {
    /// Whether to publish packages
    pub enabled: bool,

    /// Registry configurations
    #[serde(default)]
    pub registries: HashMap<String, RegistryConfig>,

    /// Dry run mode
    pub dry_run: bool,
}

impl Default for PublishConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            registries: HashMap::new(),
            dry_run: false,
        }
    }
}

/// Registry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry URL
    pub url: String,

    /// Authentication token environment variable
    pub token_env: Option<String>,
}

/// Code signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SigningConfig {
    /// Whether signing is enabled
    pub enabled: bool,

    /// Signing provider to use (macos, windows, android, gpg)
    pub provider: Option<String>,

    /// Signing identity (certificate name, fingerprint, or key ID)
    pub identity: Option<String>,

    /// macOS-specific signing options
    pub macos: MacOSSigningConfig,

    /// Windows-specific signing options
    pub windows: WindowsSigningConfig,

    /// Android-specific signing options
    pub android: AndroidSigningConfig,

    /// GPG-specific signing options
    pub gpg: GpgSigningConfig,

    /// Artifacts to sign (glob patterns)
    #[serde(default)]
    pub artifacts: Vec<String>,

    /// Whether to verify signatures after signing
    pub verify_after_sign: bool,
}

impl Default for SigningConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: None,
            identity: None,
            macos: MacOSSigningConfig::default(),
            windows: WindowsSigningConfig::default(),
            android: AndroidSigningConfig::default(),
            gpg: GpgSigningConfig::default(),
            artifacts: Vec::new(),
            verify_after_sign: true,
        }
    }
}

/// macOS-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MacOSSigningConfig {
    /// Enable hardened runtime
    pub hardened_runtime: bool,

    /// Path to entitlements file
    pub entitlements: Option<PathBuf>,

    /// Enable timestamping
    pub timestamp: bool,

    /// Deep signing (sign nested code)
    pub deep: bool,

    /// Notarize after signing
    pub notarize: bool,

    /// Apple ID for notarization
    pub apple_id: Option<String>,

    /// App Store Connect API key ID
    pub api_key_id: Option<String>,

    /// App Store Connect API issuer ID
    pub api_issuer_id: Option<String>,

    /// Path to App Store Connect API private key
    pub api_key_path: Option<PathBuf>,

    /// Team ID for notarization
    pub team_id: Option<String>,
}

impl Default for MacOSSigningConfig {
    fn default() -> Self {
        Self {
            hardened_runtime: true,
            entitlements: None,
            timestamp: true,
            deep: true,
            notarize: false,
            apple_id: None,
            api_key_id: None,
            api_issuer_id: None,
            api_key_path: None,
            team_id: None,
        }
    }
}

/// Windows-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowsSigningConfig {
    /// Timestamp server URL
    pub timestamp_url: Option<String>,

    /// Hash algorithm (sha256, sha384, sha512)
    pub algorithm: String,

    /// Description to embed in signature
    pub description: Option<String>,

    /// URL to embed in signature
    pub description_url: Option<String>,

    /// Path to PFX certificate file (alternative to store)
    pub certificate_file: Option<PathBuf>,

    /// Environment variable containing PFX password
    pub certificate_password_env: Option<String>,
}

impl Default for WindowsSigningConfig {
    fn default() -> Self {
        Self {
            timestamp_url: Some("http://timestamp.digicert.com".to_string()),
            algorithm: "sha256".to_string(),
            description: None,
            description_url: None,
            certificate_file: None,
            certificate_password_env: None,
        }
    }
}

/// Android-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AndroidSigningConfig {
    /// Path to keystore file
    pub keystore: Option<PathBuf>,

    /// Key alias in the keystore
    pub key_alias: Option<String>,

    /// Environment variable containing keystore password
    pub keystore_password_env: Option<String>,

    /// Environment variable containing key password
    pub key_password_env: Option<String>,

    /// V1 (JAR) signing scheme
    pub v1_signing: bool,

    /// V2 (APK) signing scheme
    pub v2_signing: bool,

    /// V3 signing scheme
    pub v3_signing: bool,

    /// V4 signing scheme
    pub v4_signing: bool,
}

impl Default for AndroidSigningConfig {
    fn default() -> Self {
        Self {
            keystore: None,
            key_alias: None,
            keystore_password_env: Some("ANDROID_KEYSTORE_PASSWORD".to_string()),
            key_password_env: Some("ANDROID_KEY_PASSWORD".to_string()),
            v1_signing: true,
            v2_signing: true,
            v3_signing: true,
            v4_signing: false,
        }
    }
}

/// GPG-specific signing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GpgSigningConfig {
    /// GPG key ID or email
    pub key_id: Option<String>,

    /// Create detached signatures
    pub detached: bool,

    /// ASCII armor output
    pub armor: bool,

    /// Environment variable containing passphrase
    pub passphrase_env: Option<String>,

    /// Path to GPG binary
    pub gpg_path: Option<PathBuf>,
}

impl Default for GpgSigningConfig {
    fn default() -> Self {
        Self {
            key_id: None,
            detached: true,
            armor: true,
            passphrase_env: Some("GPG_PASSPHRASE".to_string()),
            gpg_path: None,
        }
    }
}

/// App store and package registry configurations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StoresConfig {
    /// Apple App Store / macOS configuration
    #[serde(default)]
    pub apple: Option<AppleStoreConfig>,

    /// Google Play Store configuration
    #[serde(default)]
    pub google_play: Option<GooglePlayStoreConfig>,

    /// Microsoft Store configuration
    #[serde(default)]
    pub microsoft: Option<MicrosoftStoreConfig>,

    /// NPM registry configuration
    #[serde(default)]
    pub npm: Option<canaveral_stores::NpmConfig>,

    /// Crates.io registry configuration
    #[serde(default)]
    pub crates_io: Option<canaveral_stores::CratesIoConfig>,
}

impl Default for StoresConfig {
    fn default() -> Self {
        Self {
            apple: None,
            google_play: None,
            microsoft: None,
            npm: None,
            crates_io: None,
        }
    }
}

/// Metadata management configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataConfig {
    /// Enable metadata management
    #[serde(default)]
    pub enabled: bool,

    /// Storage configuration
    #[serde(default)]
    pub storage: MetadataStorageConfig,

    /// Default settings
    #[serde(default)]
    pub defaults: MetadataDefaultsConfig,

    /// Validation settings
    #[serde(default)]
    pub validation: MetadataValidationConfig,
}

/// Metadata storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MetadataStorageConfig {
    /// Storage format: "fastlane" or "unified"
    #[serde(default = "default_storage_format")]
    pub format: String,

    /// Base path for metadata files
    #[serde(default = "default_metadata_path")]
    pub path: PathBuf,
}

impl Default for MetadataStorageConfig {
    fn default() -> Self {
        Self {
            format: default_storage_format(),
            path: default_metadata_path(),
        }
    }
}

/// Metadata default settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataDefaultsConfig {
    /// Default locale
    #[serde(default)]
    pub default_locale: Option<String>,

    /// Default support URL
    #[serde(default)]
    pub support_url: Option<String>,

    /// Default privacy policy URL
    #[serde(default)]
    pub privacy_policy_url: Option<String>,
}

/// Metadata validation settings
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MetadataValidationConfig {
    /// Treat warnings as errors
    #[serde(default)]
    pub strict: bool,

    /// Locales that must be present
    #[serde(default)]
    pub required_locales: Vec<String>,
}

fn default_storage_format() -> String {
    "fastlane".to_string()
}

fn default_metadata_path() -> PathBuf {
    PathBuf::from("./metadata")
}

/// Microsoft Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MicrosoftStoreConfig {
    /// Azure AD Tenant ID
    pub tenant_id: String,

    /// Azure AD Application (Client) ID
    pub client_id: String,

    /// Azure AD Client Secret
    pub client_secret: String,

    /// Partner Center Application ID (Store ID)
    pub app_id: String,

    /// Default flight (package flight name) - optional
    pub default_flight: Option<String>,
}

/// Apple App Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleStoreConfig {
    /// App Store Connect API Key ID
    pub api_key_id: String,

    /// API Key Issuer ID
    pub api_issuer_id: String,

    /// Path to .p8 key file or env var name containing key
    pub api_key: String,

    /// Apple Team ID
    pub team_id: Option<String>,

    /// Bundle identifier
    pub app_id: Option<String>,

    /// Notarize before upload
    #[serde(default)]
    pub notarize: bool,

    /// Staple notarization ticket
    #[serde(default)]
    pub staple: bool,

    /// Primary locale for app metadata
    pub primary_locale: Option<String>,
}

/// Google Play Store configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GooglePlayStoreConfig {
    /// Android package name
    pub package_name: String,

    /// Path to service account JSON key file
    pub service_account_key: PathBuf,

    /// Default release track
    #[serde(default)]
    pub default_track: Option<String>,
}

/// Task orchestration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TasksConfig {
    /// Maximum concurrent tasks
    pub concurrency: usize,

    /// Task pipeline definitions
    #[serde(default)]
    pub pipeline: HashMap<String, PipelineTask>,

    /// Cache configuration
    #[serde(default)]
    pub cache: CacheConfig,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            concurrency: 4,
            pipeline: HashMap::new(),
            cache: CacheConfig::default(),
        }
    }
}

/// A task in the pipeline configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PipelineTask {
    /// Shell command to execute
    pub command: Option<String>,

    /// Tasks in the same package that must complete first
    #[serde(default)]
    pub depends_on: Vec<String>,

    /// Whether the same task must complete in dependency packages first
    #[serde(default)]
    pub depends_on_packages: bool,

    /// Output glob patterns (for caching)
    #[serde(default)]
    pub outputs: Vec<String>,

    /// Input glob patterns (for cache key computation)
    #[serde(default)]
    pub inputs: Vec<String>,

    /// Environment variables
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether this is a persistent/long-running task
    #[serde(default)]
    pub persistent: bool,
}

impl Default for PipelineTask {
    fn default() -> Self {
        Self {
            command: None,
            depends_on: Vec::new(),
            depends_on_packages: false,
            outputs: Vec::new(),
            inputs: Vec::new(),
            env: HashMap::new(),
            persistent: false,
        }
    }
}

/// Cache configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,

    /// Cache directory
    pub dir: PathBuf,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: PathBuf::from(".canaveral/cache"),
        }
    }
}

/// CI/CD configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CIConfig {
    /// CI platform (github, gitlab)
    pub platform: String,

    /// CI mode (native, traditional)
    pub mode: String,

    /// Tasks to run on PR
    #[serde(default)]
    pub on_pr: Vec<String>,

    /// Tasks to run on push to main
    #[serde(default)]
    pub on_push_main: Vec<String>,

    /// Tasks to run on tag
    #[serde(default)]
    pub on_tag: Vec<String>,
}

impl Default for CIConfig {
    fn default() -> Self {
        Self {
            platform: "github".to_string(),
            mode: "native".to_string(),
            on_pr: vec!["test".to_string(), "lint".to_string()],
            on_push_main: vec!["test".to_string(), "release".to_string()],
            on_tag: vec!["publish".to_string()],
        }
    }
}

/// PR validation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PrConfig {
    /// Branching model (trunk-based, gitflow, custom)
    pub branching_model: String,

    /// Checks to run on PR validation
    #[serde(default)]
    pub checks: Vec<String>,

    /// Whether to require changelog entry
    pub require_changelog: bool,

    /// Whether to require conventional commits
    pub require_conventional_commits: bool,
}

impl Default for PrConfig {
    fn default() -> Self {
        Self {
            branching_model: "trunk-based".to_string(),
            checks: vec![
                "tests".to_string(),
                "lint".to_string(),
                "commit-format".to_string(),
                "version-conflict".to_string(),
            ],
            require_changelog: false,
            require_conventional_commits: true,
        }
    }
}

/// Release notes generation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReleaseNotesConfig {
    /// Whether to categorize changes
    pub categorize: bool,

    /// Whether to include contributor list
    pub include_contributors: bool,

    /// Whether to include migration guide for breaking changes
    pub include_migration_guide: bool,

    /// Whether to auto-update store metadata with release notes
    pub auto_update_store_metadata: bool,

    /// Locales for release notes
    #[serde(default)]
    pub locales: Vec<String>,
}

impl Default for ReleaseNotesConfig {
    fn default() -> Self {
        Self {
            categorize: true,
            include_contributors: true,
            include_migration_guide: true,
            auto_update_store_metadata: false,
            locales: vec!["en-US".to_string()],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
