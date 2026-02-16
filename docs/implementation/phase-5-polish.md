# Phase 5: Polish & Extensibility ✅

**Status:** Complete

**Goal**: Enable community extensions and provide production-ready tooling.

## Tasks

### 5.1 Plugin System Implementation

- [x] Plugin discovery from multiple sources
- [x] External subprocess plugin execution (JSON over stdin/stdout)
- [x] Plugin configuration and lifecycle
- [x] Error isolation

Plugins are external executables that communicate via JSON over stdin/stdout:

```rust
// crates/canaveral-core/src/plugins/mod.rs

pub struct PluginRegistry {
    plugins: Vec<ExternalPlugin>,
}

pub struct ExternalPlugin {
    pub config: PluginConfig,
    pub info: Option<PluginInfo>,
}

impl ExternalPlugin {
    /// Execute a plugin action by sending JSON to stdin and reading from stdout
    pub fn execute(&self, action: &str, input: serde_json::Value) -> Result<serde_json::Value> {
        let request = PluginRequest {
            action: action.to_string(),
            input,
            config: self.config.config.clone().unwrap_or_default(),
        };
        // Spawn process, write JSON to stdin, read JSON from stdout
        // ...
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PluginType {
    Adapter,
    Strategy,
    Formatter,
}
```

### 5.2 Hook System

- [x] Shell command execution
- [x] Script hooks
- [x] Environment variable passing
- [x] Timeout handling
- [x] Error propagation
- [x] 12 lifecycle stages (pre/post for release, version, changelog, commit, tag, publish)

```rust
// crates/canaveral-core/src/hooks/executor.rs
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

pub struct HookExecutor {
    timeout: Duration,
}

impl HookExecutor {
    pub async fn execute(
        &self,
        stage: HookStage,
        hooks: &[Hook],
        context: &HookContext,
    ) -> Result<()> {
        for hook in hooks {
            if context.skip {
                tracing::debug!("Skipping remaining hooks for {:?}", stage);
                break;
            }

            tracing::info!("Running hook: {} at {:?}", hook.name(), stage);

            let result = timeout(
                self.timeout,
                self.run_hook(hook, context)
            ).await;

            match result {
                Ok(Ok(())) => {
                    tracing::debug!("Hook {} completed successfully", hook.name());
                }
                Ok(Err(e)) => {
                    tracing::error!("Hook {} failed: {}", hook.name(), e);
                    return Err(e);
                }
                Err(_) => {
                    return Err(anyhow::anyhow!(
                        "Hook {} timed out after {:?}",
                        hook.name(),
                        self.timeout
                    ));
                }
            }
        }
        Ok(())
    }

    async fn run_hook(&self, hook: &Hook, context: &HookContext) -> Result<()> {
        match hook {
            Hook::Shell(cmd) => {
                let interpolated = self.interpolate(cmd, context);
                let output = Command::new("sh")
                    .arg("-c")
                    .arg(&interpolated)
                    .envs(&context.env)
                    .output()
                    .await?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!(
                        "Hook failed with exit code {}: {}",
                        output.status.code().unwrap_or(-1),
                        String::from_utf8_lossy(&output.stderr)
                    ));
                }
            }
            Hook::Script(path) => {
                let output = Command::new(path)
                    .envs(&context.env)
                    .env("CANAVERAL_VERSION", &context.version)
                    .env("CANAVERAL_PREV_VERSION", &context.previous_version)
                    .output()
                    .await?;

                if !output.status.success() {
                    return Err(anyhow::anyhow!("Script hook failed"));
                }
            }
        }
        Ok(())
    }

    fn interpolate(&self, template: &str, context: &HookContext) -> String {
        template
            .replace("{{version}}", &context.version)
            .replace("{{previous_version}}", &context.previous_version)
            .replace("{{tag}}", &context.tag)
    }
}
```

### 5.3 CI/CD Integration

- [x] GitHub Actions workflow generator
- [x] GitLab CI template generator
- [x] Cross-platform release workflow (6 targets)

**GitHub Action:**
```yaml
# .github/actions/canaveral/action.yml
name: 'Canaveral Release'
description: 'Run Canaveral release automation'

inputs:
  command:
    description: 'Command to run'
    default: 'release'
  dry-run:
    description: 'Run in dry-run mode'
    default: 'false'
  version:
    description: 'Canaveral version to use'
    default: 'latest'

runs:
  using: 'composite'
  steps:
    - name: Install Canaveral
      shell: bash
      run: |
        if [ "${{ inputs.version }}" = "latest" ]; then
          cargo install canaveral
        else
          cargo install canaveral -s -- --version ${{ inputs.version }}
        fi

    - name: Run Canaveral
      shell: bash
      run: |
        canaveral ${{ inputs.command }} \
          ${{ inputs.dry-run == 'true' && '--dry-run' || '' }}
```

