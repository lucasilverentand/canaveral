# GitHub Integration Guide

This guide explains how Canaveral works within a GitHub repository and project, covering everything from initial setup to fully automated releases.

## Using the GitHub Action (Recommended)

The simplest way to use Canaveral is with the official GitHub Action:

```yaml
name: Release

on:
  push:
    branches: [main]

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: canaveral/action@v1
        with:
          registry-token: ${{ secrets.NPM_TOKEN }}
```

That's it! The action handles:
- Installing Canaveral
- Analyzing commits to determine version bump
- Updating version files
- Generating changelog
- Creating git tags
- Publishing to your registry

See the [Action README](../action/README.md) for complete documentation.

## Overview

Canaveral integrates with GitHub at multiple levels:

1. **Local Development** - CLI commands for version management
2. **GitHub Actions** - Automated CI/CD workflows
3. **GitHub Releases** - Automated release creation with artifacts
4. **Branch Protection** - Release branch strategies

```
┌─────────────────────────────────────────────────────────────────────┐
│                        GitHub Repository                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  ┌──────────────┐    ┌──────────────┐    ┌──────────────────────┐  │
│  │   Feature    │───▶│     PR       │───▶│       main           │  │
│  │   Branch     │    │   Checks     │    │      Branch          │  │
│  └──────────────┘    └──────────────┘    └──────────┬───────────┘  │
│                                                      │              │
│                                                      ▼              │
│                                          ┌───────────────────────┐  │
│                                          │  Canaveral Release    │  │
│                                          │  Workflow             │  │
│                                          └───────────┬───────────┘  │
│                                                      │              │
│         ┌────────────────────────────────────────────┼──────────┐  │
│         │                    │                       │          │  │
│         ▼                    ▼                       ▼          │  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐  │  │
│  │   GitHub    │    │  Package    │    │   Version Tag       │  │  │
│  │   Release   │    │  Registry   │    │   & Changelog       │  │  │
│  └─────────────┘    └─────────────┘    └─────────────────────┘  │  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Quick Start

### 1. Initialize Canaveral in Your Repository

```bash
# Navigate to your repository
cd my-project

# Initialize Canaveral (auto-detects package type)
canaveral init

# Or specify the package type explicitly
canaveral init --type npm
canaveral init --type cargo
canaveral init --type python
```

This creates a `canaveral.yaml` configuration file:

```yaml
version: 1
strategy: semver

package:
  type: npm              # Auto-detected
  path: .

git:
  tagPrefix: "v"
  branch: main
  push: true

changelog:
  format: conventional-commits
  file: CHANGELOG.md
```

### 2. Generate GitHub Actions Workflow

```bash
# Generate a release workflow
canaveral init --ci github

