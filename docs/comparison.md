# Comparison with Existing Tools

## Feature Matrix

| Feature | Canaveral | semantic-release | release-please | release-plz |
|---------|-----------|------------------|----------------|-------------|
| **Ecosystems** |
| npm | ✅ | ✅ | ✅ | ❌ |
| Cargo | ✅ | ❌ | ✅ | ✅ |
| Python | ✅ | ❌ | ✅ | ❌ |
| Go | ✅ | ❌ | ✅ | ❌ |
| Maven | ✅ | ❌ | ✅ | ❌ |
| Docker | ✅ | ✅ (plugin) | ❌ | ❌ |
| **Versioning** |
| SemVer | ✅ | ✅ | ✅ | ✅ |
| CalVer | ✅ | ❌ | ❌ | ❌ |
| Build Numbers | ✅ | ❌ | ❌ | ❌ |
| Custom | ✅ (plugin) | ❌ | ❌ | ❌ |
| **Monorepo** |
| Independent versioning | ✅ | ❌ | ✅ | ✅ |
| Fixed versioning | ✅ | ❌ | ✅ | ❌ |
| Change detection | ✅ | ❌ | ✅ | ✅ |
| Dependency graph | ✅ | ❌ | ❌ | ✅ |
| **Workflow** |
| Local CLI | ✅ | ✅ | ⚠️ Limited | ✅ |
| CI-first | ✅ | ✅ | ✅ | ✅ |
| Dry-run mode | ✅ | ✅ | ❌ | ✅ |
| Rollback support | ✅ | ❌ | ❌ | ❌ |
| **Extensibility** |
| Plugin system | ✅ | ✅ | ⚠️ Manifest | ❌ |
| Hooks | ✅ | ✅ | ❌ | ❌ |
| Custom templates | ✅ | ✅ | ✅ | ❌ |

## Tool Comparison

### semantic-release

**Strengths:**
- Mature and battle-tested
- Strong plugin ecosystem
- Excellent npm support
- Good CI integration

**Weaknesses:**
- npm-focused, limited other ecosystem support
- Complex plugin configuration
- No built-in monorepo support
- Heavy node_modules dependency tree

**When to use semantic-release:**
- Single npm package projects
- Already invested in the plugin ecosystem
- Need specific plugins not available in Canaveral

**Migrating to Canaveral:**
```bash
canaveral migrate --from semantic-release
```

---

### release-please

**Strengths:**
- Good GitHub integration
- Multi-language support
- Release PR workflow
- Google-backed maintenance

**Weaknesses:**
- GitHub-centric (limited local CLI)
- Opinionated workflow (release PRs)
- Limited customization
- Complex configuration for advanced use cases

**When to use release-please:**
- GitHub-heavy workflow
- Prefer release PR approach
- Need Google-supported tool

**Migrating to Canaveral:**
```bash
canaveral migrate --from release-please
```

---

### release-plz

**Strengths:**
- Rust-native performance
- Excellent Cargo workspace support
- Clean, simple design
- Good changelog generation

**Weaknesses:**
- Rust/Cargo only
- No plugin system
- Limited versioning strategies
- Smaller community

**When to use release-plz:**
- Pure Rust projects
- Want minimal tooling
- Cargo workspace optimization

**Migrating to Canaveral:**
```bash
canaveral migrate --from release-plz
```

---

### Lerna / Changesets

**Strengths:**
- npm monorepo specialists
- Well-established in JS ecosystem
- Good tooling integration

**Weaknesses:**
- JavaScript/npm only
- Lerna maintenance concerns
- Changesets requires manual changelog files

**When to use Lerna/Changesets:**
- Large existing npm monorepo
- Already invested in these tools
- Team familiar with the workflow

---

## Canaveral Advantages

### 1. Universal Ecosystem Support

Single tool for all your projects:
```yaml
# Same config structure for any ecosystem
packages:
  - type: npm
    path: ./web
  - type: cargo
    path: ./core
  - type: python
    path: ./scripts
```

### 2. Flexible Versioning

Not locked into SemVer:
```yaml
# Use CalVer for apps
strategy: calver
calver:
  format: "YYYY.MM.MICRO"

# Or build numbers for mobile
strategy: buildnum
buildnum:
  format: "SEMVER.BUILD"
```

### 3. True Local CLI

Run releases locally, not just in CI:
```bash
# Preview locally
canaveral release --dry-run

# Release from your machine
canaveral release
```

### 4. Monorepo-First Design

Native support for complex monorepos:
```bash
# Release only changed packages
canaveral release --changed

# Respects dependency order
canaveral publish
```

### 5. Extensibility

Add support for anything via external subprocess plugins (any language):

```yaml
# canaveral.yaml
plugins:
  - name: my-custom-adapter
    plugin_type: adapter
    command: /usr/local/bin/my-adapter-plugin
    enabled: true
```

Plugins communicate via JSON over stdin/stdout, so they can be written in any language. See the [Plugin System](./architecture/plugins.md) documentation.

## Migration Guides

- [From semantic-release](./migration/from-semantic-release.md)
- [From release-please](./migration/from-release-please.md)
- [From release-plz](./migration/from-release-plz.md)
- [From Lerna](./migration/from-lerna.md)
- [From Changesets](./migration/from-changesets.md)
