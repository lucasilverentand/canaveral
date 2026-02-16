# Canaveral

The unified launch system for software. One tool to build, test, release, and ship — across every platform.

Canaveral replaces the patchwork of turborepo, tuist, release-please, semantic-release, fastlane, goreleaser, and a dozen other tools with a single Rust CLI that understands your entire project lifecycle: from code landing in a PR, through CI, into main, out as a versioned release, and delivered to users — whether that's npm, crates.io, the App Store, or a GitHub Release with binaries.

Named after Cape Canaveral — we launch software.

## The problem

Modern software projects need an absurd number of tools bolted together:

- **Build orchestration** — turborepo, nx, tuist, bazel
- **Test selection** — roll your own, or run everything every time
- **Release automation** — release-please, semantic-release, release-plz, changesets
- **Publishing** — npm publish, cargo publish, fastlane deliver, twine upload
- **Marketing & metadata** — fastlane frameit, app store screenshots, changelogs for humans

Each tool has its own config format, its own mental model, its own CI integration, and its own failure modes. Polyglot monorepos are the worst — you might need turborepo for the JS packages, cargo for the Rust crate, fastlane for the iOS app, and release-please to tie it all together. None of them talk to each other.

## What canaveral does

```
code change → build → test (only what changed) → merge → version → changelog → tag → publish → distribute
```

### 1. Understand your project

Auto-detects your workspace structure — Cargo workspaces, npm/pnpm/yarn workspaces, mixed monorepos. Builds a dependency graph so it knows what depends on what.

### 2. Build and test efficiently

Knows which packages changed and only runs the builds and tests that matter. Uses the dependency graph so if `core` changed, it tests `core` and everything that depends on it — but not unrelated packages. Content-addressable caching avoids re-running work that hasn't changed.

### 3. Smart test selection

Given a set of changed files, computes the minimal set of tests to run. Parses import graphs for Rust, JavaScript/TypeScript, and Python to trace which tests cover which source files. Don't run the iOS tests if only a Rust crate changed.

### 4. Manage the path to main

Validates that your branch is ready: tests pass, conventional commit format, version conflicts resolved, changelog entries present. Works with trunk-based development, gitflow, or your own branching model.

### 5. Mint releases

Calculates the next version (SemVer, CalVer, build numbers — your choice), generates a changelog from conventional commits, tags the repo, creates a GitHub/GitLab release.

### 6. Publish everywhere

Publishes to the right registries in dependency order: npm, crates.io, PyPI, Maven Central, Docker Hub. Handles credentials, retries, and rollback if something fails partway through.

### 7. Distribute to users

Uploads to app stores (App Store Connect, Google Play, Microsoft Store), manages TestFlight and Firebase App Distribution, handles code signing and certificates.

### 8. Generate marketing material

Auto-generates human-readable release notes (not just commit logs), captures and frames app store screenshots, manages store metadata and descriptions across locales.

---

## Quick start

```bash
# Install
cargo install canaveral

# Initialize in your project
canaveral init

# See what canaveral detects
canaveral status

# Check your environment
canaveral doctor

# Preview a release (no changes)
canaveral release --dry-run

# Create a release
canaveral release
```

## Configuration

Canaveral works with zero config for simple projects (auto-detection) or a `canaveral.yaml` / `canaveral.toml` for full control:

```yaml
name: my-project

versioning:
  strategy: semver          # semver | calver | buildnum
  tag_format: "v{version}"
  independent: false        # true for independent monorepo versioning

git:
  remote: origin
  branch: main
  require_clean: true
  push_tags: true
  commit_message: "chore(release): {version}"

changelog:
  enabled: true
  file: CHANGELOG.md
  include_hashes: true
  include_authors: false
  types:
    feat: { section: "Features", hidden: false }
    fix: { section: "Bug Fixes", hidden: false }
    docs: { section: "Documentation", hidden: false }
    perf: { section: "Performance", hidden: false }

packages:
  - name: core
    path: ./core
    type: cargo
    publish: true
  - name: web
    path: ./web
    type: npm
  - name: ios-app
    path: ./ios
    type: xcode
    publish: false

hooks:
  pre_version:
    - "cargo fmt --check"
  post_publish:
    - "notify-slack.sh"

tasks:
  concurrency: 4
  pipeline:
    build:
      depends_on_packages: true
      outputs: ["dist/**", "target/**"]
      inputs: ["src/**"]
    test:
      depends_on: ["build"]
      depends_on_packages: true
    lint:
      command: "cargo clippy"

publish:
  enabled: true
  dry_run: false
  registries:
    my-registry:
      url: "https://npm.pkg.github.com"
      token_env: GITHUB_TOKEN
```

