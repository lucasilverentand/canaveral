# Configuration

Canaveral uses a `canaveral.yaml` (or `canaveral.toml`) configuration file in the project root. When no configuration exists, auto-detection provides sensible defaults.

## Configuration File

### Full Example

```yaml
# canaveral.yaml

# Optional JSON Schema reference
$schema: "https://canaveral.dev/schema/v1.json"

# Project name
name: my-project

# Versioning configuration
versioning:
  strategy: semver          # semver, calver, buildnum
  tag_format: "v{version}"
  independent: false        # Set true for independent monorepo versioning
  prerelease_identifier: null
  build_metadata: null

# Git configuration
git:
  remote: origin
  branch: main
  require_clean: true
  push_tags: true
  push_commits: true
  commit_message: "chore(release): {version}"
  sign_commits: false
  sign_tags: false

# Changelog configuration
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

# Package configurations
packages:
  - name: my-web-app
    path: ./web
    type: npm
    publish: true
    registry: https://registry.npmjs.org

  - name: my-rust-lib
    path: ./core
    type: cargo
    publish: true

  - name: my-python-lib
    path: ./scripts
    type: python
    publish: true
    registry: https://upload.pypi.org/legacy/

# Hooks (shell commands)
hooks:
  pre_version:
    - cargo test
    - cargo clippy
  post_version:
    - echo "Version bumped"
  pre_changelog: []
  post_changelog: []
  pre_publish:
    - ./scripts/validate.sh
  post_publish:
    - ./scripts/notify-slack.sh
  pre_git: []
  post_git: []

# Publishing configuration
publish:
  enabled: true
  dry_run: false
  registries:
    npm:
      url: https://registry.npmjs.org
      token_env: NPM_TOKEN
    crates-io:
      url: https://crates.io
      token_env: CARGO_REGISTRY_TOKEN

# Code signing configuration
signing:
  enabled: false
  provider: macos     # macos, windows, android, gpg
  identity: null
  artifacts: []
  verify_after_sign: true
  macos:
    hardened_runtime: true
    timestamp: true
    deep: true
    notarize: false
  windows:
    algorithm: sha256
  android:
    v1_signing: true
    v2_signing: true
    v3_signing: true
    v4_signing: false
  gpg:
    detached: true
    armor: true

# App store configurations
stores:
  apple:
    api_key_id: "KEY_ID"
    api_issuer_id: "ISSUER_ID"
    api_key: "/path/to/key.p8"
  google_play:
    package_name: "com.example.app"
    service_account_key: "/path/to/service-account.json"

# Metadata management
metadata:
  enabled: false
  storage:
    format: fastlane    # fastlane or unified
    path: ./metadata
  defaults:
    default_locale: en-US
  validation:
    strict: false
    required_locales: []

# Task orchestration
tasks:
  concurrency: 4
  pipeline:
    build:
      command: "cargo build"
      depends_on: []
      outputs: ["target/**"]
      inputs: ["src/**", "Cargo.toml"]
    test:
      command: "cargo test"
      depends_on: ["build"]
      depends_on_packages: true
    lint:
      command: "cargo clippy"
      depends_on: []
  cache:
    enabled: true
    dir: .canaveral/cache

# CI/CD configuration
ci:
  platform: github    # github, gitlab
  mode: native        # native, traditional
  on_pr:
    - test
    - lint
  on_push_main:
    - test
    - release
  on_tag:
    - publish

# PR validation
pr:
  branching_model: trunk-based   # trunk-based, gitflow, custom
  checks:
    - tests
    - lint
    - commit-format
    - version-conflict
  require_changelog: false
  require_conventional_commits: true

# Release notes generation
release_notes:
  categorize: true
  include_contributors: true
  include_migration_guide: true
  auto_update_store_metadata: false
  locales:
    - en-US
```

## Configuration Options

### Top-Level Fields

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `$schema` | string | `null` | JSON Schema reference for IDE support |
| `name` | string | `null` | Project name |

### Versioning Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `versioning.strategy` | string | `semver` | Version strategy (`semver`, `calver`, `buildnum`) |
| `versioning.tag_format` | string | `v{version}` | Tag format template |
| `versioning.independent` | bool | `false` | Use independent versioning in monorepos |
| `versioning.prerelease_identifier` | string | `null` | Pre-release identifier |
| `versioning.build_metadata` | string | `null` | Build metadata |

### Package Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `name` | string | Yes | Package name |
| `path` | string | Yes | Path to package directory (relative to repo root) |
| `type` | string | Yes | Package type (`npm`, `cargo`, `python`, `go`, `maven`, `docker`) |
| `publish` | bool | `true` | Whether to publish this package |
| `registry` | string | No | Custom registry URL |
| `tag_format` | string | No | Package-specific tag format override |
| `version_files` | string[] | `[]` | Additional files to update with version |

### Git Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `git.remote` | string | `origin` | Remote name |
| `git.branch` | string | `main` | Branch to release from |
| `git.require_clean` | bool | `true` | Require clean working directory |
| `git.push_tags` | bool | `true` | Push tags after release |
| `git.push_commits` | bool | `true` | Push commits after release |
| `git.commit_message` | string | `chore(release): {version}` | Commit message template |
| `git.sign_commits` | bool | `false` | GPG sign release commits |
| `git.sign_tags` | bool | `false` | GPG sign release tags |

