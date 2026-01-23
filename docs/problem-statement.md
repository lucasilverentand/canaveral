# Problem Statement

## Current Landscape

The release automation ecosystem is highly fragmented:

| Ecosystem | Tools |
|-----------|-------|
| npm/JavaScript | semantic-release, release-it, np, standard-version |
| Rust | release-plz, cargo-release |
| Python | bump2version, tbump, poetry version |
| Go | goreleaser |
| Multi-platform | release-please (Google) |

## Pain Points

### 1. Learning Curve
Developers working across multiple languages must learn different tools and conventions. Each tool has its own:
- Configuration format
- CLI interface
- Behavior and defaults
- Documentation style

### 2. Inconsistent Workflows
Each tool takes a different approach to:
- Version calculation
- Changelog generation
- Git tagging conventions
- Publishing mechanisms

### 3. Monorepo Complexity
Managing releases across different package types requires:
- Orchestrating multiple tools
- Maintaining separate configurations
- Handling cross-package dependencies
- Coordinating version bumps

### 4. Limited Customization
Most tools are opinionated about:
- Versioning strategies (usually SemVer-only)
- Changelog formats
- Commit conventions
- Release workflows

### 5. CI/CD Integration Challenges
Each tool requires:
- Different CI configuration
- Separate credentials management
- Custom workflow setup
- Tool-specific debugging

## The Opportunity

By creating a single, unified tool that understands the nuances of each ecosystem while providing a consistent interface, Canaveral can:

1. **Reduce cognitive overhead** - Learn one tool, use it everywhere
2. **Standardize workflows** - Same process across all projects
3. **Simplify monorepos** - Native support for multi-package projects
4. **Enable flexibility** - Pluggable strategies and adapters
5. **Improve reliability** - Built-in dry-run, validation, and rollback
