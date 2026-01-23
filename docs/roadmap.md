# Roadmap

## Current Status

**Version:** Pre-release (Development)

The project is currently in the planning and initial development phase.

## Release Timeline

### v0.1.0 - Foundation (Phase 1)

Core CLI with SemVer versioning and changelog generation.

**Features:**
- [ ] CLI framework with Commander.js
- [ ] Configuration system (YAML)
- [ ] Git integration (commits, tags)
- [ ] SemVer strategy
- [ ] Conventional Commits parsing
- [ ] Markdown changelog generation
- [ ] Basic release workflow

### v0.2.0 - Core Adapters (Phase 2)

npm, Cargo, and Python publishing support.

**Features:**
- [ ] npm adapter (package.json, npm publish)
- [ ] Cargo adapter (Cargo.toml, cargo publish)
- [ ] Python adapter (pyproject.toml, twine)
- [ ] Credential management
- [ ] Full dry-run mode
- [ ] Pre-flight validation

### v0.3.0 - Monorepo Support (Phase 3)

Multi-package repository support.

**Features:**
- [ ] Package discovery
- [ ] Change detection
- [ ] Independent versioning
- [ ] Fixed versioning
- [ ] Dependency graph
- [ ] Coordinated publishing
- [ ] Package filtering

### v0.4.0 - Extended Ecosystem (Phase 4)

Additional ecosystems and versioning strategies.

**Features:**
- [ ] Go modules adapter
- [ ] Maven adapter
- [ ] Docker adapter
- [ ] CalVer strategy
- [ ] Build number strategy

### v1.0.0 - Production Ready (Phase 5)

Full feature set with extensibility.

**Features:**
- [ ] Plugin system
- [ ] Hook system
- [ ] GitHub Actions
- [ ] GitLab CI templates
- [ ] Migration tools
- [ ] Complete documentation
- [ ] Stability guarantees

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
