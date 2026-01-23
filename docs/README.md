# Canaveral Documentation

> Universal Release Management System

**Status:** All core phases complete (205 tests passing across 6 crates)

Canaveral is a unified release management CLI tool designed to replace the fragmented ecosystem of package-specific release tools (release-please, release-plz, semantic-release, etc.) with a single, extensible solution that works across multiple package managers and platforms.

Named after Cape Canaveral, NASA's historic launch site, Canaveral reflects the tool's mission to "launch" software releases to their target registries with precision and reliability.

## Features

- **Multi-ecosystem support**: npm, Cargo, Python, Go, Maven, Docker
- **Versioning strategies**: SemVer, CalVer, Build Numbers
- **Monorepo support**: Package discovery, dependency graph, independent/fixed versioning
- **Extensibility**: Hook system (12 lifecycle stages), plugin system
- **CI/CD integration**: GitHub Actions and GitLab CI template generators
- **Migration tools**: Migrate from semantic-release and release-please

## Documentation Index

### Project Overview
- [Vision & Goals](./vision.md) - Project vision, goals, and success criteria
- [Problem Statement](./problem-statement.md) - Current landscape and pain points

### Architecture
- [Technical Architecture](./architecture/overview.md) - System architecture and design
- [Plugin System](./architecture/plugins.md) - Extensibility and plugin development
- [Configuration](./architecture/configuration.md) - Configuration file format and auto-detection

### Design
- [CLI Interface](./design/cli.md) - Command structure and options
- [Package Adapters](./design/adapters.md) - Ecosystem-specific adapters
- [Version Strategies](./design/versioning.md) - SemVer, CalVer, build numbers
- [Changelog Generation](./design/changelog.md) - Commit parsing and changelog formats

### Implementation
- [Implementation Plan](./implementation/plan.md) - Phased development approach
- [Phase 1: Foundation](./implementation/phase-1-foundation.md) - Core CLI and git integration
- [Phase 2: Core Adapters](./implementation/phase-2-adapters.md) - npm, Cargo, Python
- [Phase 3: Monorepo](./implementation/phase-3-monorepo.md) - Monorepo support
- [Phase 4: Extended Ecosystem](./implementation/phase-4-extended.md) - Go, Maven, Docker
- [Phase 5: Polish](./implementation/phase-5-polish.md) - Plugins, CI/CD, docs

### Reference
- [Comparison](./comparison.md) - Comparison with existing tools
- [Risk Analysis](./risk-analysis.md) - Technical and adoption risks
- [Roadmap](./roadmap.md) - Future development plans
