# Phase 5: Polish & Extensibility

**Goal**: Enable community extensions and provide production-ready tooling.

## Tasks

### 5.1 Plugin System Implementation

- [ ] Plugin discovery from multiple sources
- [ ] Dynamic library loading (.so, .dylib, .dll)
- [ ] WASM plugin runtime (optional)
- [ ] Plugin configuration and lifecycle
- [ ] Error isolation

```rust
// crates/canaveral-core/src/plugins/loader.rs
use libloading::{Library, Symbol};
use std::path::Path;

pub struct PluginLoader {
    plugins: Vec<LoadedPlugin>,
}

struct LoadedPlugin {
    name: String,
    library: Library,
    plugin: Box<dyn Plugin>,
}

impl PluginLoader {
    pub fn discover(&mut self, search_paths: &[PathBuf]) -> Result<()> {
        for path in search_paths {
            if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if self.is_plugin_library(&path) {
                        self.load_plugin(&path)?;
                    }
                }
            }
        }
        Ok(())
    }

    fn load_plugin(&mut self, path: &Path) -> Result<()> {
        unsafe {
            let library = Library::new(path)?;

            // Get plugin entry point
            let create_plugin: Symbol<fn() -> Box<dyn Plugin>> =
                library.get(b"create_plugin")?;

            let plugin = create_plugin();
            let name = plugin.name().to_string();

            self.plugins.push(LoadedPlugin {
                name,
                library,
                plugin,
            });
        }
        Ok(())
    }

    fn is_plugin_library(&self, path: &Path) -> bool {
        let ext = path.extension().and_then(|e| e.to_str());
        matches!(ext, Some("so") | Some("dylib") | Some("dll"))
    }
}

// Plugin trait that all plugins must implement
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn plugin_type(&self) -> PluginType;
    fn init(&mut self, config: serde_json::Value) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
}

#[derive(Debug, Clone, Copy)]
pub enum PluginType {
    Strategy,
    Adapter,
    Changelog,
    Hook,
}
```

### 5.2 Hook System

- [ ] Shell command execution
- [ ] Script hooks
- [ ] Environment variable passing
- [ ] Timeout handling
- [ ] Error propagation

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

- [ ] GitHub Actions
- [ ] GitLab CI
- [ ] General CI documentation

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
          curl -fsSL https://canaveral.dev/install.sh | sh
        else
          curl -fsSL https://canaveral.dev/install.sh | sh -s -- --version ${{ inputs.version }}
        fi

    - name: Run Canaveral
      shell: bash
      run: |
        canaveral ${{ inputs.command }} \
          ${{ inputs.dry-run == 'true' && '--dry-run' || '' }}
```

### 5.4 Migration Tools

- [ ] Analyze existing configs
- [ ] Generate Canaveral config
- [ ] Show differences

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

- [ ] API documentation (rustdoc)
- [ ] User guides
- [ ] E2E test suite
- [ ] Performance benchmarks

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

1. [ ] Plugins can be discovered and loaded
2. [ ] Custom strategy plugins work
3. [ ] Custom adapter plugins work
4. [ ] All hook stages execute correctly
5. [ ] GitHub Action is published
6. [ ] GitLab CI template works
7. [ ] Migration from semantic-release works
8. [ ] API documentation is generated
9. [ ] E2E tests pass on all platforms
10. [ ] Binary builds for Linux, macOS, Windows
11. [ ] Install script works
12. [ ] Performance targets met (<100ms version calculation)
