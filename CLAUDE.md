# Canaveral

## What this is

Canaveral is a unified launch system for software — a single Rust CLI that replaces the patchwork of turborepo, tuist, release-please, semantic-release, fastlane, goreleaser, and similar tools. It handles the full lifecycle: build orchestration, smart test selection, release management, publishing, app store distribution, and marketing material generation.

The name comes from Cape Canaveral — we launch software.

## Architecture

Rust workspace with 10 crates. Each crate has a focused responsibility:

- `canaveral` — CLI binary (clap). Entry point, command routing, output formatting.
- `canaveral-core` — Config loading (YAML/TOML), hook system (12 lifecycle stages), plugin registry, monorepo workspace detection, workflow orchestration, CI/CD templates.
- `canaveral-git` — Git operations via git2 (libgit2). Commit parsing, tag management, remote ops. No shelling out to git.
- `canaveral-changelog` — Conventional commit parsing, changelog generation, customizable formatters.
- `canaveral-strategies` — Version calculation. Trait-based: SemVer, CalVer, build numbers. Pluggable for custom strategies.
- `canaveral-adapters` — Package manager integrations. Each adapter implements a common trait: read manifest, write version, publish. Supports npm, Cargo, Python/PyPI, Go, Maven, Docker.
- `canaveral-signing` — Code signing providers (macOS codesign, Windows signtool, Android apksigner, GPG). Team vault for shared certs. Match-style sync (Git, S3, GCS, Azure).
- `canaveral-stores` — Uploaders for app stores and registries: App Store Connect, Google Play, Microsoft Store, npm, crates.io. Credential management, retry logic.
- `canaveral-metadata` — App store metadata management. Fastlane-compatible storage layout. Validation against Apple/Google requirements.
- `canaveral-frameworks` — Build/test adapters for frameworks: Flutter, React Native, native iOS/Android, Tauri, Electron, Vite, Next.js, Astro.

## Key patterns

- **Trait-based adapters** — `PackageAdapter` trait in canaveral-adapters, `VersionStrategy` trait in canaveral-strategies, `StoreUploader` trait in canaveral-stores. Add new ecosystems by implementing the trait.
- **Layered architecture** — CLI → Orchestration → Strategy → Adapter → Infrastructure. Each layer only calls downward.
- **Config-driven** — `canaveral.yaml` or `canaveral.toml`. Auto-detection as fallback. CLI flags override config.
- **Monorepo-first** — Workspace detection (Cargo, npm, pnpm, yarn, lerna, nx, turbo), dependency graph with topological sorting, change detection via git diff, coordinated or independent versioning.

## Building and testing

```bash
cargo build              # debug build
cargo build --release    # release build (LTO enabled, stripped)
cargo test               # run all 205+ tests
cargo test -p canaveral-core  # test a specific crate
```

## What's implemented

- Core CLI with all command stubs (init, version, changelog, release, status, validate, doctor, publish, build, test, signing, metadata, testflight, firebase, screenshots, match, completions)
- Git integration (commit parsing, tags, remote operations)
- Version strategies (SemVer, CalVer, build numbers)
- Changelog generation from conventional commits
- Package adapters (npm, Cargo, Python, Go, Maven, Docker)
- Monorepo support (workspace detection, dependency graph, change detection, coordinated publishing)
- Hook system (12 lifecycle stages)
- Plugin system (external subprocess plugins)
- Code signing providers
- App store uploaders (stubs with API structure)
- Framework adapters (Flutter, React Native, Vite, Next.js, Astro, Tauri, Electron)
- npm and crates.io registry publishing

## What's next — the big picture

The vision is to cover the full lifecycle of a deployable:

1. **Build orchestration** — understand the workspace dependency graph, run builds in parallel respecting dependencies, cache results. This is the turborepo/nx replacement layer.
2. **Smart test selection** — given a change, compute the minimal set of tests to run. Use the dependency graph + file-level change detection. Don't run the iOS tests if only a Rust crate changed.
3. **CI workflow management** — generate and manage CI configs (GitHub Actions, GitLab CI, etc.) that use canaveral under the hood. The CI config should be thin — just "run canaveral" — with all the logic in the tool.
4. **PR-to-main pipeline** — validate branches, run checks, manage merge requirements. Work with the project's branching model (trunk-based, gitflow, whatever).
5. **Release minting** — version calculation, changelog generation, git tags, GitHub/GitLab releases with release notes and binary attachments. This is mostly built.
6. **Publishing** — publish to registries (npm, crates.io, PyPI, etc.) and app stores in dependency order with rollback on failure. Partially built.
7. **Marketing material generation** — auto-generate human-readable release notes (not just commit logs), capture and frame app store screenshots, manage store metadata. Partially built for metadata/screenshots.

## Code conventions

- Rust 2021 edition, MSRV 1.75
- Use `thiserror` for library errors, `anyhow` for CLI errors
- Use `tracing` for logging, not `println!` or `log`
- Async with tokio where needed (HTTP, I/O), sync otherwise
- Tests go in the same file (`#[cfg(test)] mod tests`) or in a `tests/` directory for integration tests
- Config structs derive `Serialize, Deserialize, Debug, Clone`
- CLI structs derive `Parser` (clap)

## File layout conventions

- Each crate's `src/lib.rs` re-exports the public API
- Adapters/strategies/stores each get their own module or file
- Keep modules focused — split when a file exceeds ~500 lines
