# Plan: Signing and App Store Support

## Overview

Add comprehensive code signing and app store upload capabilities to Canaveral, including a secure team credential management system for sharing signing identities across development teams.

---

## Part 1: Code Signing Infrastructure

### 1.1 New Crate: `canaveral-signing`

Create a dedicated crate for signing operations:

```
canaveral-signing/
├── src/
│   ├── lib.rs
│   ├── identity.rs       # Signing identity abstraction
│   ├── keychain.rs       # macOS Keychain integration
│   ├── certificate.rs    # Certificate parsing/validation
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── macos.rs      # codesign, productsign
│   │   ├── windows.rs    # signtool, SignTool.exe
│   │   ├── android.rs    # apksigner, jarsigner
│   │   └── gpg.rs        # GPG signing (enhance existing)
│   └── team/
│       ├── mod.rs
│       ├── vault.rs      # Encrypted credential storage
│       ├── sync.rs       # Team credential synchronization
│       └── roles.rs      # Access control
```

### 1.2 Signing Identity Abstraction

```rust
pub trait SigningProvider: Send + Sync {
    fn name(&self) -> &str;
    fn sign(&self, artifact: &Path, identity: &SigningIdentity, options: &SignOptions) -> Result<()>;
    fn verify(&self, artifact: &Path) -> Result<SignatureInfo>;
    fn list_identities(&self) -> Result<Vec<SigningIdentity>>;
}

pub struct SigningIdentity {
    pub id: String,
    pub name: String,
    pub provider: SigningProviderType,
    pub fingerprint: Option<String>,
    pub expires: Option<DateTime<Utc>>,
    pub team_id: Option<String>,
}
```

### 1.3 Platform-Specific Signing

**macOS/iOS:**
- `codesign` for apps, frameworks, binaries
- `productsign` for installer packages
- Hardened runtime support
- Entitlements management
- Provisioning profile handling

**Windows:**
- `signtool.exe` for EXE, DLL, MSI
- Authenticode signing
- Timestamp server support
- EV certificate support

**Android:**
- `apksigner` for APK/AAB signing
- V1/V2/V3/V4 signature schemes
- Key rotation support

---

## Part 2: App Store Adapters

### 2.1 New Adapter Type: `StoreAdapter`

```rust
pub trait StoreAdapter: Send + Sync {
    fn name(&self) -> &str;
    fn validate_artifact(&self, path: &Path) -> Result<ValidationResult>;
    fn upload(&self, path: &Path, options: &UploadOptions) -> Result<UploadResult>;
    fn get_status(&self, build_id: &str) -> Result<BuildStatus>;
    fn list_builds(&self, app_id: &str) -> Result<Vec<Build>>;
}
```

### 2.2 Supported Stores

**Apple App Store (macOS/iOS):**
- Integration with `altool` / `notarytool` / Transporter
- App Store Connect API (JWT auth)
- Notarization workflow
- TestFlight upload
- Metadata management

**Google Play Store:**
- Google Play Developer API
- Service account authentication
- Track management (internal/alpha/beta/production)
- Release notes per locale

**Microsoft Store:**
- Windows Store submission API
- Partner Center integration
- Package flights

**Other Stores (Future):**
- F-Droid (open source Android)
- Steam (games)
- Snapcraft / Flathub (Linux)

### 2.3 Configuration Schema

```yaml
signing:
  enabled: true
  provider: macos  # macos | windows | android | gpg
  identity: "Developer ID Application: Company (TEAMID)"

  macos:
    hardened_runtime: true
    entitlements: ./entitlements.plist
    timestamp: true

  windows:
    timestamp_url: http://timestamp.digicert.com
    algorithm: sha256

  android:
    keystore: ${ANDROID_KEYSTORE_PATH}
    key_alias: release

stores:
  - type: apple
    app_id: com.company.app
    team_id: ${APPLE_TEAM_ID}
    api_key: ${APP_STORE_CONNECT_KEY_ID}
    api_issuer: ${APP_STORE_CONNECT_ISSUER}
    notarize: true

  - type: google_play
    package_name: com.company.app
    service_account: ${GOOGLE_PLAY_SERVICE_ACCOUNT}
    track: internal

  - type: microsoft
    app_id: ${MS_STORE_APP_ID}
    tenant_id: ${MS_TENANT_ID}
```

---

## Part 3: Team Signing System

### 3.1 Goals

- Securely share signing credentials across team members
- Support different access levels (admin, signer, viewer)
- Work offline with periodic sync
- Audit trail for signing operations
- No plaintext credentials in repos or CI

### 3.2 Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    Team Vault                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  Encrypted  │  │   Access    │  │   Audit     │     │
│  │ Credentials │  │   Control   │  │    Log      │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
           │                    │
           ▼                    ▼
    ┌─────────────┐      ┌─────────────┐
    │   Local     │      │     CI      │
    │  Developer  │      │  Pipeline   │
    └─────────────┘      └─────────────┘
```

### 3.3 Credential Storage Options

**Option A: Git-based Encrypted Vault (Recommended)**
- Encrypted JSON/YAML files in repo (`.canaveral/signing/`)
- Age or GPG encryption with team member public keys
- Works with existing git workflows
- Offline-first approach

**Option B: External Secrets Manager Integration**
- HashiCorp Vault
- AWS Secrets Manager
- Azure Key Vault
- 1Password CLI
- Doppler

**Option C: Hybrid**
- Metadata in git (identities, access control)
- Actual secrets in external store or CI secrets

### 3.4 Team Vault Schema

```yaml
# .canaveral/signing/vault.yaml (encrypted)
version: 1
team:
  name: "MyCompany"
  id: "uuid-here"