### 5.4 Migration Tools

- [x] Analyze existing configs
- [x] Generate Canaveral config
- [x] Show differences
- [x] semantic-release migrator
- [x] release-please migrator

```rust
// crates/canaveral/src/cli/migrate.rs

pub async fn migrate_from_semantic_release(path: &Path) -> Result<Config> {
    let sr_config = load_semantic_release_config(path)?;

    let mut config = Config::default();
    config.strategy = Strategy::Semver;

    // Convert branches
    if let Some(branches) = sr_config.get("branches") {
        config.git.branch = branches.as_array()
            .and_then(|b| b.first())
            .and_then(|b| b.as_str())
            .unwrap_or("main")
            .to_string();
    }

    // Convert plugins to hooks
    if let Some(plugins) = sr_config.get("plugins").and_then(|p| p.as_array()) {
        for plugin in plugins {
            match plugin_name(plugin) {
                "@semantic-release/exec" => {
                    if let Some(cmd) = get_plugin_option(plugin, "prepareCmd") {
                        config.hooks.pre_publish.push(cmd);
                    }
                    if let Some(cmd) = get_plugin_option(plugin, "successCmd") {
                        config.hooks.post_publish.push(cmd);
                    }
                }
                "@semantic-release/git" => {
                    if let Some(msg) = get_plugin_option(plugin, "message") {
                        config.git.commit_message_format = msg;
                    }
                }
                _ => {}
            }
        }
    }

    Ok(config)
}
```

### 5.5 Documentation & Testing

- [x] API documentation (rustdoc)
- [x] Implementation documentation
- [x] E2E test suite (530+ tests across 11 crates)
- [ ] Performance benchmarks (future enhancement)

```rust
// tests/e2e/release_test.rs
use assert_cmd::Command;
use tempfile::TempDir;

#[test]
fn test_npm_release_dry_run() {
    let dir = TempDir::new().unwrap();

    // Create package.json
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "test-pkg", "version": "1.0.0"}"#,
    ).unwrap();

    // Initialize git repo
    Command::new("git")
        .args(["init"])
        .current_dir(dir.path())
        .assert()
        .success();

    Command::new("git")
        .args(["add", "."])
        .current_dir(dir.path())
        .assert()
        .success();

    Command::new("git")
        .args(["commit", "-m", "feat: initial commit"])
        .current_dir(dir.path())
        .assert()
        .success();

    // Run canaveral
    Command::cargo_bin("canaveral")
        .unwrap()
        .args(["release", "--dry-run"])
        .current_dir(dir.path())
        .assert()
        .success()
        .stdout(predicates::str::contains("1.1.0"));
}

#[test]
fn test_version_calculation_performance() {
    use std::time::Instant;

    let start = Instant::now();
    // ... setup and run version calculation
    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() < 100, "Version calculation took too long");
}
```

## Cross-Compilation & Distribution

### Build targets
```bash
# Linux
cargo build --release --target x86_64-unknown-linux-gnu
cargo build --release --target aarch64-unknown-linux-gnu

# macOS
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

# Windows
cargo build --release --target x86_64-pc-windows-msvc
```

### GitHub Release workflow
```yaml
# .github/workflows/release.yml
name: Release

on:
  push:
    tags: ['v*']

jobs:
  build:
    strategy:
      matrix:
        include:
          - os: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - os: ubuntu-latest
            target: aarch64-unknown-linux-gnu
          - os: macos-latest
            target: x86_64-apple-darwin
          - os: macos-latest
            target: aarch64-apple-darwin
          - os: windows-latest
            target: x86_64-pc-windows-msvc

    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target ${{ matrix.target }}

      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: canaveral-${{ matrix.target }}
          path: target/${{ matrix.target }}/release/canaveral*
```

## Definition of Done

Phase 5 is complete when:

1. [x] Plugins can be discovered and loaded
2. [x] Custom strategy plugins work
3. [x] Custom adapter plugins work
4. [x] All hook stages execute correctly
5. [x] GitHub Actions workflow created
6. [x] GitLab CI template generator works
7. [x] Migration from semantic-release works
8. [x] API documentation is generated
9. [x] E2E tests pass on all platforms (530+ tests)
10. [x] Binary builds for Linux, macOS, Windows (6 targets)
11. [ ] Install script works (future enhancement)
12. [ ] Performance targets met (<100ms version calculation) (future benchmarking)
