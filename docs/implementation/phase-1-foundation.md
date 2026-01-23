# Phase 1: Foundation

**Goal**: Create a working CLI that can bump SemVer versions and generate changelogs for a single-package project.

## Tasks

### 1.1 Project Setup

- [ ] Initialize Cargo workspace
- [ ] Configure clippy and rustfmt
- [ ] Set up test framework
- [ ] Create crate structure
- [ ] Configure CI (GitHub Actions)
- [ ] Set up cross-compilation for releases

**Workspace structure:**
```
canaveral/
├── Cargo.toml              # Workspace manifest
├── crates/
│   ├── canaveral/          # Main binary
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── main.rs
│   ├── canaveral-core/     # Core library
│   ├── canaveral-git/      # Git operations
│   ├── canaveral-strategies/
│   └── canaveral-changelog/
├── tests/                  # Integration tests
├── .github/workflows/
└── rustfmt.toml
```

**Workspace Cargo.toml:**
```toml
[workspace]
resolver = "2"
members = ["crates/*"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT OR Apache-2.0"
repository = "https://github.com/seventwo-studio/canaveral"

[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive", "env", "wrap_help"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"

# Git
git2 = "0.18"

# Async
tokio = { version = "1", features = ["full"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Utilities
semver = "1"
chrono = "0.4"
regex = "1"
glob = "0.3"
once_cell = "1"
```

### 1.2 CLI Framework

- [ ] Set up clap with derive macros
- [ ] Implement command structure
- [ ] Add version and help commands
- [ ] Set up colored output with console crate
- [ ] Implement verbose logging with tracing

**Commands to implement:**
```bash
canaveral --version
canaveral --help
canaveral <command> --help
```

**Main CLI structure:**
```rust
// crates/canaveral/src/main.rs
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "canaveral")]
#[command(about = "Universal release management CLI")]
#[command(version, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: OutputFormat,

    /// Path to config file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Execute a full release
    Release(ReleaseArgs),
    /// Calculate and update version
    Version(VersionArgs),
    /// Generate changelog
    Changelog(ChangelogArgs),
    /// Publish to registries
    Publish(PublishArgs),
    /// Initialize configuration
    Init(InitArgs),
    /// Show release status
    Status,
    /// Validate configuration and credentials
    Validate,
}
```

**Crate structure:**
```
crates/canaveral/src/
├── main.rs            # Entry point
├── cli/
│   ├── mod.rs
│   ├── args.rs        # Argument structs
│   ├── release.rs     # Release command
│   ├── version.rs     # Version command
│   ├── changelog.rs   # Changelog command
│   ├── publish.rs     # Publish command
│   ├── init.rs        # Init command
│   └── output.rs      # Output formatting
└── lib.rs
```

### 1.3 Configuration System

- [ ] YAML configuration loading with serde
- [ ] TOML configuration support
- [ ] Validation with helpful errors
- [ ] Default value handling
- [ ] CLI flag override support
- [ ] Environment variable expansion

**Configuration crate:**
```
crates/canaveral-core/src/
├── lib.rs
├── config/
│   ├── mod.rs
│   ├── loader.rs      # Load and parse config
│   ├── types.rs       # Config structs
│   ├── defaults.rs    # Default values
│   └── validation.rs  # Validate config
└── ...
```

**Config loading:**
```rust
// crates/canaveral-core/src/config/loader.rs
use std::path::Path;
use anyhow::{Context, Result};

pub fn load_config(path: Option<&Path>) -> Result<Config> {
    let config_path = match path {
        Some(p) => p.to_owned(),
        None => find_config_file()?,
    };

    let content = std::fs::read_to_string(&config_path)
        .with_context(|| format!("Failed to read config: {}", config_path.display()))?;

    let config: Config = if config_path.extension().map_or(false, |e| e == "toml") {
        toml::from_str(&content)?
    } else {
        serde_yaml::from_str(&content)?
    };

    validate_config(&config)?;
    Ok(config)
}

fn find_config_file() -> Result<PathBuf> {
    for name in ["canaveral.yaml", "canaveral.yml", "canaveral.toml"] {
        let path = PathBuf::from(name);
        if path.exists() {
            return Ok(path);
        }
    }
    Err(anyhow::anyhow!("No configuration file found"))
}
```

### 1.4 Git Integration

