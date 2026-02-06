# Vite Example with Canaveral

This example demonstrates building and managing a Vite application using Canaveral.

## Prerequisites

- Node.js 18+ and npm
- Canaveral CLI installed

## Setup

```bash
npm install
```

## Build with Canaveral

```bash
# Development build
canaveral build --platform web --profile debug

# Production build
canaveral build --platform web --profile release
```

Output will be in the `dist/` directory.

## Version Management

```bash
# Get current version
canaveral version get

# Set new version
canaveral version set 1.1.0

# Bump version
canaveral version bump patch
```

## Publishing to NPM

After building your package:

```bash
# Package the library
npm pack

# Dry run publish
canaveral publish npm vite-example-1.0.0.tgz --dry-run

# Publish to NPM
canaveral publish npm vite-example-1.0.0.tgz

# Publish with dist-tag
canaveral publish npm vite-example-1.0.0.tgz --tag next
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Build and Deploy

on:
  push:
    branches: [main]

jobs:
  build:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: '18'

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Build
        run: canaveral build --platform web --profile release

      - name: Deploy
        run: # Your deployment command
```

## Learn More

- [Canaveral Documentation](https://canaveral.dev/docs)
- [Vite Documentation](https://vitejs.dev)
- [Web Framework Support](https://canaveral.dev/docs/frameworks/vite)