See [docs/architecture/configuration.md](docs/architecture/configuration.md) for the full configuration reference.

## CLI reference

### Core workflow

```bash
canaveral init                   # Create a canaveral.yaml config
canaveral status                 # Show repo status, current version, pending changes
canaveral validate               # Validate config and repo state
canaveral doctor                 # Check environment for required tools
```

### Versioning and releases

```bash
canaveral version                # Calculate and display next version
canaveral changelog              # Generate changelog from commits
canaveral release                # Full release: version + changelog + tag + publish
canaveral release --dry-run      # Preview release without making changes
canaveral release -t minor       # Force a minor release
canaveral release --version 2.0.0  # Set an explicit version
canaveral release --no-publish   # Release without publishing
canaveral release --no-changelog # Release without changelog generation
canaveral release --no-git       # Release without git operations
canaveral release -p my-package  # Release a specific monorepo package
canaveral release -y             # Skip confirmation prompt
```

### Build and test

```bash
canaveral run build test         # Run tasks across workspace
canaveral run build --affected   # Only build packages affected by changes
canaveral run test --filter core # Only run in specific packages
canaveral run build --dry-run    # Show execution plan without running
canaveral run build --no-cache   # Bypass task cache
canaveral run build --concurrency 8  # Override parallel task limit

canaveral test                   # Run tests with framework detection
canaveral test --smart           # Smart test selection (only changed code)
canaveral test --affected        # Only test affected packages
canaveral test --coverage        # Collect code coverage
canaveral test --reporter junit --output results.xml  # JUnit output
canaveral test --fail-fast       # Stop on first failure
canaveral test --retry 2         # Retry failed tests

canaveral build                  # Build for detected platform
canaveral cache                  # Task cache management
```

### Publishing

```bash
canaveral publish                # Publish to registries/stores
```

### Code signing

```bash
canaveral signing                # Sign artifacts
canaveral match                  # Sync certificates and profiles (fastlane match style)
```

### App stores and distribution

```bash
canaveral metadata               # Manage app store metadata
canaveral screenshots            # Capture and frame screenshots
canaveral test-flight            # Manage TestFlight beta testing
canaveral firebase               # Manage Firebase App Distribution
```

### CI/CD

```bash
canaveral ci                     # CI pipeline management
canaveral pr                     # PR validation and preview
```

### Utilities

```bash
canaveral completions bash       # Generate shell completions (bash, zsh, fish, etc.)
```

### Global flags

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output |
| `--quiet` | `-q` | Suppress output except errors |
| `--format <text\|json>` | | Output format (default: text) |
| `--directory <path>` | `-C` | Working directory |

## Version strategies

### SemVer (default)

Standard Semantic Versioning 2.0.0. Automatically determines bump type from conventional commits:

- `feat:` → minor bump
- `fix:`, `perf:`, `docs:` → patch bump
- `BREAKING CHANGE:` or `feat!:` → major bump
- Pre-release support: `1.0.0-alpha.1` → `1.0.0-alpha.2`

### CalVer

Calendar-based versioning with multiple formats:

| Format | Example | Description |
|--------|---------|-------------|
| `YearMonth` | `2025.2.0` | YYYY.MM.MICRO |
| `YearMonthPadded` | `2025.02.0` | YYYY.0M.MICRO |
| `ShortYearMonth` | `25.2.0` | YY.MM.MICRO |
| `YearMonthDay` | `2025.2.16` | YYYY.MM.DD |
| `YearWeek` | `2025.7.0` | YYYY.WW.MICRO |
| `YearMicro` | `2025.1` | YYYY.MICRO |

Micro version resets when the calendar period changes.

### Build numbers

Sequential build numbering with optional date prefixes:

| Format | Example | Description |
|--------|---------|-------------|
| `Simple` | `42` | Sequential integer |
| `WithBase` | `1.0.42` | BASE.BUILD |
| `DateBuild` | `20250216.1` | YYYYMMDD.BUILD |
| `FullDate` | `1.0.20250216.1` | MAJOR.MINOR.YYYYMMDD.BUILD |

## Package adapters

Canaveral detects and manages packages for these ecosystems:

| Ecosystem | Manifest | Registry | Features |
|-----------|----------|----------|----------|
| **npm** | `package.json` | npm, GitHub Packages | Scoped packages, access control, OTP, tags |
| **Cargo** | `Cargo.toml` | crates.io | Registry support, `cargo check` validation |
| **Python** | `pyproject.toml`, `setup.py` | PyPI | Build, publish via twine |
| **Go** | `go.mod` | Go proxy | Module versioning |
| **Maven** | `pom.xml`, `build.gradle` | Maven Central | Gradle/Maven support |
| **Docker** | `Dockerfile` | Docker Hub, any registry | Multi-platform builds |

