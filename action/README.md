# Canaveral GitHub Action

Universal release management for any package ecosystem. Automate versioning, changelogs, and publishing with a single action.

## Features

- **Multi-ecosystem support**: npm, Cargo, Python, Go, Maven, Docker
- **Automatic versioning**: Based on conventional commits
- **Changelog generation**: From commit history
- **Monorepo support**: Release individual or all packages
- **Pre-release support**: Alpha, beta, RC versions
- **Cross-platform**: Linux, macOS, Windows runners

## Quick Start

```yaml
- uses: canaveral/action@v1
  with:
    registry-token: ${{ secrets.NPM_TOKEN }}
```

That's it! Canaveral will:
1. Analyze commits since the last release
2. Calculate the next version
3. Update version files
4. Generate changelog
5. Create git tag
6. Publish to registry

## Usage Examples

### Basic Release

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

### Manual Version Bump

```yaml
- uses: canaveral/action@v1
  with:
    bump: minor  # major, minor, patch
    registry-token: ${{ secrets.NPM_TOKEN }}
```

### Dry Run (Preview)

```yaml
- uses: canaveral/action@v1
  id: preview
  with:
    dry-run: true

- run: echo "Would release version ${{ steps.preview.outputs.version }}"
```

### Monorepo Package

```yaml
- uses: canaveral/action@v1
  with:
    package: '@myorg/core'
    registry-token: ${{ secrets.NPM_TOKEN }}
```

### Version Only (No Publish)

```yaml
- uses: canaveral/action@v1
  with:
    command: version
    bump: patch
```

### Generate Changelog Only

```yaml
- uses: canaveral/action@v1
  with:
    command: changelog
```

### Skip Steps

```yaml
- uses: canaveral/action@v1
  with:
    skip-publish: true    # Don't publish to registry
    skip-changelog: true  # Don't generate changelog
    skip-git: true        # Don't commit/tag/push
```

### Using Outputs

```yaml
- uses: canaveral/action@v1
  id: release
  with:
    registry-token: ${{ secrets.NPM_TOKEN }}

- name: Create GitHub Release
  if: steps.release.outputs.released == 'true'
  uses: softprops/action-gh-release@v1
  with:
    tag_name: ${{ steps.release.outputs.tag }}
    body: ${{ steps.release.outputs.changelog }}

- name: Notify on Slack
  if: steps.release.outputs.released == 'true'
  run: |
    curl -X POST $SLACK_WEBHOOK -d '{
      "text": "Released ${{ steps.release.outputs.version }}"
    }'
```

## Inputs

| Input | Description | Default |
|-------|-------------|---------|
| `command` | Command to run: `release`, `version`, `changelog`, `init` | `release` |
| `bump` | Version bump type: `major`, `minor`, `patch`, `auto` (from commits) | `auto` |
| `version` | Explicit version string (overrides bump) | |
| `package` | Package name for monorepo releases | |
| `git-user-name` | Git user name for commits | `github-actions[bot]` |
| `git-user-email` | Git user email for commits | `github-actions[bot]@users.noreply.github.com` |
| `dry-run` | Simulate without making changes | `false` |
| `skip-publish` | Skip registry publishing | `false` |
| `skip-changelog` | Skip changelog generation | `false` |
| `skip-git` | Skip git operations | `false` |
| `registry-token` | Token for package registry | |
| `working-directory` | Working directory | `.` |
| `canaveral-version` | Canaveral version to use | `latest` |

## Outputs

| Output | Description |
|--------|-------------|
| `version` | The released/calculated version |
| `previous-version` | Version before release |
| `changelog` | Generated changelog content |
| `tag` | Git tag created |
| `released` | Whether a release was created (`true`/`false`) |
| `skipped` | Whether release was skipped (no changes) |

## Complete Workflow Examples

### npm Package

```yaml
name: Release

on:
  push:
    branches: [main]

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
      - run: npm ci
      - run: npm test

  release:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions/setup-node@v4
        with:
          node-version: '20'
          registry-url: 'https://registry.npmjs.org'

      - uses: canaveral/action@v1
        id: release
        with:
          registry-token: ${{ secrets.NPM_TOKEN }}

      - name: Create GitHub Release
        if: steps.release.outputs.released == 'true'
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ steps.release.outputs.tag }}
          body: ${{ steps.release.outputs.changelog }}
```