- [ ] Set up git2 crate
- [ ] Read commit history since last tag
- [ ] Get current branch name
- [ ] Check for uncommitted changes
- [ ] Create commits
- [ ] Create annotated tags
- [ ] Push to remote

**Git crate structure:**
```
crates/canaveral-git/src/
├── lib.rs
├── repository.rs      # Repository wrapper
├── commits.rs         # Commit operations
├── tags.rs            # Tag operations
├── remote.rs          # Push operations
└── types.rs           # Git types
```

**Key functions:**
```rust
// crates/canaveral-git/src/lib.rs
use git2::{Repository, Commit, Oid};
use anyhow::Result;

pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    pub fn open(path: &Path) -> Result<Self> {
        let repo = Repository::discover(path)?;
        Ok(Self { repo })
    }

    /// Get commits since the specified tag
    pub fn commits_since_tag(&self, tag_pattern: &str) -> Result<Vec<CommitInfo>> {
        let tag = self.find_latest_tag(tag_pattern)?;
        let tag_commit = tag.map(|t| t.target_id());

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push_head()?;

        if let Some(stop_at) = tag_commit {
            revwalk.hide(stop_at)?;
        }

        let commits = revwalk
            .filter_map(|oid| oid.ok())
            .filter_map(|oid| self.repo.find_commit(oid).ok())
            .map(|c| CommitInfo::from_commit(&c))
            .collect();

        Ok(commits)
    }

    /// Get the latest tag matching pattern
    pub fn find_latest_tag(&self, pattern: &str) -> Result<Option<Tag>> {
        // Implementation
    }

    /// Check if working directory is clean
    pub fn is_clean(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;
        Ok(statuses.is_empty())
    }

    /// Create a commit with the given message
    pub fn commit(&self, message: &str, files: &[&Path]) -> Result<Oid> {
        // Implementation
    }

    /// Create an annotated tag
    pub fn create_tag(&self, name: &str, message: &str) -> Result<()> {
        // Implementation
    }

    /// Push commits and tags to remote
    pub fn push(&self, remote: &str, refspecs: &[&str]) -> Result<()> {
        // Implementation
    }
}
```

### 1.5 Commit Parsing

- [ ] Implement Conventional Commits parser
- [ ] Extract type, scope, description
- [ ] Handle breaking changes (! suffix, BREAKING CHANGE footer)
- [ ] Parse PR/issue references
- [ ] Determine release type from commits

**Changelog crate structure:**
```
crates/canaveral-changelog/src/
├── lib.rs
├── parser/
│   ├── mod.rs
│   ├── conventional.rs  # Conventional Commits
│   ├── angular.rs       # Angular style
│   └── types.rs
├── generator.rs         # Generate changelog
└── formatter.rs         # Format output
```

**Commit parsing:**
```rust
// crates/canaveral-changelog/src/parser/conventional.rs
use regex::Regex;
use once_cell::sync::Lazy;

static COMMIT_PATTERN: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(?P<type>\w+)(?:\((?P<scope>[^)]+)\))?(?P<breaking>!)?: (?P<description>.+)$")
        .unwrap()
});

static BREAKING_FOOTER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?m)^BREAKING[ -]CHANGE:\s*(?P<description>.+)$").unwrap()
});

#[derive(Debug, Clone)]
pub struct ParsedCommit {
    pub hash: String,
    pub commit_type: String,
    pub scope: Option<String>,
    pub description: String,
    pub body: Option<String>,
    pub breaking: bool,
    pub breaking_description: Option<String>,
    pub references: Vec<String>,
}

pub fn parse_commit(message: &str, hash: &str) -> Option<ParsedCommit> {
    let first_line = message.lines().next()?;
    let captures = COMMIT_PATTERN.captures(first_line)?;

    let commit_type = captures.name("type")?.as_str().to_string();
    let scope = captures.name("scope").map(|m| m.as_str().to_string());
    let description = captures.name("description")?.as_str().to_string();
    let breaking_marker = captures.name("breaking").is_some();

    // Check for BREAKING CHANGE footer
    let breaking_footer = BREAKING_FOOTER.captures(message);
    let breaking = breaking_marker || breaking_footer.is_some();
    let breaking_description = breaking_footer
        .map(|c| c.name("description").unwrap().as_str().to_string());

    // Parse references (#123, GH-456, etc.)
    let references = parse_references(message);

    Some(ParsedCommit {
        hash: hash.to_string(),
        commit_type,
        scope,
        description,
        body: extract_body(message),
        breaking,
        breaking_description,
        references,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseType {
    Major,
    Minor,
    Patch,
    None,
}

pub fn determine_release_type(commits: &[ParsedCommit]) -> ReleaseType {
    let mut release_type = ReleaseType::None;

    for commit in commits {
        if commit.breaking {
            return ReleaseType::Major;
        }

        match commit.commit_type.as_str() {
            "feat" => {
                if release_type != ReleaseType::Major {
                    release_type = ReleaseType::Minor;
                }
            }
            "fix" | "perf" => {
                if release_type == ReleaseType::None {
                    release_type = ReleaseType::Patch;
                }
            }
            _ => {}
        }
    }

    release_type
}
```

