# CLI Interface Design

This document defines the CLI interface for Canaveral, following the [Command Line Interface Guidelines](https://clig.dev/).

## Design Principles

### Human-First
- Output is designed for humans by default, with machine-readable options available via `--json`
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

### `canaveral release [type]`

Execute a full release: calculate version, generate changelog, commit, tag, and publish.

```bash
# Auto-detect release type from commits
canaveral release

# Explicit version bump
canaveral release patch
canaveral release minor
canaveral release major

# Pre-release
canaveral release prepatch --preid alpha
canaveral release preminor --preid beta
canaveral release prerelease  # increment existing pre-release
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--dry-run` | `-n` | Preview changes without executing |
| `--no-git` | | Skip git commit, tag, and push |
| `--no-publish` | | Skip publishing to registries |
| `--no-changelog` | | Skip changelog generation |
| `--preid <id>` | | Pre-release identifier (alpha, beta, rc) |
| `--tag <tag>` | | Registry tag (e.g., latest, next) |
| `--filter <pattern>` | | Filter packages by name or glob (repeatable) |
| `--exclude <pattern>` | | Exclude packages by name or glob (repeatable) |
| `--changed` | | Only release packages with changes |
| `--force` | `-f` | Continue on non-critical errors |
| `--yes` | `-y` | Skip confirmation prompts |

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

### `canaveral version [type]`

Calculate and update version without publishing.

```bash
# Auto-detect from commits
canaveral version

# Explicit bump
canaveral version minor

# Set specific version
canaveral version 2.0.0
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--dry-run` | `-n` | Show version without updating files |
| `--no-git` | | Update files but don't commit |
| `--preid <id>` | | Pre-release identifier |
| `--filter <pattern>` | | Filter packages |

### `canaveral changelog`

Generate changelog from commits.

```bash
# Generate for unreleased changes
canaveral changelog

# Generate for specific version range
canaveral changelog --from v1.2.0 --to v1.3.0

# Output to stdout (don't write file)
canaveral changelog --stdout
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--version <ver>` | | Version header for changelog |
| `--from <ref>` | | Starting git ref |
| `--to <ref>` | | Ending git ref (default: HEAD) |
| `--stdout` | | Output to stdout instead of file |
| `--format <fmt>` | | Output format: `markdown`, `json` |

### `canaveral publish`

Publish current version to registries.

```bash
# Publish all packages
canaveral publish

# Publish specific packages
canaveral publish --filter @myorg/core

# Publish to specific registry
canaveral publish --registry https://npm.mycompany.com
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--dry-run` | `-n` | Preview publish without executing |
| `--tag <tag>` | | Registry tag |
| `--registry <url>` | | Override registry URL |
| `--filter <pattern>` | | Filter packages |
| `--access <level>` | | npm access level: `public`, `restricted` |

### `canaveral init`

Initialize configuration in current project.

```bash
# Interactive setup (default in TTY)
canaveral init

# Auto-detect and generate config non-interactively
canaveral init --auto

# Specify config format
canaveral init --format yaml
canaveral init --format toml
```

**Options:**

| Flag | Short | Description |
|------|-------|-------------|
| `--auto` | | Auto-detect settings, skip prompts |
| `--format <fmt>` | | Config format: `yaml`, `toml` |
| `--force` | `-f` | Overwrite existing config |

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
$ canaveral status --json
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
| `--config <path>` | `-c` | Path to config file |
| `--verbose` | `-v` | Show detailed output (repeatable: -vv, -vvv) |
| `--quiet` | `-q` | Suppress non-essential output |
| `--json` | | Output in JSON format |
| `--no-color` | | Disable colored output |
| `--no-input` | | Disable all interactive prompts |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show Canaveral version |

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
- `--no-color` flag is passed

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

When stdin is a TTY and `--no-input` is not set:
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

# Disable all interactivity
canaveral release --no-input

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

1. **Command-line flags** (`--tag-prefix release-`)
2. **Environment variables** (`CANAVERAL_TAG_PREFIX=release-`)
3. **Project config** (`.canaveral.toml` in project root)
4. **User config** (`~/.config/canaveral/config.toml`)
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
| `CANAVERAL_CONFIG` | Path to config file |
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
    -n, --dry-run           Preview changes without executing
        --no-git            Skip git operations
        --no-publish        Skip publishing to registries
        --preid <ID>        Pre-release identifier [default: alpha]
        --filter <PATTERN>  Filter packages (can be repeated)
    -f, --force             Continue on non-critical errors
    -y, --yes               Skip confirmation prompts

GLOBAL OPTIONS:
    -c, --config <PATH>     Path to config file
    -v, --verbose           Increase verbosity (-v, -vv, -vvv)
    -q, --quiet             Suppress non-essential output
        --json              Output JSON format
        --no-color          Disable colors
        --no-input          Disable interactive prompts
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
