# Configuration

Canaveral uses a `canaveral.yaml` (or `.toml`) configuration file in the project root. When no configuration exists, auto-detection provides sensible defaults.

## Configuration File

### Full Example

```yaml
# canaveral.yaml
version: 1

# Global settings
strategy: semver
changelog:
  format: conventional-commits
  file: CHANGELOG.md

# Package manager configurations
packages:
  - type: npm
    path: .
    registry: https://registry.npmjs.org

  - type: cargo
    path: ./rust-package
    registry: crates.io

  - type: python
    path: ./python-package
    registry: https://upload.pypi.org/legacy/

# Monorepo settings
monorepo:
  mode: independent  # or "fixed"
  packages:
    - packages/*
    - apps/*

# Git configuration
git:
  tagPrefix: v
  commitMessageFormat: "chore(release): publish {{version}}"
  push: true
  signTags: false

# Hooks
hooks:
  pre-version:
    - npm test
    - npm run lint
  post-publish:
    - ./scripts/notify-slack.sh

# Plugins
plugins:
  - name: canaveral-plugin-slack
    config:
      webhook: ${SLACK_WEBHOOK_URL}
```

## Configuration Options

### Global Settings

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `version` | number | `1` | Config schema version |
| `strategy` | string | `semver` | Version strategy (`semver`, `calver`, `buildnum`) |
| `changelog.format` | string | `conventional-commits` | Commit parsing format |
| `changelog.file` | string | `CHANGELOG.md` | Changelog output file |

### Package Configuration

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `type` | string | Yes | Package manager (`npm`, `cargo`, `python`, etc.) |
| `path` | string | Yes | Path to package directory |
| `registry` | string | No | Custom registry URL |
| `private` | boolean | No | Skip publishing if true |

### Monorepo Settings

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `mode` | string | `independent` | `independent` or `fixed` versioning |
| `packages` | string[] | `[]` | Glob patterns for package locations |
| `ignoreChanges` | string[] | `[]` | Patterns to ignore in change detection |

### Git Configuration

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `tagPrefix` | string | `v` | Prefix for version tags |
| `commitMessageFormat` | string | `chore(release): {{version}}` | Commit message template |
| `push` | boolean | `true` | Push commits and tags |
| `signTags` | boolean | `false` | GPG sign tags |
| `signCommits` | boolean | `false` | GPG sign commits |
| `branch` | string | `main` | Target branch for releases |

### Hooks

Hooks can be shell commands or scripts:

```yaml
hooks:
  pre-version:
    - npm test
    - npm run build
  post-version:
    - echo "Version bumped to {{version}}"
  pre-publish:
    - ./scripts/validate.sh
  post-publish:
    - ./scripts/notify.sh
```

Available hook stages:
- `pre-version` - Before version bump
- `post-version` - After version bump
- `pre-changelog` - Before changelog generation
- `post-changelog` - After changelog generation
- `pre-commit` - Before git commit
- `post-commit` - After git commit
- `pre-publish` - Before registry publish
- `post-publish` - After registry publish

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
| `build.gradle` | gradle |
| `*.csproj` | nuget |
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

## Environment Variables

Configuration values can reference environment variables:

```yaml
packages:
  - type: npm
    registry: ${NPM_REGISTRY:-https://registry.npmjs.org}

hooks:
  post-publish:
    - curl -X POST ${WEBHOOK_URL}
```

## CLI Overrides

All configuration can be overridden via CLI flags:

```bash
# Override config file
canaveral release --config ./custom-config.yaml

# Override specific options
canaveral release --no-git --no-publish
canaveral release --tag-prefix "release-"
canaveral release --dry-run
```

## JSON Schema

Configuration is validated against a JSON Schema. IDEs with YAML/JSON Schema support will provide autocomplete and validation.

Schema location: `https://canaveral.dev/schema/v1.json`

```yaml
# yaml-language-server: $schema=https://canaveral.dev/schema/v1.json
version: 1
strategy: semver
```

## Internal Representation

Configuration is parsed into strongly-typed Rust structs:

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub version: u32,
    #[serde(default)]
    pub strategy: Strategy,
    #[serde(default)]
    pub changelog: ChangelogConfig,
    #[serde(default)]
    pub packages: Vec<PackageConfig>,
    #[serde(default)]
    pub monorepo: Option<MonorepoConfig>,
    #[serde(default)]
    pub git: GitConfig,
    #[serde(default)]
    pub hooks: HooksConfig,
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    #[default]
    Semver,
    Calver,
    Buildnum,
    Custom(String),
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageConfig {
    #[serde(rename = "type")]
    pub package_type: String,
    pub path: PathBuf,
    pub registry: Option<String>,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct GitConfig {
    #[serde(default = "default_tag_prefix")]
    pub tag_prefix: String,
    #[serde(default)]
    pub commit_message_format: String,
    #[serde(default = "default_true")]
    pub push: bool,
    #[serde(default)]
    pub sign_tags: bool,
    #[serde(default)]
    pub sign_commits: bool,
    #[serde(default = "default_branch")]
    pub branch: String,
}

fn default_tag_prefix() -> String { "v".to_string() }
fn default_branch() -> String { "main".to_string() }
fn default_true() -> bool { true }
```
