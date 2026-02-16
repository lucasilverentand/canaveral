# Technical Architecture

## Technology Stack

| Component | Choice | Rationale |
|-----------|--------|-----------|
| Core Language | Rust | Single binary distribution, no runtime required |
| CLI Framework | clap | Mature, derive macros, shell completions |
| Git Integration | git2 (libgit2) | Native git operations, no git CLI dependency |
| Configuration | serde + toml/yaml | Fast parsing, strong typing |
| HTTP Client | reqwest | Async HTTP, TLS support |
| Async Runtime | tokio | Industry standard async runtime |

## Why Rust?

1. **Single Binary Distribution** - Users download one executable, no runtime needed
2. **Cross-Platform** - Compile for macOS, Linux, Windows from one codebase
3. **Performance** - Fast startup, low memory usage
4. **Reliability** - Strong type system catches errors at compile time
5. **Ecosystem** - Excellent crates for CLI, git, HTTP, serialization

## System Layers

The architecture follows a layered design with clear separation of concerns:

```
┌─────────────────────────────────────────────────────────────┐
│                      CLI Layer                              │
│         Command parsing, argument validation, UI            │
├─────────────────────────────────────────────────────────────┤
│                  Orchestration Layer                        │
│       Workflow coordination, hook execution, errors         │
├─────────────────────────────────────────────────────────────┤
│                    Strategy Layer                           │
│     Version calculation, changelog generation, parsing      │
├─────────────────────────────────────────────────────────────┤
│                    Adapter Layer                            │
│        Package manager-specific implementations             │
├─────────────────────────────────────────────────────────────┤
│                 Infrastructure Layer                        │
│      Git operations, file I/O, HTTP, credentials            │
└─────────────────────────────────────────────────────────────┘
```

## Layer Responsibilities

### CLI Layer
- Parse commands and arguments (clap)
- Validate user input
- Format output (JSON, table, plain text)
- Handle interactive prompts (dialoguer)
- Display progress and errors (indicatif)

### Orchestration Layer
- Coordinate multi-step workflows
- Execute lifecycle hooks
- Handle errors and rollback
- Manage dry-run mode
- Log operations (tracing)

### Strategy Layer
- Calculate version bumps (SemVer, CalVer, etc.)
- Parse commit messages
- Generate changelogs
- Determine release types

### Adapter Layer
- Read/write package manifests
- Authenticate with registries
- Publish packages
- Handle ecosystem-specific quirks

### Infrastructure Layer
- Git operations (git2)
- File system operations (std::fs, tokio::fs)
- HTTP requests to registries (reqwest)
- Credential management (keyring)

## Crate Structure

The workspace contains 11 crates (1 binary + 10 libraries):

```
canaveral/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── canaveral/              # Main binary crate (CLI)
│   ├── canaveral-core/         # Config, hooks, plugins, migration, monorepo, workflow
│   ├── canaveral-git/          # Git operations via git2 (libgit2)
│   ├── canaveral-changelog/    # Conventional commit parsing, changelog generation
│   ├── canaveral-strategies/   # Version strategies (SemVer, CalVer, build numbers)
│   ├── canaveral-adapters/     # Package adapters (npm, Cargo, Python, Go, Maven, Docker)
│   ├── canaveral-signing/      # Code signing (macOS, Windows, Android, GPG)
│   ├── canaveral-stores/       # App store uploaders (Apple, Google Play, Microsoft, npm, crates.io)
│   ├── canaveral-metadata/     # App store metadata management
│   ├── canaveral-frameworks/   # Framework adapters (Flutter, React Native, Vite, Next.js, etc.)
│   └── canaveral-tasks/        # Task orchestration (DAG, caching, smart test selection)
├── docs/                       # Documentation
└── tests/                      # Integration tests
```

## Key Dependencies

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive", "env"] }
clap_complete = "4"
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"
toml_edit = "0.22"

# Git
git2 = "0.18"

# Async
tokio = { version = "1", features = ["full"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Logging/tracing
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Versioning
semver = "1"
chrono = { version = "0.4", features = ["serde"] }

# Utilities
regex = "1"
glob = "0.3"
globset = "0.4"
walkdir = "2"
tempfile = "3"
url = "2"
dirs = "5"
which = "7"
sha2 = "0.10"
```

## Data Flow

```
User Command
     │
     ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│  Parse  │────▶│ Validate│────▶│  Load   │
│ Command │     │  Input  │     │ Config  │
└─────────┘     └─────────┘     └─────────┘
                                     │
     ┌───────────────────────────────┘
     ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│ Detect  │────▶│Calculate│────▶│ Generate│
│ Changes │     │ Version │     │Changelog│
└─────────┘     └─────────┘     └─────────┘
                                     │
     ┌───────────────────────────────┘
     ▼
┌─────────┐     ┌─────────┐     ┌─────────┐
│  Update │────▶│  Commit │────▶│ Publish │
│Manifests│     │  & Tag  │     │  to Reg │
└─────────┘     └─────────┘     └─────────┘
```

## Key Design Decisions

### 1. Single Binary Distribution
- No runtime dependencies (Node.js, Python, etc.)
- Users download and run immediately
- Simplifies CI/CD integration
- Cross-compile for all platforms

### 2. Workspace Crate Structure
- Separation of concerns
- Faster incremental compilation
- Reusable components
- Easier testing

### 3. Plugin System via External Subprocess
- Plugins as external executables communicating via JSON over stdin/stdout
- Three plugin types: Adapter, Strategy, Formatter
- Clear trait interfaces for extension points
- Plugin registry with discovery from search paths

### 4. Configuration-as-Code
- YAML for human readability (primary)
- TOML as alternative
- JSON Schema for validation
- Auto-detection as fallback
- Override via CLI flags

### 5. Trait-Based Adapters
- Abstract trait for all package managers
- Ecosystem-specific implementations
- Consistent behavior across adapters
- Easy to add new ecosystems
