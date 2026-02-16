# Package Adapters

Package adapters handle the ecosystem-specific details of reading manifests, updating versions, and publishing packages.

## Supported Ecosystems

| Ecosystem | Manifest | Registry | Status |
|-----------|----------|----------|--------|
| npm | package.json | npmjs.com | Phase 2 |
| Cargo | Cargo.toml | crates.io | Phase 2 |
| Python | pyproject.toml | PyPI | Phase 2 |
| Go | go.mod | Git tags | Phase 4 |
| Maven | pom.xml | Maven Central | Phase 4 |
| Docker | Dockerfile | Docker Hub, ghcr.io | Phase 4 |
| NuGet | *.csproj | nuget.org | Future |
| Gradle | build.gradle | Maven Central | Future |

## Adapter Interface

All adapters implement the `PackageAdapter` trait (synchronous, from `canaveral-adapters/src/traits.rs`):

```rust
pub trait PackageAdapter: Send + Sync {
    /// Get the adapter name (e.g., "npm", "cargo")
    fn name(&self) -> &'static str;

    /// Get the default registry URL for this adapter
    fn default_registry(&self) -> &'static str;

    /// Check if this adapter applies to the given path
    fn detect(&self, path: &Path) -> bool;

    /// Get package information from manifest
    fn get_info(&self, path: &Path) -> Result<PackageInfo>;

    /// Get current version
    fn get_version(&self, path: &Path) -> Result<String>;

    /// Set version in manifest
    fn set_version(&self, path: &Path, version: &str) -> Result<()>;

    /// Publish package (simple version)
    fn publish(&self, path: &Path, dry_run: bool) -> Result<()>;

    /// Publish package with detailed options
    fn publish_with_options(&self, path: &Path, options: &PublishOptions) -> Result<()>;

    /// Validate that the package can be published
    fn validate_publishable(&self, path: &Path) -> Result<ValidationResult>;

    /// Check if authentication is configured for publishing
    fn check_auth(&self, credentials: &mut CredentialProvider) -> Result<bool>;

    /// Get the manifest filename(s) this adapter handles
    fn manifest_names(&self) -> &[&str];

    /// Build the package (if applicable)
    fn build(&self, path: &Path) -> Result<()>;

    /// Run tests (if applicable)
    fn test(&self, path: &Path) -> Result<()>;

    /// Clean build artifacts (if applicable)
    fn clean(&self, path: &Path) -> Result<()>;

    /// Pack the package for publishing without actually publishing
    fn pack(&self, path: &Path) -> Result<Option<PathBuf>>;
}
```

Note: The trait is synchronous (not async). Several methods like `build`, `test`, `clean`, and `pack` have default no-op implementations.

## npm Adapter

### Detection

Looks for `package.json` in the project root or specified path.

### Manifest Operations

**Read version:**
```json
{
  "name": "@myorg/package",
  "version": "1.2.3"
}
```

**Write version:** Updates `version` field while preserving formatting.

**Dependencies:**
- `dependencies`
- `devDependencies`
- `peerDependencies`
- `optionalDependencies`

### Publishing

```bash
npm publish [--tag <tag>] [--access <public|restricted>]
```

**Options:**
| Option | Description |
|--------|-------------|
| `--tag` | Registry tag (default: latest) |
| `--access` | Access level for scoped packages |
| `--registry` | Custom registry URL |
| `--otp` | One-time password for 2FA |

### Authentication

**Methods (priority order):**
1. `NPM_TOKEN` environment variable
2. `--token` CLI flag
3. `.npmrc` file in project or home directory
4. `npm login` session

**Token format:**
```
//registry.npmjs.org/:_authToken=${NPM_TOKEN}
```

### Configuration

```yaml
packages:
  - type: npm
    path: .
    registry: https://registry.npmjs.org
    access: public
    tag: latest
```

---

## Cargo Adapter

### Detection

Looks for `Cargo.toml` with `[package]` section.

### Manifest Operations

**Read version:**
```toml
[package]
name = "my-crate"
version = "1.2.3"
```

**Write version:** Updates version in `[package]` section.

**Workspace handling:**
```toml
[workspace]
members = ["crates/*"]
```

### Publishing

```bash
cargo publish [--dry-run] [--no-verify]
```

**Pre-publish checks:**
- Runs `cargo package` to verify
- Checks `publish = false` setting
- Validates `crates.io` name availability

### Authentication

**Methods:**
1. `CARGO_REGISTRY_TOKEN` environment variable
2. `cargo login` stored token (`~/.cargo/credentials.toml`)

### Configuration

```yaml
packages:
  - type: cargo
    path: .
    registry: crates.io
    # Skip verification (not recommended)
    verify: true
```

---

## Python Adapter

### Detection

Looks for (in order):
1. `pyproject.toml` with `[project]` section
2. `setup.py`
3. `setup.cfg`

### Manifest Operations

**pyproject.toml (PEP 621):**
```toml
[project]
name = "my-package"
version = "1.2.3"
```

**setup.py:**
```python
setup(
    name="my-package",
    version="1.2.3",
)
```

**Dynamic version:**
```toml
[project]
dynamic = ["version"]

[tool.setuptools.dynamic]
version = {attr = "mypackage.__version__"}
```