### 1.6 SemVer Strategy

- [ ] Parse semantic version strings
- [ ] Validate version format
- [ ] Increment major/minor/patch
- [ ] Handle pre-release versions (alpha, beta, rc)
- [ ] Handle build metadata

**Strategy crate:**
```
crates/canaveral-strategies/src/
├── lib.rs
├── traits.rs          # Strategy trait
├── semver.rs          # SemVer implementation
├── calver.rs          # CalVer (placeholder for Phase 4)
└── buildnum.rs        # Build numbers (placeholder for Phase 4)
```

**SemVer implementation:**
```rust
// crates/canaveral-strategies/src/semver.rs
use semver::Version;
use crate::traits::{VersionStrategy, ReleaseType};
use anyhow::Result;

pub struct SemVerStrategy {
    allow_zero_major: bool,
}

impl SemVerStrategy {
    pub fn new() -> Self {
        Self { allow_zero_major: true }
    }

    pub fn bump(&self, version: &Version, release_type: ReleaseType) -> Version {
        let mut new_version = version.clone();

        match release_type {
            ReleaseType::Major => {
                if self.allow_zero_major && version.major == 0 {
                    new_version.minor += 1;
                    new_version.patch = 0;
                } else {
                    new_version.major += 1;
                    new_version.minor = 0;
                    new_version.patch = 0;
                }
            }
            ReleaseType::Minor => {
                new_version.minor += 1;
                new_version.patch = 0;
            }
            ReleaseType::Patch => {
                new_version.patch += 1;
            }
            ReleaseType::None => {}
        }

        // Clear pre-release and build metadata
        new_version.pre = semver::Prerelease::EMPTY;
        new_version.build = semver::BuildMetadata::EMPTY;

        new_version
    }

    pub fn prerelease(&self, version: &Version, identifier: &str) -> Result<Version> {
        let mut new_version = version.clone();

        // Parse existing prerelease or start fresh
        let num = if let Some(current) = parse_prerelease_num(&version.pre, identifier) {
            current + 1
        } else {
            0
        };

        new_version.pre = semver::Prerelease::new(&format!("{}.{}", identifier, num))?;
        Ok(new_version)
    }
}
```

### 1.7 Changelog Generation

- [ ] Group commits by type
- [ ] Format as Markdown
- [ ] Include commit hashes (short)
- [ ] Include PR/issue links
- [ ] Generate date-stamped headers
- [ ] Prepend to existing CHANGELOG.md

**Changelog generation:**
```rust
// crates/canaveral-changelog/src/generator.rs
use chrono::Utc;

pub struct ChangelogGenerator {
    config: ChangelogConfig,
}

impl ChangelogGenerator {
    pub fn generate(&self, commits: &[ParsedCommit], version: &str) -> String {
        let mut sections: HashMap<&str, Vec<&ParsedCommit>> = HashMap::new();

        // Group commits by type
        for commit in commits {
            let section = self.config.sections.get(&commit.commit_type)
                .map(|s| s.as_str())
                .unwrap_or("Other");

            sections.entry(section).or_default().push(commit);
        }

        // Generate markdown
        let mut output = String::new();
        let date = Utc::now().format("%Y-%m-%d");

        writeln!(output, "## [{}] - {}\n", version, date).unwrap();

        // Breaking changes first
        if let Some(breaking) = self.collect_breaking_changes(commits) {
            writeln!(output, "### BREAKING CHANGES\n").unwrap();
            for desc in breaking {
                writeln!(output, "- {}", desc).unwrap();
            }
            writeln!(output).unwrap();
        }

        // Other sections
        for (section, section_commits) in &sections {
            if section_commits.is_empty() {
                continue;
            }

            writeln!(output, "### {}\n", section).unwrap();

            for commit in section_commits {
                let scope = commit.scope.as_ref()
                    .map(|s| format!("**{}:** ", s))
                    .unwrap_or_default();

                let refs = if !commit.references.is_empty() {
                    format!(" ({})", commit.references.join(", "))
                } else {
                    String::new()
                };

                let hash = &commit.hash[..7];
                writeln!(output, "- {}{}{} ({})", scope, commit.description, refs, hash).unwrap();
            }

            writeln!(output).unwrap();
        }

        output
    }
}
```