Each adapter implements the `PackageAdapter` trait: detect, get/set version, build, test, publish, validate.

## Monorepo support

Canaveral is monorepo-first. It detects workspace structures automatically:

- **Cargo** workspaces (`[workspace]` in `Cargo.toml`)
- **npm/pnpm/yarn** workspaces (`workspaces` field in `package.json`)
- **Lerna, Nx, Turborepo** configurations

Features:
- **Dependency graph** — builds a full dependency graph with topological sorting
- **Change detection** — git-diff-based detection of which packages changed
- **Affected packages** — computes transitive closure: if `core` changed, everything depending on `core` is also affected
- **Coordinated publishing** — publishes in dependency order; if one fails, rolls back
- **Independent versioning** — optionally version each package independently

## Task orchestration

The `canaveral run` command is a task runner that understands your workspace:

```yaml
tasks:
  concurrency: 4
  pipeline:
    build:
      depends_on_packages: true   # run build in dependency packages first
      outputs: ["dist/**"]
      inputs: ["src/**"]
    test:
      depends_on: ["build"]       # run build before test in same package
      depends_on_packages: true   # run test in dependency packages first
    lint:
      command: "eslint src/"
  cache:
    enabled: true
    dir: ".canaveral/cache"
```

- **Parallel execution** — runs independent tasks concurrently, respecting the dependency DAG
- **Content-addressable caching** — hashes inputs (source files + command + env) to skip unchanged work
- **Wave-based scheduling** — groups tasks into waves of parallelizable work
- **Smart test selection** — parses import graphs (Rust `use`, JS `import`/`require`, Python `import`) to run only the tests that cover changed code

## Code signing

Platform-native code signing with a unified interface:

| Platform | Provider | Features |
|----------|----------|----------|
| **macOS** | `codesign`, `productsign` | Hardened runtime, entitlements, deep signing, notarization, stapling |
| **Windows** | `signtool` (Authenticode) | PFX certificates, timestamp servers, SHA-256/384/512 |
| **Android** | `apksigner`, `jarsigner` | Keystore, V1/V2/V3/V4 signing schemes |
| **GPG** | `gpg` | Detached signatures, ASCII armor, passphrase via env |

**Match-style certificate sync** — share signing credentials across a team via a Git repo, S3, GCS, or Azure Blob Storage. Inspired by fastlane match.

## App store distribution

| Store | Features |
|-------|----------|
| **App Store Connect** | Upload builds, notarize, TestFlight distribution, API key auth |
| **Google Play** | Upload APK/AAB, track management (internal/alpha/beta/production), service account auth |
| **Microsoft Store** | Upload MSIX, Azure AD auth |
| **Firebase App Distribution** | Beta distribution, group management |
| **npm / crates.io** | Registry publish with token auth and tag support |

## Metadata management

Manage app store metadata in your repo, validated before upload:

- **Storage formats** — Fastlane-compatible directory layout or Canaveral's unified format
- **Apple App Store** — name, description, keywords, screenshots per device, age ratings, categories
- **Google Play** — title, descriptions, screenshots per device, content ratings, categories
- **Validation** — character limits, required fields, screenshot dimensions, locale checks
- **Template variables** — inject version, date, etc. into metadata fields

Supported screenshot devices: iPhone 5.5"/6.1"/6.5"/6.7", iPad Pro 12.9" (3rd/6th gen), Apple TV, Apple Watch, Google Play phone/7" tablet/10" tablet/TV.

## Framework adapters

Canaveral detects and integrates with these frameworks for building, testing, and distribution:

| Framework | Build | Test | Screenshots | Distribute |
|-----------|-------|------|-------------|------------|
| Flutter | yes | yes | yes | yes |
| React Native | yes | yes | yes | yes |
| Expo | yes | yes | yes | yes |
| Native iOS | yes | yes | yes | yes |
| Native Android | yes | yes | - | yes |
| Vite | yes | yes | - | - |
| Next.js | yes | yes | - | - |
| Astro | yes | yes | - | - |
| Tauri | yes | yes | - | yes |

## Hook system

12 lifecycle hooks let you run custom commands at each stage of the release process:

| Hook | When |
|------|------|
| `pre_version` | Before version bump |
| `post_version` | After version bump |
| `pre_changelog` | Before changelog generation |
| `post_changelog` | After changelog generation |
| `pre_git` | Before git commit/tag |
| `post_git` | After git commit/tag |
| `pre_publish` | Before publishing |
| `post_publish` | After publishing |

