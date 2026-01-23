# Version Strategies

Canaveral supports multiple versioning schemes through a pluggable strategy system.

## Built-in Strategies

### SemVer (Semantic Versioning)

The default strategy. Format: `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`

**Examples:**
- `1.0.0` - Stable release
- `1.2.3` - Stable release
- `2.0.0-alpha.1` - Pre-release
- `1.0.0+20260115` - With build metadata

**Bump Rules:**
| Commit Type | Bump |
|-------------|------|
| `BREAKING CHANGE` | major |
| `feat` | minor |
| `fix`, `perf` | patch |
| `docs`, `style`, `refactor`, `test`, `chore` | none (or patch if forced) |

**Configuration:**
```yaml
strategy: semver
semver:
  # Allow 0.x.y versions (breaking changes bump minor)
  allowZeroMajor: true

  # Pre-release identifier format
  prereleaseFormat: "${identifier}.${number}"  # alpha.0, alpha.1
```

### CalVer (Calendar Versioning)

Date-based versioning. Multiple formats supported.

**Formats:**
| Format | Example | Description |
|--------|---------|-------------|
| `YYYY.MM.DD` | `2026.01.15` | Full date |
| `YYYY.MM.MICRO` | `2026.01.3` | Year.Month.Release |
| `YY.MM.MICRO` | `26.01.3` | Short year |
| `YYYY.0M.MICRO` | `2026.01.3` | Zero-padded month |
| `YYYY.WW.MICRO` | `2026.03.1` | Week number |

**Bump Rules:**
- New day/week/month → reset micro to 0 (or 1)
- Same period → increment micro

**Configuration:**
```yaml
strategy: calver
calver:
  format: "YYYY.MM.MICRO"
  microStart: 0      # Start micro at 0 or 1
  timezone: "UTC"    # Timezone for date calculation
```

### Build Number

Monotonically increasing build numbers. Common for mobile apps.

**Formats:**
| Format | Example | Description |
|--------|---------|-------------|
| `BUILD` | `456` | Pure build number |
| `SEMVER.BUILD` | `1.2.3.456` | SemVer + build |
| `SEMVER+BUILD` | `1.2.3+456` | SemVer with build metadata |
| `MAJOR.MINOR.BUILD` | `1.2.456` | Version + build |

**Counter Sources:**
| Source | Description |
|--------|-------------|
| `git` | Count commits on branch |
| `file` | Persist counter in `.buildnum` file |
| `ci` | Use CI environment variable |

**Configuration:**
```yaml
strategy: buildnum
buildnum:
  format: "SEMVER.BUILD"
  counter: git  # or: file, ci

  # For CI counter, specify env var
  ciVariable: GITHUB_RUN_NUMBER
```

### Hybrid Strategies

Combine strategies for complex requirements.

**SemVer + CalVer:**
```yaml
strategy: hybrid
hybrid:
  base: semver      # Major.minor from SemVer
  suffix: calver    # Date as patch
  format: "${MAJOR}.${MINOR}.${YYYY}${MM}${DD}"
  # Result: 2.1.20260115
```

**SemVer + Build Number:**
```yaml
strategy: hybrid
hybrid:
  base: semver
  suffix: buildnum
  format: "${SEMVER}+build.${BUILD}"
  # Result: 1.2.3+build.456
```

## Strategy Interface

Custom strategies implement this interface:

```typescript
interface VersionStrategy {
  name: string;

  /**
   * Calculate the next version based on commits
   */
  calculate(
    currentVersion: string,
    commits: Commit[],
    options: StrategyOptions
  ): Promise<string>;

  /**
   * Parse a version string into components
   */
  parse(version: string): VersionComponents;

  /**
   * Compare two versions (-1, 0, 1)
   */
  compare(a: string, b: string): number;

  /**
   * Validate a version string
   */
  validate(version: string): boolean;

  /**
   * Format components into a version string
   */
  format(components: VersionComponents): string;
}

interface VersionComponents {
  // Common
  raw: string;

  // SemVer
  major?: number;
  minor?: number;
  patch?: number;
  prerelease?: string[];
  build?: string[];

  // CalVer
  year?: number;
  month?: number;
  day?: number;
  week?: number;
  micro?: number;

  // Build number
  buildNumber?: number;
}
```

## Version Sources

Where the current version is read from:

| Ecosystem | Source |
|-----------|--------|
| npm | `package.json` → `version` |
| Cargo | `Cargo.toml` → `[package].version` |
| Python | `pyproject.toml` → `[project].version` |
| Go | Latest git tag |
| Maven | `pom.xml` → `<version>` |
| Docker | Build arg or label |

## Pre-release Handling

### Creating Pre-releases

```bash
# From stable to pre-release
canaveral release preminor --preid alpha
# 1.2.3 → 1.3.0-alpha.0

# Increment pre-release
canaveral release prerelease
# 1.3.0-alpha.0 → 1.3.0-alpha.1

# Change pre-release type
canaveral release prerelease --preid beta
# 1.3.0-alpha.1 → 1.3.0-beta.0

# Promote to stable
canaveral release minor
# 1.3.0-beta.0 → 1.3.0
```

### Pre-release Configuration

```yaml
semver:
  prerelease:
    # Allowed identifiers
    allowed: [alpha, beta, rc]

    # Auto-increment format
    format: "${id}.${num}"  # alpha.0, alpha.1

    # npm dist-tag mapping
    tags:
      alpha: next
      beta: next
      rc: latest
```

## Version Validation

Before publishing, versions are validated:

1. **Format** - Matches strategy's expected format
2. **Increment** - Greater than current published version
3. **Uniqueness** - Not already published to registry
4. **Branch rules** - Pre-releases only from non-main branches (configurable)

```yaml
validation:
  # Require pre-releases from feature branches
  prereleaseOnlyFromBranches:
    - feature/*
    - develop

  # Allow same version re-publish (for failed publishes)
  allowRepublish: false
```

## Monorepo Versioning

### Independent Mode

Each package has its own version:

```
packages/
├── core/         # 1.5.0
├── cli/          # 2.1.3
└── utils/        # 1.0.0
```

**Tags:** `@myorg/core@1.5.0`, `@myorg/cli@2.1.3`

### Fixed Mode

All packages share the same version:

```
packages/
├── core/         # 3.0.0
├── cli/          # 3.0.0
└── utils/        # 3.0.0
```

**Tag:** `v3.0.0`

### Configuration

```yaml
monorepo:
  mode: independent  # or: fixed

  # For fixed mode, where to store the version
  versionFile: lerna.json  # or: package.json, VERSION

  # Package-specific overrides
  packages:
    "@myorg/legacy":
      strategy: calver  # Different strategy for this package
```
