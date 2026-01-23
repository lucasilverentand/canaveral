# Roadmap

## Current Status

**Version:** 0.1.0 (All Core Phases Complete)

All five implementation phases have been completed. The project is ready for initial release and real-world testing.

## Completed Phases

### v0.1.0 - Foundation (Phase 1) ✅

Core CLI with SemVer versioning and changelog generation.

**Features:**
- [x] CLI framework with Clap (Rust)
- [x] Configuration system (YAML/TOML)
- [x] Git integration (commits, tags)
- [x] SemVer strategy
- [x] Conventional Commits parsing
- [x] Markdown changelog generation
- [x] Basic release workflow

### v0.2.0 - Core Adapters (Phase 2) ✅

npm, Cargo, and Python publishing support.

**Features:**
- [x] npm adapter (package.json, npm publish)
- [x] Cargo adapter (Cargo.toml, cargo publish)
- [x] Python adapter (pyproject.toml, twine)
- [x] Credential management
- [x] Full dry-run mode
- [x] Pre-flight validation

### v0.3.0 - Monorepo Support (Phase 3) ✅

Multi-package repository support.

**Features:**
- [x] Package discovery
- [x] Change detection
- [x] Independent versioning
- [x] Fixed versioning
- [x] Dependency graph
- [x] Coordinated publishing
- [x] Package filtering

### v0.4.0 - Extended Ecosystem (Phase 4) ✅

Additional ecosystems and versioning strategies.

**Features:**
- [x] Go modules adapter
- [x] Maven adapter
- [x] Docker adapter
- [x] CalVer strategy
- [x] Build number strategy

### v1.0.0 - Production Ready (Phase 5) ✅

Full feature set with extensibility.

**Features:**
- [x] Plugin system (external subprocess plugins)
- [x] Hook system (12 lifecycle stages)
- [x] GitHub Actions workflow
- [x] GitLab CI templates
- [x] Migration tools (semantic-release, release-please)
- [x] Cross-platform release automation
- [x] 205 tests passing across 6 crates

---

## Short-term Roadmap (6 months post-v1)

### v1.1.0 - Mobile & Enterprise

- [ ] iOS adapter (Info.plist, App Store Connect)
- [ ] Android adapter (build.gradle, Play Store)
- [ ] NuGet adapter (.csproj, nuget.org)
- [ ] Enhanced changelog templates
- [ ] Custom changelog formats

### v1.2.0 - CI/CD Excellence

- [ ] Official GitHub Action in marketplace
- [ ] Official GitLab CI component
- [ ] CircleCI orb
- [ ] Jenkins plugin
- [ ] Azure Pipelines extension

### v1.3.0 - Notifications & Integration

- [ ] Slack integration
- [ ] Discord integration
- [ ] Microsoft Teams integration
- [ ] Webhook support
- [ ] Email notifications

---

## Medium-term Roadmap (12 months post-v1)

### v1.4.0 - Analytics & Insights

- [ ] Release analytics dashboard
- [ ] Version history visualization
- [ ] Changelog diff viewer
- [ ] Release frequency metrics
- [ ] Breaking change tracking

### v1.5.0 - Dependency Management

- [ ] Automated dependency update PRs
- [ ] Security vulnerability scanning
- [ ] Dependency changelog aggregation
- [ ] Update strategy configuration

### v1.6.0 - Web Dashboard

- [ ] Web UI for release history
- [ ] Release approval workflows
- [ ] Team permissions
- [ ] Audit logs

---

## Long-term Roadmap (18+ months post-v1)

### v2.0.0 - Intelligence

- [ ] AI-powered changelog summarization
- [ ] Automatic breaking change detection
- [ ] Release notes generation
- [ ] Commit message suggestions
- [ ] Version recommendation

### v2.1.0 - Advanced Deployment

- [ ] Canary releases
- [ ] Progressive rollouts
- [ ] Feature flags integration
- [ ] A/B testing support
- [ ] Rollback automation

### v2.2.0 - Metrics & Observability

- [ ] Deployment success tracking
- [ ] Error rate correlation
- [ ] Performance metrics integration
- [ ] SLO monitoring
- [ ] Incident linking

### v2.3.0 - Enterprise Features

- [ ] SAML/SSO authentication
- [ ] Audit logging
- [ ] Compliance reports
- [ ] Release policies
- [ ] Approval workflows
- [ ] Custom RBAC

---

## Community Wishlist

Features suggested by the community (not yet scheduled):

- [ ] Homebrew adapter
- [ ] Chocolatey adapter
- [ ] APT/DEB packaging
- [ ] RPM packaging
- [ ] Flatpak/Snap support
- [ ] Helm chart releases
- [ ] Terraform module releases
- [ ] VS Code extension
- [ ] JetBrains plugin
- [ ] GitHub CLI extension

---

## Contributing to the Roadmap

We welcome community input on the roadmap:

1. **Feature Requests:** Open an issue with the `enhancement` label
2. **Voting:** React with 👍 on issues you want prioritized
3. **Discussion:** Join roadmap discussions in GitHub Discussions
4. **Implementation:** PRs welcome for roadmap items

## Version Policy

- **Major versions (x.0.0):** Breaking changes, new architecture
- **Minor versions (0.x.0):** New features, backward compatible
- **Patch versions (0.0.x):** Bug fixes, security patches

We follow semantic versioning and maintain a changelog for all releases.
