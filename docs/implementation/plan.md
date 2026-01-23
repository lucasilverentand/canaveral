# Implementation Plan

This document outlines the phased approach to building Canaveral, with each phase building on the previous to deliver incremental value.

## Phase Overview

| Phase | Focus | Key Deliverables |
|-------|-------|------------------|
| 1 | Foundation | CLI framework, Git integration, SemVer, basic changelog |
| 2 | Core Adapters | npm, Cargo, Python adapters, credentials, dry-run |
| 3 | Monorepo | Change detection, versioning modes, dependency graph |
| 4 | Extended Ecosystem | Go, Maven, Docker, CalVer, build numbers |
| 5 | Polish | Plugin system, hooks, CI/CD guides, migration tools |

## Development Principles

### 1. Test-Driven Development
- Write tests before implementation
- High coverage for core functionality
- Integration tests with real registries (staging)

### 2. Incremental Delivery
- Each phase produces a usable tool
- Features are complete before moving on
- Documentation updated with each phase

### 3. User Feedback
- Release early betas for community testing
- Iterate based on real-world usage
- Prioritize issues that block adoption

## Phase 1: Foundation

**Goal**: Create a working CLI that can bump SemVer versions and generate changelogs for a single-package project.

### Deliverables

1. **CLI Framework**
   - Project scaffolding with Bun
   - Commander.js command structure
   - Help system and version info
   - Colored output and progress indicators

2. **Configuration System**
   - YAML configuration loading
   - JSON Schema validation
   - Default value handling
   - CLI flag overrides

3. **Git Integration**
   - Read commit history
   - Parse conventional commits
   - Create commits and tags
   - Push to remote

4. **SemVer Strategy**
   - Parse semantic versions
   - Calculate bump type from commits
   - Handle pre-release versions
   - Validate version strings

5. **Changelog Generation**
   - Group commits by type
   - Format as Markdown
   - Prepend to CHANGELOG.md
   - Link to commits/PRs

### Exit Criteria
- [ ] `canaveral version` calculates and updates version
- [ ] `canaveral changelog` generates changelog
- [ ] `canaveral release --no-publish` does version + changelog + git
- [ ] Conventional commits are parsed correctly
- [ ] Configuration file is respected

## Phase 2: Core Adapters

**Goal**: Support publishing to npm, crates.io, and PyPI with proper credential management.

### Deliverables

1. **Adapter Interface**
   - Define abstract adapter contract
   - Version read/write methods
   - Publish interface
   - Detection logic

2. **npm Adapter**
   - Read/write package.json
   - npm publish command
   - npm token authentication
   - Scoped package support

3. **Cargo Adapter**
   - Read/write Cargo.toml
   - cargo publish command
   - crates.io token auth
   - Workspace member handling

4. **Python Adapter**
   - Read/write pyproject.toml
   - twine/poetry publish
   - PyPI token auth
   - Build system detection

5. **Credential Management**
   - Environment variable support
   - System keychain integration
   - Credential validation pre-publish
   - Secure token handling

6. **Dry-Run Mode**
   - Preview all changes
   - Show what would be published
   - No side effects
   - Detailed output

### Exit Criteria
- [ ] `canaveral release` publishes to npm
- [ ] `canaveral release` publishes to crates.io
- [ ] `canaveral release` publishes to PyPI
- [ ] `canaveral release --dry-run` shows changes without executing
- [ ] Credentials are securely handled

## Phase 3: Monorepo Support

**Goal**: Handle multi-package repositories with coordinated or independent versioning.

### Deliverables

1. **Change Detection**
   - Git-based file change detection
   - Map changes to packages
   - Handle shared dependencies
   - Configurable ignore patterns

2. **Independent Versioning**
   - Per-package version tracking
   - Package-specific changelogs
   - Selective publishing
   - Tag naming (package@version)

3. **Fixed Versioning**
   - Shared version across packages
   - Unified changelog
   - All-or-nothing publishing
   - Single tag

4. **Dependency Graph**
   - Build internal dependency graph
   - Topological sort for publish order
   - Auto-bump dependents
   - Circular dependency detection

5. **Filtering**
   - Filter by package name
   - Filter by glob pattern
   - Filter by changed
   - Exclude patterns

### Exit Criteria
- [ ] Detect which packages changed since last release
- [ ] Independent versioning works with 10+ packages
- [ ] Fixed versioning keeps all packages in sync
- [ ] Internal dependencies are updated automatically
- [ ] `--filter` flags work correctly

## Phase 4: Extended Ecosystem

**Goal**: Support additional ecosystems and versioning strategies.

### Deliverables

1. **Go Adapter**
   - Read go.mod
   - Tag-based versioning
   - Module path handling
   - No registry publish (git tags only)

2. **Maven Adapter**
   - Read/write pom.xml
   - mvn deploy command
   - Maven Central auth
   - Parent POM handling

3. **Docker Adapter**
   - Read Dockerfile
   - docker build & push
   - Multiple registries
   - Tag strategies

4. **CalVer Strategy**
   - Multiple CalVer formats
   - Date-based versioning
   - Micro version handling
   - Format customization

5. **Build Number Strategy**
   - Monotonic build numbers
   - iOS/Android compatibility
   - Hybrid with SemVer
   - CI integration

### Exit Criteria
- [ ] Go modules can be released (via tags)
- [ ] Maven packages can be published
- [ ] Docker images can be built and pushed
- [ ] CalVer versioning works correctly
- [ ] Build numbers increment properly

## Phase 5: Polish & Extensibility

**Goal**: Enable community extensions and provide production-ready tooling.

### Deliverables

1. **Plugin System**
   - Plugin discovery
   - Plugin configuration
   - Extension points
   - Documentation

2. **Hook System**
   - Lifecycle hook execution
   - Script hooks (shell commands)
   - Plugin hooks (JavaScript)
   - Error handling

3. **CI/CD Integration**
   - GitHub Actions
   - GitLab CI
   - CircleCI
   - General CI guide

4. **Migration Tools**
   - semantic-release migration
   - release-please migration
   - release-plz migration
   - Configuration conversion

5. **Documentation**
   - API reference
   - Plugin development guide
   - Migration guides
   - Troubleshooting

### Exit Criteria
- [ ] Custom plugins can be loaded and executed
- [ ] All hook stages execute correctly
- [ ] Official CI/CD actions are published
- [ ] Migration from 3 tools is documented
- [ ] Full API documentation is complete

## Timeline Summary

```
Phase 1: Foundation         ████████████████
Phase 2: Core Adapters                      ████████████████
Phase 3: Monorepo                                           ████████████████
Phase 4: Extended                                                           ████████████████
Phase 5: Polish                                                                             ████████████████
```

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Registry API changes | Abstract registry interactions, monitor APIs |
| Credential security | Use system stores, audit credential handling |
| Git edge cases | Comprehensive test suite, clear error messages |
| Adoption resistance | Migration tools, compatibility modes, clear value prop |