### Publishing

**Build:**
```bash
python -m build
# Creates dist/my-package-1.2.3.tar.gz
# Creates dist/my_package-1.2.3-py3-none-any.whl
```

**Upload:**
```bash
twine upload dist/*
```

### Authentication

**Methods:**
1. `TWINE_USERNAME` + `TWINE_PASSWORD` environment variables
2. `TWINE_TOKEN` (PyPI API token)
3. `~/.pypirc` file
4. Keyring integration

**pypirc format:**
```ini
[pypi]
username = __token__
password = pypi-xxxxxxxxxxxxx
```

### Configuration

```yaml
packages:
  - type: python
    path: .
    registry: https://upload.pypi.org/legacy/
    # Build system
    buildBackend: setuptools  # or: poetry, flit, hatch
```

---

## Go Adapter

### Detection

Looks for `go.mod` file.

### Manifest Operations

**Read version:** From git tags (Go convention)
```
v1.2.3
```

**Module path:**
```go
module github.com/org/repo
```

**Major version path:** For v2+:
```go
module github.com/org/repo/v2
```

### Publishing

Go modules are "published" via git tags. No registry upload needed.

```bash
git tag v1.2.3
git push origin v1.2.3
```

**Post-publish:** Module becomes available on `pkg.go.dev` automatically.

### Authentication

No registry authentication. Uses git authentication for private repos.

### Configuration

```yaml
packages:
  - type: go
    path: .
    # Tag prefix (default: v)
    tagPrefix: v
```

---

## Maven Adapter

### Detection

Looks for `pom.xml` file.

### Manifest Operations

**Read version:**
```xml
<project>
  <groupId>com.example</groupId>
  <artifactId>my-lib</artifactId>
  <version>1.2.3</version>
</project>
```

**Parent POM:**
```xml
<parent>
  <groupId>com.example</groupId>
  <artifactId>parent</artifactId>
  <version>1.0.0</version>
</parent>
```

### Publishing

```bash
mvn deploy
```

**Maven Central requirements:**
- GPG signing
- Javadoc and sources JARs
- Sonatype staging process

### Authentication

**settings.xml:**
```xml
<servers>
  <server>
    <id>ossrh</id>
    <username>${env.MAVEN_USERNAME}</username>
    <password>${env.MAVEN_PASSWORD}</password>
  </server>
</servers>
```

### Configuration

```yaml
packages:
  - type: maven
    path: .
    registry: https://oss.sonatype.org
    # GPG key ID
    gpgKeyId: ABC12345
```

---

## Docker Adapter

### Detection

Looks for `Dockerfile` in project root.

### Manifest Operations

**Version sources:**
1. Build argument: `ARG VERSION=1.2.3`
2. Label: `LABEL version="1.2.3"`
3. External (not in Dockerfile)

### Publishing

**Build:**
```bash
docker build -t image:version .
```

**Push:**
```bash
docker push image:version
```

**Multi-platform:**
```bash
docker buildx build --platform linux/amd64,linux/arm64 --push .
```

### Authentication

**Docker Hub:**
```bash
docker login -u $DOCKER_USER -p $DOCKER_TOKEN
```

**GitHub Container Registry:**
```bash
echo $GITHUB_TOKEN | docker login ghcr.io -u $GITHUB_ACTOR --password-stdin
```

**AWS ECR:**
```bash
aws ecr get-login-password | docker login --password-stdin $ECR_REGISTRY
```

### Configuration

```yaml
packages:
  - type: docker
    path: .
    image: myorg/myimage
    registries:
      - docker.io
      - ghcr.io/myorg
    # Tag strategy
    tags:
      - latest
      - "{{version}}"
      - "{{major}}.{{minor}}"
    # Multi-platform
    platforms:
      - linux/amd64
      - linux/arm64
```

---

## Custom Adapters

Custom adapters can be created via the external subprocess plugin system. Adapter plugins communicate with Canaveral via JSON over stdin/stdout.

See the [Plugin System](../architecture/plugins.md) documentation for the full protocol specification.

**Example: Custom adapter plugin (Python)**

```python
#!/usr/bin/env python3
import json, sys, os

def handle_request(request):
    action = request["action"]
    input_data = request.get("input", {})

    if action == "info":
        return {"output": {
            "name": "my-adapter",
            "version": "1.0.0",
            "plugin_type": "adapter",
            "capabilities": ["detect", "get_version", "set_version", "publish"]
        }}
    elif action == "detect":
        path = input_data.get("path", "")
        return {"output": os.path.exists(os.path.join(path, "my-manifest.json"))}
    elif action == "get_version":
        path = input_data.get("path", "")
        manifest = json.load(open(os.path.join(path, "my-manifest.json")))
        return {"output": manifest.get("version", "0.0.0")}
    else:
        return {"error": f"Unknown action: {action}"}

request = json.load(sys.stdin)
print(json.dumps(handle_request(request)))
```

**Register in config:**
```yaml
plugins:
  - name: my-custom-adapter
    plugin_type: adapter
    command: /usr/local/bin/my-adapter-plugin
    enabled: true
```
