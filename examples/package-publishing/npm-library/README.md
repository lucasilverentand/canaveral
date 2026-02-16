# NPM Library Publishing Example

Demonstrates publishing a JavaScript/TypeScript library to NPM using Canaveral.

## Setup

```bash
npm install
```

## Build

```bash
npm run build
```

This compiles TypeScript to JavaScript in the `dist/` directory.

## Publish Workflow

### Manual Publishing

```bash
# Package the library
npm pack
# Produces: example-my-library-1.0.0.tgz

# Dry run (test without publishing)
canaveral publish npm example-my-library-1.0.0.tgz --dry-run

# Publish to NPM
export NPM_TOKEN="your-npm-token"
canaveral publish npm example-my-library-1.0.0.tgz

# Publish with dist-tag
canaveral publish npm example-my-library-1.0.0.tgz --tag next
```

### Version Management

```bash
# Show current version
canaveral version --current

# Calculate next version (auto-detected from commits)
canaveral version

# Force a specific bump type
canaveral version --release-type patch
canaveral version --release-type minor
canaveral version --release-type major
```

### Full Release

```bash
# Create a release (bumps version, generates changelog, creates tag)
canaveral release --yes

# Release a specific version
canaveral release --version 1.2.3 --yes

# Dry run
canaveral release --dry-run
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Publish to NPM

on:
  release:
    types: [published]

jobs:
  publish:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: '18'
          registry-url: 'https://registry.npmjs.org'

      - name: Install dependencies
        run: npm ci

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Build package
        run: npm pack

      - name: Publish to NPM
        run: canaveral publish npm *.tgz
        env:
          NPM_TOKEN: ${{ secrets.NPM_TOKEN }}
```

### GitLab CI

```yaml
publish:
  stage: deploy
  image: rust:latest
  only:
    - tags
  before_script:
    - apt-get update && apt-get install -y nodejs npm
    - cargo install canaveral
    - npm ci
  script:
    - npm pack
    - canaveral publish npm *.tgz
  variables:
    NPM_TOKEN: $CI_NPM_TOKEN
```

## Configuration Options

You can configure NPM publishing in `canaveral.toml`:

```toml
name = "my-npm-library"

[versioning]
strategy = "semver"
tag_format = "v{version}"

[[packages]]
name = "@example/my-library"
path = "."
type = "npm"
publish = true

[publish.registries.npm]
url = "https://registry.npmjs.org"
# Token will be read from NPM_TOKEN env var
```

## Authentication

Canaveral looks for your NPM token in this order:

1. `NPM_TOKEN` environment variable
2. `--token` flag on the command line

## Learn More

- [NPM Publishing Guide](https://docs.npmjs.com/packages-and-modules/contributing-packages-to-the-registry)
- [Semantic Versioning](https://semver.org)
