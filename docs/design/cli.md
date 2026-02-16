# CLI Interface Design

This document defines the CLI interface for Canaveral, following the [Command Line Interface Guidelines](https://clig.dev/).

## Design Principles

### Human-First
- Output is designed for humans by default, with machine-readable options available via `--format json`
- Detect TTY to adapt behavior (interactive prompts, colors, progress indicators)
- Keep output concise but informative

### Composability
- Standard I/O conventions: primary output to `stdout`, messages/errors to `stderr`
- Well-defined exit codes for scripting
- Support piping with `-` for stdin/stdout

### Consistency
- Consistent flag names across all subcommands
- Predictable `verb noun` command structure
- Standard flag aliases (`-h`/`--help`, `-v`/`--verbose`, `-q`/`--quiet`)

### Robustness
- Print initial output within 100ms to show the program is working
- Show progress indicators for long operations
- Operations are idempotent where possible
- Graceful handling of Ctrl-C

---

## Command Structure

```
canaveral <command> [options]
```

## Primary Commands

### `canaveral release`

Execute a full release: calculate version, generate changelog, commit, tag, and publish.

```bash
# Auto-detect release type from commits
canaveral release

# Explicit version bump
canaveral release --release-type minor
canaveral release -r patch

# Set explicit version
canaveral release --version 2.0.0

# Dry run
canaveral release --dry-run

# Skip specific steps
canaveral release --no-git --no-publish --no-changelog
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--release-type <type>` | `-r` | Release type (major, minor, patch, prerelease, custom) |
| `--version <ver>` | | Explicit version to release |
| `--dry-run` | | Preview changes without executing |
| `--no-git` | | Skip git operations (commit, tag, push) |
| `--no-publish` | | Skip publishing to registries |
| `--no-changelog` | | Skip changelog generation |
| `--yes` | `-y` | Skip confirmation prompts |
| `--allow-branch` | | Allow release from non-release branch |
| `--package <name>` | `-p` | Package to release (for monorepos) |

**Example output:**

```
$ canaveral release --dry-run

Analyzing commits since v1.2.3...

  Found 5 commits:
    feat: add OAuth2 support
    fix: handle empty responses
    fix: correct timeout handling
    docs: update API reference
    chore: update dependencies

  Calculated version: 1.3.0 (minor bump from feat commit)

Would perform:
  1. Update version in package.json to 1.3.0
  2. Generate changelog entry
  3. Create commit: chore(release): v1.3.0
  4. Create tag: v1.3.0
  5. Push to origin
  6. Publish to npm registry

Run without --dry-run to execute.
```

### `canaveral version`

Calculate the next version based on commits.

```bash
# Auto-detect from commits
canaveral version

# Force a specific release type
canaveral version --release-type minor

# Show current version only
canaveral version --current

# For a specific package
canaveral version --package my-lib
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--release-type <type>` | `-r` | Force a specific release type (major, minor, patch, prerelease, custom) |
| `--current` | | Show current version only |
| `--package <name>` | `-p` | Package name (for monorepos) |

### `canaveral changelog`

Generate changelog from commits.

```bash
# Generate and print to stdout
canaveral changelog

# Generate for a specific version
canaveral changelog --version 1.2.0

# Write to file
canaveral changelog --write

# Write to specific file
canaveral changelog --write --output CHANGES.md

# Include all commit types (don't filter)
canaveral changelog --all
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--version <ver>` | `-v` | Version to generate changelog for |
| `--write` | `-w` | Write to file (default: print to stdout) |
| `--output <path>` | `-o` | Output file (defaults to configured changelog file) |
| `--all` | | Include all commits (don't filter by type) |

### `canaveral publish`

Publish to app stores or package registries. Uses subcommands for each target.

```bash
# Publish to npm
canaveral publish npm ./dist/my-package-1.0.0.tgz --tag latest

# Publish to crates.io
canaveral publish crates ./target/package/my-crate-1.0.0.crate

# Publish to Apple App Store
canaveral publish apple ./build/App.ipa --api-key-id KEY_ID --api-issuer-id ISSUER_ID --api-key key.p8

# Publish to Google Play
canaveral publish google-play ./build/app.aab --package-name com.example.app --service-account sa.json

# Publish to Microsoft Store
canaveral publish microsoft ./build/app.msix --tenant-id TID --client-id CID --client-secret SEC --app-id AID
```

**Subcommands:**

| Subcommand | Description |
|------------|-------------|
| `npm` | Publish to NPM registry |
| `crates` | Publish to Crates.io registry |
| `apple` | Publish to Apple App Store |
| `google-play` | Publish to Google Play Store |
| `microsoft` | Publish to Microsoft Store |

Each subcommand requires an artifact path and has its own authentication options. All subcommands support `--dry-run` and `--verbose`.

### `canaveral init`

Initialize a new Canaveral configuration.

```bash
# Interactive setup (prompts for format)
canaveral init

# Use defaults without prompting
canaveral init --yes

# Overwrite existing config
canaveral init --force

# Write to specific file
canaveral init --output canaveral.toml
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--force` | `-f` | Overwrite existing configuration |
| `--yes` | `-y` | Use defaults without prompting |
| `--output <path>` | `-o` | Output file path |

### `canaveral status`

Show current release status.

```bash
canaveral status
```

**Example output (TTY):**

```
my-package v1.2.3
Last release: 3 days ago (2026-01-20)

Unreleased changes (5 commits):
  2 features
  2 fixes
  1 docs

Next version: 1.3.0 (minor)

Run 'canaveral release --dry-run' to preview the release.
```

**Example output (JSON):**

```bash
$ canaveral status --format json
```
```json
{
  "package": "my-package",
  "currentVersion": "1.2.3",
  "lastRelease": "2026-01-20T10:30:00Z",
  "commits": {
    "total": 5,
    "features": 2,
    "fixes": 2,
    "docs": 1
  },
  "nextVersion": "1.3.0",
  "bumpType": "minor"
}
```

### `canaveral validate`

Run pre-release validation checks.

```bash
canaveral validate
```

**Example output:**

```
Validating release readiness...

  [ok] Git working directory is clean
  [ok] On branch 'main' (matches config)
  [ok] package.json is valid
  [ok] npm credentials configured
  [!!] 2 uncommitted changes in staged files

Validation failed. Fix the issues above before releasing.
```

---

## Global Options

Available on all commands:

| Flag | Short | Description |
|------|-------|-------------|
| `--verbose` | `-v` | Enable verbose output |
| `--quiet` | `-q` | Suppress output except errors |
| `--format <fmt>` | | Output format: `text` (default), `json` |
| `--directory <path>` | `-C` | Working directory |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show Canaveral version |

Note: There is no `--config`, `--json`, `--no-color`, or `--no-input` flag. Use `--format json` for JSON output. Color is controlled by the `NO_COLOR` environment variable or terminal detection.

---

## Output Conventions

### Standard Streams

- **stdout**: Primary output (version numbers, changelogs, JSON data)
- **stderr**: Progress messages, warnings, errors, interactive prompts

This ensures `canaveral version --dry-run` can be captured cleanly:

```bash
NEW_VERSION=$(canaveral version --dry-run)
```

### Color Handling

Colors are automatically disabled when:
- Output is not a TTY (piped or redirected)
- `NO_COLOR` environment variable is set
- `TERM=dumb`

Colors are used sparingly:
- **Red**: Errors only
- **Yellow**: Warnings
- **Green**: Success confirmations
- **Cyan**: Informational highlights

### Progress Indicators

For operations taking more than 1 second:
- Show a spinner or progress bar on stderr
- Include what's currently happening
- For multi-step operations, show step progress

```
Publishing packages... [2/5] @myorg/utils
```

---

## Interactive Mode

### TTY Detection

When stdin is a TTY:
- Prompt for missing required information
- Show confirmation prompts for destructive actions
- Allow interactive selection menus

When stdin is NOT a TTY (scripts, CI):
- Never prompt; require all input via flags
- Fail with clear error if required input is missing
- Auto-detected via `CI` environment variable

### Confirmation Prompts

**Moderate changes** (releasing, publishing):
```
About to release v2.0.0 (major version bump).
This will publish to npm and create a git tag.

Continue? [y/N]
```

**Destructive operations** (if any):
```
This will delete the local tag v1.2.3.
Type 'v1.2.3' to confirm:
```

### Disabling Prompts

```bash
# Skip confirmations
canaveral release --yes

# Auto-detected in CI
CI=true canaveral release
```

---

## Error Handling

### User-Friendly Errors

Errors are written to stderr with:
1. Clear description of what went wrong
2. Likely cause
3. Suggested fix

**Example:**

```
Error: Cannot publish to npm registry

  The npm authentication token is invalid or expired.

  To fix this:
    1. Run 'npm login' to authenticate
    2. Or set NPM_TOKEN in your environment
    3. Or add token to ~/.npmrc

  For CI environments, see: https://canaveral.dev/docs/ci-setup
```

### Grouped Errors

When multiple errors occur, group them:

```
Validation failed with 3 errors:

  package.json:
    - Missing "repository" field
    - Invalid "version" format

  Cargo.toml:
    - Missing "license" field

Run 'canaveral validate' for details on each error.
```

---

## Exit Codes

| Code | Name | Meaning |
|------|------|---------|
| 0 | `SUCCESS` | Operation completed successfully |
| 1 | `ERROR` | General/unknown error |
| 2 | `CONFIG_ERROR` | Configuration file invalid or missing |
| 3 | `VALIDATION_ERROR` | Pre-release validation failed |
| 4 | `GIT_ERROR` | Git operation failed |
| 5 | `PUBLISH_ERROR` | Publish to registry failed |
| 10 | `CANCELLED` | User cancelled operation (Ctrl-C or prompt) |
| 126 | `NOT_EXECUTABLE` | Command found but not executable |
| 127 | `NOT_FOUND` | Command or dependency not found |

For scripting:

```bash
canaveral release
case $? in
  0) echo "Release successful" ;;
  3) echo "Validation failed, fix issues first" ;;
  5) echo "Publish failed, check credentials" ;;
  *) echo "Release failed" ;;
esac
```

---

## Configuration Hierarchy

Settings are resolved in this order (highest priority first):

1. **Command-line flags** (e.g., `--format json`)
2. **Environment variables** (e.g., `CANAVERAL_LOG=debug`)
3. **Project config** (`canaveral.yaml`, `canaveral.yml`, or `canaveral.toml` in project root)
4. **Auto-detection** (package manager and convention detection)
5. **Built-in defaults**

### Config File Locations

Following XDG Base Directory Specification:

| Platform | User Config | Cache |
|----------|-------------|-------|
| Linux | `~/.config/canaveral/` | `~/.cache/canaveral/` |
| macOS | `~/Library/Application Support/canaveral/` | `~/Library/Caches/canaveral/` |
| Windows | `%APPDATA%\canaveral\` | `%LOCALAPPDATA%\canaveral\` |

---

## Environment Variables

### Canaveral-Specific

| Variable | Description |
|----------|-------------|
| `CANAVERAL_VERSION` | Current version (set during hooks) |
| `CANAVERAL_LOG` | Log level: `error`, `warn`, `info`, `debug`, `trace` |

### Respected Standard Variables

| Variable | Description |
|----------|-------------|
| `NO_COLOR` | Disable colored output (any value) |
| `TERM` | Terminal type (`dumb` disables colors/progress) |
| `CI` | Disable interactive prompts (any value) |
| `EDITOR` | Editor for commit message editing |
| `HOME` | User home directory |
| `XDG_CONFIG_HOME` | Config directory override |
| `XDG_CACHE_HOME` | Cache directory override |

### Credential Handling

**Important**: Secrets should NOT be passed via environment variables due to security risks (process listing, logging, shell history).

Instead, Canaveral reads credentials from:

1. **Credential files** (recommended):
   - `~/.npmrc` for npm tokens
   - `~/.cargo/credentials.toml` for crates.io
   - `~/.pypirc` for PyPI

2. **Stdin** (for CI):
   ```bash
   echo "$NPM_TOKEN" | canaveral publish --token-stdin
   ```

3. **Credential helpers** (git-credential-store pattern)

If you must use environment variables in CI (common pattern), Canaveral will read:
- `NPM_TOKEN` for npm
- `CARGO_REGISTRY_TOKEN` for crates.io
- `PYPI_TOKEN` for PyPI

But a warning will be shown recommending more secure alternatives.

---

## Help System

### Help Flags

```bash
canaveral --help        # Show main help
canaveral -h            # Short alias
canaveral help          # Subcommand alias
canaveral release --help  # Command-specific help
canaveral help release    # Alternative syntax
```

### Help Structure

Help text is structured with the most common use cases first:

```
canaveral release - Create a new release

USAGE:
    canaveral release [TYPE] [OPTIONS]

EXAMPLES:
    canaveral release              # Auto-detect from commits
    canaveral release minor        # Explicit minor bump
    canaveral release --dry-run    # Preview without executing

TYPE:
    patch       Increment patch version (1.2.3 -> 1.2.4)
    minor       Increment minor version (1.2.3 -> 1.3.0)
    major       Increment major version (1.2.3 -> 2.0.0)
    prepatch    Create patch pre-release (1.2.3 -> 1.2.4-alpha.0)
    preminor    Create minor pre-release (1.2.3 -> 1.3.0-alpha.0)
    premajor    Create major pre-release (1.2.3 -> 2.0.0-alpha.0)
    prerelease  Increment pre-release (1.3.0-alpha.0 -> 1.3.0-alpha.1)

OPTIONS:
    -r, --release-type <TYPE>  Release type (major, minor, patch, prerelease, custom)
        --version <VER>        Explicit version to release
        --dry-run              Preview changes without executing
        --no-git               Skip git operations
        --no-publish           Skip publishing to registries
        --no-changelog         Skip changelog generation
    -y, --yes                  Skip confirmation prompts
        --allow-branch         Allow release from non-release branch
    -p, --package <NAME>       Package to release (monorepos)

GLOBAL OPTIONS:
    -v, --verbose           Enable verbose output
    -q, --quiet             Suppress non-essential output
        --format <FMT>      Output format: text (default), json
    -C, --directory <PATH>  Working directory
    -h, --help              Show this help
    -V, --version           Show version

LEARN MORE:
    Documentation: https://canaveral.dev/docs/release
    Report issues: https://github.com/your-org/canaveral/issues
```

### Typo Suggestions

When a command is mistyped, suggest corrections:

```
$ canaveral relase

Error: Unknown command 'relase'

Did you mean 'release'?

Run 'canaveral --help' to see available commands.
```

---

## Monorepo Commands

### Package Filtering

```bash
# By exact name
canaveral release --filter @myorg/core

# By glob pattern
canaveral release --filter "packages/*"
canaveral release --filter "@myorg/*"

# Multiple filters (OR logic)
canaveral release --filter packages/core --filter packages/utils

# Exclude packages
canaveral release --exclude packages/internal

# Only changed packages since last release
canaveral release --changed

# Combine filters
canaveral release --filter "@myorg/*" --exclude "@myorg/internal" --changed
```

### Monorepo Status

```bash
$ canaveral status

Workspace: my-monorepo
Packages: 5

  @myorg/core       v2.1.0  (3 unreleased commits)
  @myorg/utils      v1.5.2  (no changes)
  @myorg/cli        v2.0.1  (1 unreleased commit)
  @myorg/server     v2.1.0  (2 unreleased commits)
  @myorg/internal   v0.1.0  (private, excluded)

Run 'canaveral release --changed' to release packages with changes.
```

---

## Example Workflows

### Standard Release

```bash
# Check status
canaveral status

# Preview changes
canaveral release --dry-run

# Execute release
canaveral release
```

### Pre-release Workflow

```bash
# Create alpha
canaveral release preminor --preid alpha
# 1.2.3 -> 1.3.0-alpha.0

# Iterate on alpha
canaveral release prerelease
# 1.3.0-alpha.0 -> 1.3.0-alpha.1

# Promote to beta
canaveral release prerelease --preid beta
# 1.3.0-alpha.1 -> 1.3.0-beta.0

# Final release
canaveral release minor
# 1.3.0-beta.0 -> 1.3.0
```

### CI/CD Integration

```yaml
# GitHub Actions
- name: Release
  run: |
    canaveral release --yes
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
    # Token passed via file for security
- name: Setup npm credentials
  run: echo "//registry.npmjs.org/:_authToken=${{ secrets.NPM_TOKEN }}" > ~/.npmrc
```

### Monorepo Release

```bash
# Release only changed packages
canaveral release --changed

# Release specific package
canaveral release --filter @myorg/core

# Release all packages in dependency order
canaveral release
```

---

## Privacy & Telemetry

Canaveral does **not** collect any usage data, analytics, or telemetry. Your release process stays entirely local and private.

---

## Future Compatibility

### Deprecation Policy

When features are deprecated:

1. Warning message shown for 2 minor versions
2. Migration instructions provided
3. Feature removed in next major version

```
Warning: --skip-changelog is deprecated and will be removed in v3.0.
Use --no-changelog instead.
```

### Stable Interfaces

- Command names and flag names are stable within major versions
- JSON output schema is stable (additive changes only)
- Exit codes are stable
- Human-readable output may change for clarity
