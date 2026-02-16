# Canaveral

The unified launch system for software. One tool to build, test, release, and ship — across every platform.

Canaveral replaces the patchwork of turborepo, tuist, release-please, semantic-release, fastlane, and a dozen other tools with a single CLI that understands your entire project lifecycle: from a code change landing in a PR, through CI, into main, out as a versioned release, and delivered to users — whether that's npm, crates.io, the App Store, or a GitHub Release with binaries.

## The problem

Modern software projects need an absurd number of tools bolted together:

- **Build orchestration** — turborepo, nx, tuist, bazel
- **Test selection** — roll your own, or run everything every time
- **Release automation** — release-please, semantic-release, release-plz, changesets
- **Publishing** — npm publish, cargo publish, fastlane deliver, twine upload
- **Marketing & metadata** — fastlane frameit, app store screenshots, changelogs for humans

Each tool has its own config format, its own mental model, its own CI integration, and its own failure modes. Polyglot monorepos are the worst — you might need turborepo for the JS packages, cargo for the Rust crate, fastlane for the iOS app, and release-please to tie it all together. None of them talk to each other.

## What canaveral does

Canaveral is a single binary (Rust, no runtime needed) that handles the full lifecycle:

```
code change → build → test (only what changed) → merge → version → changelog → tag → publish → distribute
```

### 1. Understand your project

Auto-detects your workspace structure — Cargo workspaces, npm/pnpm/yarn workspaces, Xcode projects, mixed monorepos. Builds a dependency graph so it knows what depends on what.

### 2. Build and test efficiently

Knows which packages changed and only runs the builds and tests that matter. Understands the dependency graph so if `core` changed, it tests `core` and everything that depends on it — but not unrelated packages.

### 3. Manage the path to main

Validates that your branch is ready: tests pass, version conflicts resolved, changelog entries present. Works with your branching model, not against it.

### 4. Mint releases

Calculates the next version (SemVer, CalVer, build numbers — your choice), generates a changelog from conventional commits, tags the repo, creates a GitHub/GitLab release.

### 5. Publish everywhere

Publishes to the right registries in dependency order: npm, crates.io, PyPI, Maven, Docker Hub. Handles credentials, retries, and rollback if something fails partway through.

### 6. Distribute to users

Uploads to app stores (App Store Connect, Google Play, Microsoft Store), manages TestFlight and Firebase App Distribution, handles code signing and certificates.

### 7. Generate marketing material

Auto-generates release notes for humans (not just commit logs), captures and frames app store screenshots, manages store metadata and descriptions.

## Quick start

```bash
# Install
curl -fsSL https://canaveral.dev/install.sh | sh

# Initialize in your project
canaveral init

# See what would happen
canaveral release --dry-run

# Do it for real
canaveral release
```

## Configuration

Canaveral works with zero config for simple projects (auto-detection) or a `canaveral.yaml` for full control:

```yaml
versioning:
  strategy: semver

packages:
  - path: ./core
    type: cargo
  - path: ./web
    type: npm
  - path: ./ios
    type: xcode

publish:
  - registry: crates-io
    package: core
  - registry: npm
    package: web
  - store: app-store-connect
    package: ios

ci:
  test_selection: changed  # only test affected packages
  parallel: true
```

## CLI

```bash
canaveral init          # set up a new project
canaveral status        # what's changed, what needs releasing
canaveral build         # build affected packages
canaveral test          # test only what changed
canaveral release       # version + changelog + tag + publish
canaveral publish       # publish to registries/stores
canaveral doctor        # check your environment is set up right
canaveral validate      # lint your config and repo state
```

## Replaces

| Tool | What canaveral covers |
|------|----------------------|
| turborepo / nx | workspace understanding, task orchestration, change detection |
| tuist | Xcode project generation, build orchestration |
| release-please | version bumps, changelog generation, release PRs |
| semantic-release | version calculation, publishing, git tags |
| changesets | version management, changelog entries |
| fastlane | app store publishing, screenshots, code signing |
| goreleaser | binary building, GitHub releases, checksums |
| cargo-release | Cargo publishing, version bumps |

## Project structure

Rust workspace with focused crates:

```
crates/
  canaveral/             # CLI binary
  canaveral-core/        # config, hooks, orchestration, monorepo support
  canaveral-git/         # git operations, commit parsing, tags
  canaveral-changelog/   # changelog generation and formatting
  canaveral-strategies/  # version calculation (semver, calver, buildnum)
  canaveral-adapters/    # package manager integrations (npm, cargo, python, etc.)
  canaveral-signing/     # code signing (macOS, Windows, Android, GPG)
  canaveral-stores/      # app store and registry uploaders
  canaveral-metadata/    # app store metadata management
  canaveral-frameworks/  # build framework adapters (vite, next.js, flutter, etc.)
```

## Current status

v0.1.0 — foundation is built and compiling. Core versioning, changelog generation, git integration, monorepo support, and package adapters are implemented with 205+ tests. The build orchestration, smart test selection, and CI workflow layers are next.

## License

MIT OR Apache-2.0