### Rust Crate

```yaml
name: Release

on:
  push:
    branches: [main]

permissions:
  contents: write

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo test
      - run: cargo clippy -- -D warnings

  release:
    needs: test
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: dtolnay/rust-toolchain@stable

      - uses: canaveral/action@v1
        id: release
        with:
          registry-token: ${{ secrets.CRATES_IO_TOKEN }}

      - name: Create GitHub Release
        if: steps.release.outputs.released == 'true'
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ steps.release.outputs.tag }}
```

### Python Package

```yaml
name: Release

on:
  push:
    branches: [main]

permissions:
  contents: write
  id-token: write

jobs:
  release:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - uses: canaveral/action@v1
        id: release
        with:
          registry-token: ${{ secrets.PYPI_TOKEN }}
```

### Tag-Triggered Release

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

      - uses: canaveral/action@v1
        with:
          skip-git: true  # Tag already exists
          registry-token: ${{ secrets.NPM_TOKEN }}
```

### Release PR Workflow

```yaml
name: Release

on:
  workflow_dispatch:
    inputs:
      bump:
        description: 'Version bump'
        type: choice
        options: [patch, minor, major]

jobs:
  release-pr:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: canaveral/action@v1
        id: version
        with:
          command: version
          bump: ${{ inputs.bump }}
          dry-run: true

      - name: Create Release PR
        uses: peter-evans/create-pull-request@v5
        with:
          title: 'chore(release): v${{ steps.version.outputs.version }}'
          branch: release/v${{ steps.version.outputs.version }}
```

### Monorepo with Matrix

```yaml
name: Release

on:
  push:
    branches: [main]

jobs:
  detect:
    runs-on: ubuntu-latest
    outputs:
      packages: ${{ steps.changes.outputs.packages }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: canaveral/action@v1
        id: changes
        with:
          command: version
          dry-run: true

  release:
    needs: detect
    if: needs.detect.outputs.packages != '[]'
    runs-on: ubuntu-latest
    strategy:
      matrix:
        package: ${{ fromJson(needs.detect.outputs.packages) }}
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - uses: canaveral/action@v1
        with:
          package: ${{ matrix.package }}
          registry-token: ${{ secrets.NPM_TOKEN }}
```

## Comparison: Before & After

### Before (Manual Steps)

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

      - name: Install release tool
        run: npm install -g semantic-release

      - name: Get current version
        id: current
        run: echo "version=$(node -p "require('./package.json').version")" >> $GITHUB_OUTPUT

      - name: Calculate next version
        id: next
        run: |
          # Complex script to parse commits and calculate version
          # ...

      - name: Update package.json
        run: npm version ${{ steps.next.outputs.version }} --no-git-tag-version

      - name: Generate changelog
        run: |
          # Another script to generate changelog
          # ...

      - name: Commit and tag
        run: |
          git add -A
          git commit -m "chore(release): ${{ steps.next.outputs.version }}"
          git tag "v${{ steps.next.outputs.version }}"
          git push --follow-tags

      - name: Publish
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: v${{ steps.next.outputs.version }}
          body_path: CHANGELOG.md
```

### After (With Canaveral Action)

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

## Troubleshooting

### "Permission denied" when pushing

Ensure your workflow has write permissions:

```yaml
permissions:
  contents: write
```

### "No version bump detected"

Your commits may not follow conventional commit format. Use:
- `feat: ...` for features (minor bump)
- `fix: ...` for bug fixes (patch bump)
- `feat!: ...` or `BREAKING CHANGE:` for breaking changes (major bump)

### Releases not triggering

Ensure you're checking out with full history:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0  # Required for commit analysis
```

### Using with protected branches

If your main branch is protected, you'll need a PAT:

```yaml
- uses: actions/checkout@v4
  with:
    fetch-depth: 0
    token: ${{ secrets.PAT_TOKEN }}
```

## License

MIT