### Changelog Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `changelog.enabled` | bool | `true` | Whether to generate changelog |
| `changelog.file` | string | `CHANGELOG.md` | Changelog output file |
| `changelog.format` | string | `markdown` | Changelog format |
| `changelog.include_hashes` | bool | `true` | Include commit hashes in changelog |
| `changelog.include_authors` | bool | `false` | Include commit authors |
| `changelog.include_dates` | bool | `true` | Include dates |
| `changelog.types` | map | (see below) | Commit type to section mapping |

Default commit types:

| Type | Section | Hidden |
|------|---------|--------|
| `feat` | Features | false |
| `fix` | Bug Fixes | false |
| `docs` | Documentation | false |
| `perf` | Performance | false |
| `refactor` | Refactoring | true |
| `test` | Tests | true |
| `chore` | Chores | true |

### Hooks

Hooks are shell commands run at lifecycle stages. In the config file (`HooksConfig`), 8 hook points are available:

```yaml
hooks:
  pre_version:
    - cargo test
  post_version: []
  pre_changelog: []
  post_changelog: []
  pre_publish:
    - ./scripts/validate.sh
  post_publish:
    - ./scripts/notify.sh
  pre_git: []
  post_git: []
```

The hook runtime engine (`HookStage` enum) supports 12 lifecycle stages:
- `pre-release` / `post-release` - Before/after the entire release process
- `pre-version` / `post-version` - Before/after version bump
- `pre-changelog` / `post-changelog` - Before/after changelog generation
- `pre-commit` / `post-commit` - Before/after git commit
- `pre-tag` / `post-tag` - Before/after git tag
- `pre-publish` / `post-publish` - Before/after registry publish

Hook context environment variables:
- `CANAVERAL_VERSION` - Current version being released
- `CANAVERAL_PREVIOUS_VERSION` - Previous version
- `CANAVERAL_PACKAGE` - Package name
- `CANAVERAL_RELEASE_TYPE` - Release type (major, minor, patch)
- `CANAVERAL_TAG` - Git tag name
- `CANAVERAL_DRY_RUN` - Whether this is a dry run

## Auto-Detection

When no configuration file exists, Canaveral automatically detects:

### Package Manager Detection

| File | Detected Type |
|------|---------------|
| `package.json` | npm |
| `Cargo.toml` | cargo |
| `pyproject.toml` | python |
| `setup.py` | python |
| `go.mod` | go |
| `pom.xml` | maven |
| `Dockerfile` | docker |

### Monorepo Detection

| Pattern | Detection |
|---------|-----------|
| `packages/*/package.json` | npm workspaces |
| `Cargo.toml` with `[workspace]` | Cargo workspace |
| `pnpm-workspace.yaml` | pnpm workspaces |
| `lerna.json` | Lerna monorepo |

### Commit Convention Detection

Analyzes recent commits to determine format:
- Conventional Commits: `feat:`, `fix:`, etc.
- Angular style: `type(scope): message`
- No convention: Falls back to manual version

## CLI Overrides

Configuration can be overridden via CLI flags:

```bash
# Override specific options
canaveral release --no-git --no-publish
canaveral release --dry-run
canaveral release --allow-branch feature/my-branch

# Specify working directory
canaveral -C ./my-project release
```

Note: There is no `--config` flag. Canaveral searches for `canaveral.yaml`, `canaveral.yml`, or `canaveral.toml` in the working directory.

## Internal Representation

Configuration is parsed into strongly-typed Rust structs (from `canaveral-core/src/config/types.rs`):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "$schema")]
    pub schema: Option<String>,
    pub name: Option<String>,
    pub versioning: VersioningConfig,
    pub git: GitConfig,
    pub changelog: ChangelogConfig,
    pub packages: Vec<PackageConfig>,
    pub hooks: HooksConfig,
    pub publish: PublishConfig,
    pub signing: SigningConfig,
    pub stores: StoresConfig,
    pub metadata: MetadataConfig,
    pub tasks: TasksConfig,
    pub ci: CIConfig,
    pub pr: PrConfig,
    pub release_notes: ReleaseNotesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct VersioningConfig {
    pub strategy: String,          // default: "semver"
    pub tag_format: String,        // default: "v{version}"
    pub independent: bool,         // default: false
    pub prerelease_identifier: Option<String>,
    pub build_metadata: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GitConfig {
    pub remote: String,            // default: "origin"
    pub branch: String,            // default: "main"
    pub require_clean: bool,       // default: true
    pub push_tags: bool,           // default: true
    pub push_commits: bool,        // default: true
    pub commit_message: String,    // default: "chore(release): {version}"
    pub sign_commits: bool,        // default: false
    pub sign_tags: bool,           // default: false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageConfig {
    pub name: String,
    pub path: PathBuf,
    #[serde(rename = "type")]
    pub package_type: String,
    pub publish: bool,             // default: true
    pub registry: Option<String>,
    pub tag_format: Option<String>,
    pub version_files: Vec<PathBuf>,
}
```
