# Changelog Generation

Canaveral automatically generates changelogs from commit history, supporting multiple conventions and output formats.

## Commit Conventions

### Conventional Commits (Default)

Format: `type(scope): description`

**Types:**
| Type | Description | Changelog Section |
|------|-------------|-------------------|
| `feat` | New feature | Features |
| `fix` | Bug fix | Bug Fixes |
| `docs` | Documentation | Documentation |
| `style` | Code style (formatting) | Styles |
| `refactor` | Code refactoring | Code Refactoring |
| `perf` | Performance improvement | Performance |
| `test` | Tests | Tests |
| `build` | Build system | Build |
| `ci` | CI configuration | CI |
| `chore` | Maintenance | Chores |
| `revert` | Revert commit | Reverts |

**Examples:**
```
feat(auth): add OAuth2 support
fix(api): handle null response correctly
docs: update installation guide
feat!: remove deprecated endpoints
```

**Breaking Changes:**
```
feat!: remove deprecated API

# or with footer
feat(api): update response format

BREAKING CHANGE: Response now returns array instead of object
```

### Angular Style

Similar to Conventional Commits with stricter scope requirements.

```
<type>(<scope>): <subject>

<body>

<footer>
```

### Custom Patterns

Define custom patterns via regex:

```yaml
changelog:
  format: custom
  custom:
    pattern: "^\\[(\\w+)\\]\\s+(.+)$"
    groups:
      type: 1
      description: 2
    typeMap:
      ADD: Features
      FIX: Bug Fixes
      CHG: Changes
```

**Matches:** `[ADD] New feature`, `[FIX] Bug fix`

## Changelog Output

### Markdown Format (Default)

```markdown
## [1.2.0] - 2026-01-15

### Features

- **auth:** Add OAuth2 support ([#123](https://github.com/org/repo/pull/123)) ([abc1234](https://github.com/org/repo/commit/abc1234))
- **api:** New endpoint for user preferences ([def5678](https://github.com/org/repo/commit/def5678))

### Bug Fixes

- **api:** Handle null response correctly ([#124](https://github.com/org/repo/pull/124)) ([ghi9012](https://github.com/org/repo/commit/ghi9012))

### BREAKING CHANGES

- **api:** Response format changed from object to array. Update your client code to handle the new format.
```

### Keep a Changelog Format

Follows [keepachangelog.com](https://keepachangelog.com) conventions:

```markdown
## [Unreleased]

## [1.2.0] - 2026-01-15

### Added
- OAuth2 support for authentication

### Fixed
- Null response handling in API

### Changed
- Response format now returns array

### Removed
- Deprecated v1 endpoints
```

### JSON Format

```json
{
  "version": "1.2.0",
  "date": "2026-01-15",
  "sections": {
    "features": [
      {
        "scope": "auth",
        "description": "Add OAuth2 support",
        "hash": "abc1234",
        "references": ["#123"]
      }
    ],
    "fixes": [
      {
        "scope": "api",
        "description": "Handle null response correctly",
        "hash": "ghi9012",
        "references": ["#124"]
      }
    ],
    "breaking": [
      {
        "description": "Response format changed from object to array"
      }
    ]
  }
}
```

## Configuration

The changelog section in `canaveral.yaml` maps to the `ChangelogConfig` struct:

```yaml
changelog:
  # Enable changelog generation
  enabled: true

  # Output file
  file: CHANGELOG.md

  # Output format
  format: markdown

  # Optional header text
  header: null

  # Include commit hashes in changelog entries
  include_hashes: true

  # Include commit authors
  include_authors: false

  # Include dates
  include_dates: true

  # Commit type to section mapping
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
```

Types marked `hidden: true` are not included in the changelog output by default.

## Reference Detection

Canaveral automatically detects references to issues and PRs:

**Patterns recognized:**
- `#123` - GitHub/GitLab issue or PR
- `GH-123` - GitHub issue
- `JIRA-123` - Jira ticket
- `closes #123` - Closing reference
- `fixes #123` - Fixing reference

**Configuration:**
```yaml
changelog:
  references:
    # Custom patterns
    patterns:
      - pattern: "PROJ-\\d+"
        url: "https://jira.company.com/browse/${match}"
        label: "Jira"

    # Auto-link GitHub references
    github: true

    # Auto-link GitLab references
    gitlab: false
```

## Changelog Commands

### Generate Changelog

```bash
# Generate for unreleased changes (prints to stdout)
canaveral changelog

# Generate for specific version
canaveral changelog --version 1.2.0

# Write to file
canaveral changelog --write

# Write to specific file
canaveral changelog --write --output CHANGES.md

# Include all commit types
canaveral changelog --all

# JSON output
canaveral changelog --format json
```

## Monorepo Changelogs

### Per-Package Changelogs

With independent versioning:

```
packages/
├── core/
│   ├── package.json
│   └── CHANGELOG.md    # Core-specific changelog
├── cli/
│   ├── package.json
│   └── CHANGELOG.md    # CLI-specific changelog
```

Configuration:
```yaml
monorepo:
  mode: independent
  changelog:
    perPackage: true
    rootChangelog: false  # No root CHANGELOG.md
```

### Root Changelog

With fixed versioning or for overview:

```yaml
monorepo:
  mode: fixed
  changelog:
    perPackage: false
    rootChangelog: true
    # Include package name in entries
    includePackageName: true
```

Output:
```markdown
## [2.0.0] - 2026-01-15

### Features

- **@myorg/core:** Add new API endpoints
- **@myorg/cli:** Support config file

### Bug Fixes

- **@myorg/utils:** Fix date parsing
```

## Customization

### Custom Templates

Use custom templates for changelog output:

```yaml
changelog:
  template: ./changelog-template.hbs
```

**Template (Handlebars):**
```handlebars
## {{version}} ({{date}})

{{#each sections}}
### {{title}}

{{#each entries}}
- {{description}} ({{hash}})
{{/each}}

{{/each}}
```

### Hooks

Transform changelog before writing:

```yaml
hooks:
  post-changelog:
    - node ./scripts/format-changelog.js
```

```javascript
// scripts/format-changelog.js
const fs = require('fs');

const changelog = fs.readFileSync('CHANGELOG.md', 'utf8');
const formatted = changelog
  .replace(/\n{3,}/g, '\n\n')  // Remove extra newlines
  .trim();

fs.writeFileSync('CHANGELOG.md', formatted + '\n');
```