Hooks receive environment variables: `CANAVERAL_VERSION`, `CANAVERAL_PREVIOUS_VERSION`, `CANAVERAL_PACKAGE`, `CANAVERAL_RELEASE_TYPE`, `CANAVERAL_TAG`, `CANAVERAL_DRY_RUN`, plus any custom variables you define.

## Plugin system

Extend canaveral with external plugins for custom package adapters, version strategies, or changelog formatters.

Plugins communicate via subprocess JSON protocol:

```json
// Request
{"action": "get_version", "input": {"path": "/my/package"}, "config": {}}
// Response
{"output": {"version": "1.2.3"}}
```

Plugin types:
- **Adapter** — custom package manager support
- **Strategy** — custom version calculation
- **Formatter** — custom changelog formatting

Plugins are auto-discovered from search paths or configured explicitly.

## CI integration

Canaveral generates and manages CI configs so your CI pipeline is thin — just "run canaveral":

```yaml
# GitHub Actions
ci:
  platform: github
  on_pr: [test, lint]
  on_push_main: [test, release]
  on_tag: [publish]
```

Pre-built templates for:
- GitHub Actions (release, iOS build, Android build, multiplatform, screenshots)
- GitLab CI
- CircleCI
- Bitrise
- Azure Pipelines

A GitHub Action is also available for direct use in workflows.

## What canaveral replaces

| Tool | What canaveral covers |
|------|----------------------|
| turborepo / nx | Workspace understanding, task orchestration, caching, change detection |
| tuist | Xcode project understanding, build orchestration |
| release-please | Version bumps, changelog generation, release PRs |
| semantic-release | Version calculation, publishing, git tags |
| changesets | Version management, changelog entries |
| fastlane | App store publishing, screenshots, code signing, metadata |
| goreleaser | Binary building, GitHub releases, checksums |
| cargo-release | Cargo publishing, version bumps |

## Project structure

Rust workspace with focused crates:

```
crates/
  canaveral/             # CLI binary (clap)
  canaveral-core/        # Config loading, hooks, plugins, monorepo detection, orchestration
  canaveral-git/         # Git operations via libgit2 (commit parsing, tags, remote ops)
  canaveral-changelog/   # Conventional commit parsing, changelog generation, release notes
  canaveral-strategies/  # Version calculation (SemVer, CalVer, build numbers)
  canaveral-adapters/    # Package manager integrations (npm, Cargo, Python, Go, Maven, Docker)
  canaveral-signing/     # Code signing (macOS, Windows, Android, GPG)
  canaveral-stores/      # App store and registry uploaders
  canaveral-metadata/    # App store metadata management and validation
  canaveral-frameworks/  # Build/test framework adapters (Flutter, React Native, Vite, etc.)
  canaveral-tasks/       # Task DAG, parallel scheduler, caching, smart test selection
```

## Building from source

```bash
# Clone
git clone https://github.com/example/canaveral.git
cd canaveral

# Debug build
cargo build

# Release build (LTO enabled, binary stripped)
cargo build --release

# Run tests
cargo test

# Test a specific crate
cargo test -p canaveral-core
cargo test -p canaveral-tasks
```

Requires Rust 1.75+ (2021 edition).

## Documentation

Detailed documentation lives in the [`docs/`](docs/) directory:

- **Architecture**
  - [Overview](docs/architecture/overview.md) — system design and crate responsibilities
  - [Configuration reference](docs/architecture/configuration.md) — every config key explained
  - [Plugin system](docs/architecture/plugins.md) — writing and registering plugins
- **Design**
  - [CLI design](docs/design/cli.md) — command structure and UX decisions
  - [Adapters](docs/design/adapters.md) — the `PackageAdapter` trait and ecosystem integrations
  - [Versioning](docs/design/versioning.md) — version strategy design and the `VersionStrategy` trait
  - [Changelog](docs/design/changelog.md) — commit parsing and changelog generation
- **Guides**
  - [Mobile quick start](docs/guides/mobile/quick-start.md) — iOS/Android release setup
  - [Framework guide](docs/guides/mobile/frameworks.md) — framework-specific setup
  - [Migrating from fastlane](docs/guides/mobile/migration-from-fastlane.md)
- **Reference**
  - [Comparison with other tools](docs/comparison.md)
  - [Roadmap](docs/roadmap.md)
  - [GitHub integration](docs/github-integration.md)

The project [website](website/) contains additional documentation covering commands, frameworks, signing, distribution, CI/CD, and migration guides.

## Current status

**v0.1.0** — the foundation is built and compiling. Core versioning, changelog generation, git integration, monorepo support, package adapters, task orchestration with caching, smart test selection, and framework adapters are implemented with 205+ tests. The project is functional for release workflows and is actively being expanded.

## License

MIT OR Apache-2.0