# With custom options
canaveral init --ci github --branch main --include-tests
```

This creates `.github/workflows/release.yml`.

### 3. Set Up Repository Secrets

Add these secrets in **Settings → Secrets and variables → Actions**:

| Package Type | Required Secret | Description |
|--------------|-----------------|-------------|
| npm | `NPM_TOKEN` | npm access token for publishing |
| Cargo | `CRATES_IO_TOKEN` | crates.io API token |
| Python | `PYPI_TOKEN` | PyPI API token |
| Maven | `MAVEN_USERNAME`, `MAVEN_PASSWORD` | Maven Central credentials |

## Workflow Strategies

### Strategy 1: Tag-Triggered Releases (Recommended)

Releases are triggered when you push a version tag. This gives you full control over when releases happen.

**Workflow:**
```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Setup environment
        # ... setup steps for your ecosystem

      - name: Run Canaveral release
        run: canaveral release --no-git  # Tag already exists
        env:
          REGISTRY_TOKEN: ${{ secrets.REGISTRY_TOKEN }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true
```

**Usage:**
```bash
# Local: Create and push a release
canaveral release minor
git push --follow-tags
```

### Strategy 2: Automated Releases on Main

Every push to `main` automatically calculates the next version based on conventional commits.

**Workflow:**
```yaml
name: Release

on:
  push:
    branches:
      - main

permissions:
  contents: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
          token: ${{ secrets.GITHUB_TOKEN }}

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Check for releasable changes
        id: check
        run: |
          if canaveral version --print --dry-run | grep -q "No version bump"; then
            echo "skip=true" >> $GITHUB_OUTPUT
          else
            echo "skip=false" >> $GITHUB_OUTPUT
          fi

      - name: Release
        if: steps.check.outputs.skip == 'false'
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          canaveral release
        env:
          REGISTRY_TOKEN: ${{ secrets.REGISTRY_TOKEN }}
```

### Strategy 3: Release PR Workflow

Create a release PR that can be reviewed before merging triggers the release.

**Workflow:**
```yaml
name: Release PR

on:
  workflow_dispatch:
    inputs:
      version_type:
        description: 'Version bump type'
        required: true
        type: choice
        options:
          - patch
          - minor
          - major

jobs:
  create-release-pr:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Calculate version
        id: version
        run: |
          VERSION=$(canaveral version --${{ inputs.version_type }} --print)
          echo "version=$VERSION" >> $GITHUB_OUTPUT

      - name: Create release branch
        run: |
          git checkout -b release/v${{ steps.version.outputs.version }}
          canaveral version --set ${{ steps.version.outputs.version }}
          canaveral changelog
          git add -A
          git commit -m "chore(release): prepare v${{ steps.version.outputs.version }}"
          git push -u origin release/v${{ steps.version.outputs.version }}

      - name: Create Pull Request
        uses: peter-evans/create-pull-request@v5
        with:
          title: "chore(release): v${{ steps.version.outputs.version }}"
          body: |
            ## Release v${{ steps.version.outputs.version }}

            This PR was automatically created by Canaveral.

            **Changes:**
            - Version bump: ${{ inputs.version_type }}
            - Updated CHANGELOG.md

            Merge this PR to trigger the release.
          branch: release/v${{ steps.version.outputs.version }}
```

## Complete Example Workflows

### npm Package

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
      - run: npm ci
      - run: npm test
      - run: npm run lint

  release:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'

      - name: Install Canaveral
        run: npm install -g canaveral

      - name: Publish to npm
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true
```

### Rust Crate

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  test:
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - run: cargo test --all-features
      - run: cargo clippy -- -D warnings

  release:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable

      - name: Publish to crates.io
        run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true

  # Build binaries for multiple platforms
  build-binaries:
    needs: test
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
          - target: x86_64-apple-darwin
            os: macos-latest
          - target: aarch64-apple-darwin
            os: macos-latest
          - target: x86_64-pc-windows-msvc
            os: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - run: cargo build --release --target ${{ matrix.target }}
      - uses: actions/upload-artifact@v4
        with:
          name: binary-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/myapp*
```

### Python Package

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write
  id-token: write  # Required for PyPI trusted publishing

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'
          cache: 'pip'
      - run: pip install -e .[dev]
      - run: pytest
      - run: ruff check .

  release:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Build package
        run: |
          pip install build
          python -m build

      - name: Publish to PyPI
        uses: pypa/gh-action-pypi-publish@release/v1
        with:
          password: ${{ secrets.PYPI_TOKEN }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true
```

### Monorepo (Multiple Packages)

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'         # Fixed versioning: single tag
      - '@*'         # Independent versioning: @scope/pkg@version

permissions:
  contents: write

jobs:
  detect-packages:
    runs-on: ubuntu-latest
    outputs:
      packages: ${{ steps.detect.outputs.packages }}
    steps:
      - uses: actions/checkout@v4
      - name: Install Canaveral
        run: cargo install canaveral
      - name: Detect changed packages
        id: detect
        run: |
          PACKAGES=$(canaveral list --changed --json)
          echo "packages=$PACKAGES" >> $GITHUB_OUTPUT

  release:
    needs: detect-packages
    runs-on: ubuntu-latest
    strategy:
      matrix:
        package: ${{ fromJson(needs.detect-packages.outputs.packages) }}
    steps:
      - uses: actions/checkout@v4

      - name: Release package
        run: canaveral release --package ${{ matrix.package }}
        env:
          REGISTRY_TOKEN: ${{ secrets.REGISTRY_TOKEN }}
```

## GitHub-Specific Features

### Conventional Commits & Release Notes

Canaveral analyzes your commits to generate:
- Automatic version bumps based on commit types
- Structured changelogs
- GitHub Release notes

```
feat: add user authentication      → minor version bump
fix: resolve login redirect issue  → patch version bump
feat!: redesign API endpoints      → major version bump
BREAKING CHANGE: remove v1 API     → major version bump
```

### Branch Protection Integration

Configure branch protection rules to work with Canaveral:

**Settings → Branches → Branch protection rules:**

1. **Require pull request reviews**
   - Ensures all changes are reviewed before release

2. **Require status checks to pass**
   - Add your CI workflow as required

3. **Require branches to be up to date**
   - Prevents releases with stale code

### Pre-release Branches

Configure Canaveral to create pre-releases from specific branches:

```yaml
# canaveral.yaml
git:
  branch: main
  prereleases:
    - branch: develop
      preid: alpha
    - branch: beta
      preid: beta
    - branch: "rc/*"
      preid: rc
```

This enables:
- Pushes to `develop` → `1.2.3-alpha.1`
- Pushes to `beta` → `1.2.3-beta.1`
- Pushes to `rc/1.2` → `1.2.3-rc.1`

## Migrating from Other Tools

### From semantic-release

```bash
# Detect and migrate configuration
canaveral migrate --from semantic-release

# Or specify the config file
canaveral migrate --from semantic-release --config .releaserc.json
```

This converts:
- `.releaserc`, `.releaserc.json`, `.releaserc.yaml`
- `release.config.js`
- Plugin configurations

### From release-please

```bash
# Detect and migrate configuration
canaveral migrate --from release-please
```

This converts:
- `release-please-config.json`
- `.release-please-manifest.json`
- Monorepo configurations

## Configuration Reference

### Full canaveral.yaml Example

```yaml
version: 1

# Version strategy: semver, calver, or buildnum
strategy: semver

# Package configuration
package:
  type: npm
  path: .
  registry: https://registry.npmjs.org

# Monorepo settings (if applicable)
monorepo:
  mode: independent  # or "fixed"
  packages:
    - packages/*
    - apps/*
  ignoreChanges:
    - "**/*.md"
    - "**/*.test.ts"

# Git settings
git:
  tagPrefix: "v"
  branch: main
  push: true
  signTags: false
  signCommits: false
  commitMessageFormat: "chore(release): ${version}"

# Changelog settings
changelog:
  format: conventional-commits
  file: CHANGELOG.md

# Hooks for custom automation
hooks:
  pre-version:
    - npm test
    - npm run build
  post-version:
    - echo "Version updated to ${VERSION}"
  pre-publish:
    - ./scripts/validate-release.sh
  post-publish:
    - ./scripts/notify-slack.sh
```

### Environment Variables

Use environment variables in your configuration:

```yaml
package:
  registry: ${NPM_REGISTRY:-https://registry.npmjs.org}

hooks:
  post-publish:
    - curl -X POST ${SLACK_WEBHOOK_URL} -d '{"text": "Released ${VERSION}"}'
```

## Simplified Workflows with the Action

Using the Canaveral GitHub Action dramatically simplifies your workflow files.

### Before: Manual Setup (~50 lines)

```yaml
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'

      - run: npm ci

      - name: Configure git
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Calculate version
        id: version
        run: echo "version=$(canaveral version --print)" >> $GITHUB_OUTPUT

      - name: Generate changelog
        run: canaveral changelog

      - name: Bump version
        run: canaveral version --set ${{ steps.version.outputs.version }}

      - name: Commit and push
        run: |
          git add -A
          git commit -m "chore(release): ${{ steps.version.outputs.version }}"
          git tag "v${{ steps.version.outputs.version }}"
          git push --follow-tags

      - name: Publish
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### After: With Canaveral Action (~15 lines)

```yaml
jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: canaveral/action@v1
        id: release
        with:
          registry-token: ${{ secrets.NPM_TOKEN }}

      - uses: softprops/action-gh-release@v1
        if: steps.release.outputs.released == 'true'
        with:
          tag_name: ${{ steps.release.outputs.tag }}
          body: ${{ steps.release.outputs.changelog }}
```

### Action Examples by Ecosystem

**npm:**
```yaml
- uses: canaveral/action@v1
  with:
    registry-token: ${{ secrets.NPM_TOKEN }}
```

**Cargo:**
```yaml
- uses: canaveral/action@v1
  with:
    registry-token: ${{ secrets.CRATES_IO_TOKEN }}
```

**Python:**
```yaml
- uses: canaveral/action@v1
  with:
    registry-token: ${{ secrets.PYPI_TOKEN }}
```

**Monorepo package:**
```yaml
- uses: canaveral/action@v1
  with:
    package: '@myorg/core'
    registry-token: ${{ secrets.NPM_TOKEN }}
```

**Pre-release:**
```yaml
- uses: canaveral/action@v1
  with:
    bump: minor
    preid: beta
    registry-token: ${{ secrets.NPM_TOKEN }}
```

**Dry run preview:**
```yaml
- uses: canaveral/action@v1
  id: preview
  with:
    dry-run: true

- run: echo "Would release ${{ steps.preview.outputs.version }}"
```

## Troubleshooting

### Common Issues

**1. "Permission denied" when pushing tags**

Ensure your workflow has write permissions:
```yaml
permissions:
  contents: write
```

**2. "No version bump detected"**

Your commits may not follow conventional commit format:
```bash
# Check what version would be calculated
canaveral version --dry-run --print
```

**3. "Failed to publish: already exists"**

The version may already be published. Check:
```bash
# Verify current version
canaveral version --current

# Check registry
npm view my-package versions
```

**4. Workflow not triggering on tags**

Ensure your tag format matches the trigger:
```yaml
on:
  push:
    tags:
      - 'v*'  # Matches v1.0.0, v2.1.3, etc.
```

### Debug Mode

Run Canaveral with verbose output:
```bash
RUST_LOG=debug canaveral release --dry-run
```

### Dry Run

Always test before actual releases:
```bash
# See what would happen without making changes
canaveral release --dry-run
```

## Best Practices

1. **Use conventional commits** - Enables automatic version detection
2. **Test with dry-run first** - Validate before real releases
3. **Set up branch protection** - Require reviews and CI checks
4. **Use tag-triggered releases** - More control over release timing
5. **Configure pre-releases** - Test before production releases
6. **Automate changelogs** - Keep users informed of changes
7. **Use GitHub Releases** - Distribute binaries and release notes