### 1.8 Release Workflow

- [ ] Orchestrate the full release flow
- [ ] Validate pre-conditions
- [ ] Handle errors gracefully
- [ ] Support dry-run mode
- [ ] Provide clear output

**Workflow orchestration:**
```rust
// crates/canaveral-core/src/workflow.rs
use anyhow::Result;

pub struct ReleaseWorkflow {
    git: GitRepo,
    config: Config,
    dry_run: bool,
}

impl ReleaseWorkflow {
    pub async fn execute(&self) -> Result<ReleaseResult> {
        // 1. Validate pre-conditions
        self.validate()?;

        // 2. Get commits since last release
        let commits = self.git.commits_since_tag(&self.config.git.tag_prefix)?;
        let parsed = self.parse_commits(&commits)?;

        // 3. Calculate new version
        let current = self.get_current_version()?;
        let release_type = determine_release_type(&parsed);
        let new_version = self.strategy.bump(&current, release_type);

        if self.dry_run {
            return Ok(self.dry_run_result(&current, &new_version, &parsed));
        }

        // 4. Generate changelog
        let changelog_entry = self.changelog.generate(&parsed, &new_version.to_string());

        // 5. Update manifest(s)
        self.update_version(&new_version)?;

        // 6. Update CHANGELOG.md
        self.prepend_changelog(&changelog_entry)?;

        // 7. Commit changes
        let message = self.format_commit_message(&new_version);
        self.git.commit(&message, &["CHANGELOG.md", self.manifest_path()])?;

        // 8. Create tag
        let tag_name = format!("{}{}", self.config.git.tag_prefix, new_version);
        self.git.create_tag(&tag_name, &format!("Release {}", new_version))?;

        // 9. Push (if configured)
        if self.config.git.push {
            self.git.push("origin", &["HEAD", &tag_name])?;
        }

        Ok(ReleaseResult {
            previous_version: current,
            new_version,
            commits: parsed,
            tag: tag_name,
        })
    }

    fn validate(&self) -> Result<()> {
        // Check git is clean
        if !self.git.is_clean()? {
            anyhow::bail!("Working directory has uncommitted changes");
        }

        // Check on correct branch
        let branch = self.git.current_branch()?;
        if branch != self.config.git.branch {
            anyhow::bail!(
                "Not on release branch. Expected '{}', got '{}'",
                self.config.git.branch,
                branch
            );
        }

        Ok(())
    }
}
```

## Testing Strategy

### Unit Tests
- Commit parser with various formats
- SemVer parsing and incrementing
- Configuration loading and validation
- Changelog formatting

### Integration Tests
- Git operations in temp repos
- Full release workflow (dry-run)
- Configuration file discovery

### Test organization
```
crates/canaveral-changelog/src/parser/
├── conventional.rs
└── tests.rs           # Unit tests

tests/
├── integration/
│   ├── git_test.rs
│   ├── release_test.rs
│   └── config_test.rs
└── fixtures/
    ├── commits/       # Sample commit messages
    ├── configs/       # Sample configurations
    └── repos/         # Git repo templates
```

## Definition of Done

Phase 1 is complete when:

1. [ ] `canaveral init` creates a config file
2. [ ] `canaveral version` calculates and displays next version
3. [ ] `canaveral changelog` generates changelog content
4. [ ] `canaveral release --no-publish` performs full release (no registry)
5. [ ] Dry-run mode previews all changes
6. [ ] Conventional commits are parsed correctly
7. [ ] SemVer versions are calculated correctly
8. [ ] Configuration file is loaded and validated
9. [ ] Unit test coverage > 80%
10. [ ] Cross-compiles for macOS, Linux, Windows
11. [ ] Binary size < 10MB
12. [ ] Documentation is complete for Phase 1 features
