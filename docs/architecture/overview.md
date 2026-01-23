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

```
canaveral/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── canaveral/          # Main binary crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── main.rs
│   │       └── cli/        # Command definitions
│   ├── canaveral-core/     # Core library
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── workflow.rs # Release workflow
│   │       ├── hooks.rs    # Hook system
│   │       └── validation.rs
│   ├── canaveral-git/      # Git operations
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── commits.rs
│   │       └── tags.rs
│   ├── canaveral-strategies/ # Version strategies
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── semver.rs
│   │       ├── calver.rs
│   │       └── buildnum.rs
│   ├── canaveral-adapters/ # Package adapters
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── npm/
│   │       ├── cargo/
│   │       └── python/
│   └── canaveral-changelog/ # Changelog generation
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── parser.rs
│           └── generator.rs
├── docs/                   # Documentation
└── tests/                  # Integration tests
```

## Key Dependencies

```toml
# Cargo.toml (workspace)
[workspace.dependencies]
# CLI
clap = { version = "4", features = ["derive", "env"] }
dialoguer = "0.11"
indicatif = "0.17"
console = "0.15"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
toml = "0.8"

# Git
git2 = "0.18"

# HTTP
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
tokio = { version = "1", features = ["full"] }

# Utilities
thiserror = "1"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
semver = "1"
chrono = "0.4"
regex = "1"
glob = "0.3"
keyring = "2"
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

### 3. Plugin System via Dynamic Loading
- Plugins as shared libraries (.so, .dylib, .dll)
- Or WASM plugins for sandboxed execution
- Clear trait interfaces for extension points

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