members:
  - id: "user-uuid"
    email: "dev@company.com"
    public_key: "age1..."
    role: admin  # admin | signer | viewer

identities:
  - id: "apple-dist"
    name: "Apple Distribution"
    type: macos
    allowed_roles: [admin, signer]
    # Encrypted blob containing actual certificate/key
    encrypted_data: "AGE-ENCRYPTED-..."

  - id: "android-release"
    name: "Android Release Key"
    type: android
    allowed_roles: [admin, signer]
    encrypted_data: "AGE-ENCRYPTED-..."

audit:
  - timestamp: "2024-01-15T10:30:00Z"
    user: "dev@company.com"
    action: "sign"
    identity: "apple-dist"
    artifact: "MyApp-1.2.0.dmg"
```

### 3.5 CLI Commands

```bash
# Initialize team signing
canaveral signing init --team "MyCompany"

# Add team member
canaveral signing member add --email dev@company.com --role signer

# Import signing identity
canaveral signing identity import ./certificate.p12 --name "Apple Distribution"

# List available identities
canaveral signing identity list

# Sign an artifact
canaveral signing sign ./build/MyApp.app --identity apple-dist

# Sync credentials (pull updates)
canaveral signing sync

# Audit log
canaveral signing audit --last 30d
```

---

## Part 4: CI/CD Integration

### 4.1 GitHub Actions

```yaml
# .github/workflows/release.yml
jobs:
  release:
    runs-on: macos-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Canaveral
        uses: canaveral/setup-action@v1

      - name: Import Signing Credentials
        run: |
          canaveral signing import-ci \
            --apple-cert "${{ secrets.APPLE_CERTIFICATE }}" \
            --apple-cert-password "${{ secrets.APPLE_CERT_PASSWORD }}"

      - name: Build and Sign
        run: canaveral release --sign --upload
        env:
          APP_STORE_CONNECT_KEY: ${{ secrets.ASC_KEY }}
```

### 4.2 Secure CI Credential Handling

- Ephemeral keychain creation on macOS
- Automatic cleanup after signing
- No credential persistence between runs
- Support for base64-encoded secrets

---

## Part 5: Implementation Phases

### Phase 1: Signing Foundation (Core Infrastructure) ✅
- [x] Create `canaveral-signing` crate
- [x] Implement `SigningProvider` trait
- [x] macOS `codesign` provider
- [x] GPG provider
- [x] Android `apksigner` provider
- [x] Windows `signtool` provider (stub for cross-platform)
- [x] Basic CLI commands (`signing list`, `signing sign`, `signing verify`, `signing info`)
- [x] Configuration schema for signing

### Phase 2: Team Vault (Credential Management) ✅
- [x] Encrypted vault storage format
- [x] Age encryption integration
- [x] Member management (add/remove/roles)
- [x] Identity import/export
- [x] Audit logging
- [x] CLI commands for team management

### Phase 3: App Store Adapters ✅
- [x] `StoreAdapter` trait
- [x] Apple notarization workflow
- [x] App Store Connect API integration
- [x] Google Play API integration
- [x] Upload CLI commands

### Phase 4: Extended Platform Support
- [ ] Windows signtool provider
- [ ] Android apksigner provider
- [x] Microsoft Store adapter
- [ ] Additional signing providers as needed

### Phase 5: CI/CD & Polish
- [ ] GitHub Actions integration examples
- [ ] Ephemeral keychain management
- [ ] External secrets manager integration
- [ ] Documentation and guides
- [ ] Migration guide from fastlane match

---

## Part 6: Security Considerations

### 6.1 Threat Model

- **Credential theft**: Encrypt at rest, minimal decryption window
- **Unauthorized signing**: Role-based access, audit logging
- **CI compromise**: Short-lived credentials, ephemeral storage
- **Key rotation**: Support for identity versioning and rotation

### 6.2 Best Practices Enforced

- Never store plaintext credentials in config
- Require encryption for team vault
- Automatic credential cleanup in CI
- Certificate expiration warnings
- Signature verification as default

---

## Part 6: File Structure After Implementation

```
canaveral/
├── canaveral-signing/          # NEW
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── identity.rs
│       ├── certificate.rs
│       ├── providers/
│       │   ├── mod.rs
│       │   ├── macos.rs
│       │   ├── windows.rs
│       │   ├── android.rs
│       │   └── gpg.rs
│       └── team/
│           ├── mod.rs
│           ├── vault.rs
│           ├── encryption.rs
│           └── roles.rs
├── canaveral-stores/           # NEW
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── traits.rs
│       ├── apple/
│       │   ├── mod.rs
│       │   ├── notarize.rs
│       │   └── connect.rs
│       ├── google_play/
│       │   └── mod.rs
│       └── microsoft/
│           └── mod.rs
├── canaveral/
│   └── src/cli/commands/
│       ├── signing.rs          # NEW
│       └── store.rs            # NEW
└── docs/
    └── guides/
        ├── signing.md          # NEW
        ├── team-signing.md     # NEW
        └── app-stores.md       # NEW
```

---

## Appendix: Prior Art & Inspiration

- **fastlane match**: Team code signing for iOS, stores in git/S3/GCS
- **goreleaser**: Has signing support via GPG/cosign
- **electron-builder**: Code signing for Electron apps
- **App Store Connect API**: Apple's REST API for store operations
- **Google Play Developer API**: Android publishing API
