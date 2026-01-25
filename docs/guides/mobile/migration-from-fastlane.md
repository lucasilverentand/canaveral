# Migrating from Fastlane to Canaveral

This guide helps you migrate from fastlane to Canaveral for mobile app CI/CD. Canaveral provides the same capabilities with a framework-agnostic approach that works across Flutter, React Native, Expo, native iOS/Android, and Tauri.

## Quick Comparison

| Fastlane | Canaveral | Notes |
|----------|-----------|-------|
| `Fastfile` (Ruby DSL) | `canaveral.toml` | Declarative config instead of Ruby |
| `gym` | `canaveral build --platform ios` | Auto-detects framework |
| `gradle` | `canaveral build --platform android` | Works with any Android framework |
| `pilot` | `canaveral testflight upload` | TestFlight uploads |
| `deliver` | `canaveral upload ios` | App Store submission |
| `supply` | `canaveral upload android` | Google Play submission |
| `match` | `canaveral match` | Certificate sync |
| `snapshot` | `canaveral screenshots capture` | Screenshot automation |
| `scan` | `canaveral test` | Test running |
| `frameit` | `canaveral screenshots frame` | Screenshot framing |
| `pem` | (built into match) | APNs cert handling |
| `sigh` | (built into match) | Profile management |

## Step 1: Install Canaveral

```bash
# macOS/Linux
curl -fsSL https://get.canaveral.dev | sh

# Or with cargo
cargo install canaveral
```

## Step 2: Create Configuration

Create `canaveral.toml` in your project root:

```toml
[project]
name = "MyApp"
# Framework is auto-detected, but you can specify it
# framework = "flutter"  # flutter, expo, react-native, native-ios, native-android, tauri

[ios]
bundle_id = "com.example.myapp"
team_id = "TEAMID123"

[android]
package_name = "com.example.myapp"

[build]
# Output formats: text, json, github-actions, gitlab-ci
output_format = "text"

[signing]
# Certificate storage: git, s3, gcs, azure
storage = "git"
git_url = "git@github.com:org/certs.git"

[testflight]
# App Store Connect API key (from environment)
api_key_id = "${ASC_API_KEY_ID}"
api_key_issuer = "${ASC_API_KEY_ISSUER}"
api_key_path = "${ASC_API_KEY_PATH}"

[play_store]
# Service account for Google Play
service_account_path = "${GOOGLE_PLAY_SERVICE_ACCOUNT}"
```

## Step 3: Migrate Common Lanes

### Building iOS

**Fastlane:**
```ruby
lane :build_ios do
  gym(
    scheme: "MyApp",
    export_method: "app-store",
    output_directory: "build",
    output_name: "MyApp.ipa"
  )
end
```

**Canaveral:**
```bash
canaveral build --platform ios --profile release
```

### Building Android

**Fastlane:**
```ruby
lane :build_android do
  gradle(
    task: "bundle",
    build_type: "Release",
    project_dir: "android/"
  )
end
```

**Canaveral:**
```bash
canaveral build --platform android --profile release
```

### TestFlight Upload

**Fastlane:**
```ruby
lane :beta do
  build_ios
  pilot(
    skip_waiting_for_build_processing: true,
    changelog: "Bug fixes and improvements"
  )
end
```

**Canaveral:**
```bash
canaveral build --platform ios --profile release
canaveral testflight upload build/ios/*.ipa --changelog "Bug fixes and improvements"
```

### Play Store Upload

**Fastlane:**
```ruby
lane :deploy_android do
  build_android
  supply(
    track: "internal",
    aab: "android/app/build/outputs/bundle/release/app-release.aab"
  )
end
```

**Canaveral:**
```bash
canaveral build --platform android --profile release
canaveral store upload android --artifact build/android/*.aab --track internal
```

### Certificate Sync (Match)

**Fastlane:**
```ruby
lane :sync_certs do
  match(
    type: "appstore",
    readonly: true,
    git_url: "git@github.com:org/certs.git"
  )
end
```

**Canaveral:**
```bash
# Initialize (first time)
canaveral match init --storage git --git-url git@github.com:org/certs.git

# Sync certificates
canaveral match sync --profile-type appstore --readonly
```

### Screenshots

**Fastlane:**
```ruby
lane :screenshots do
  capture_screenshots
  frame_screenshots(white: true)
end
```

**Canaveral:**
```bash
canaveral screenshots capture --config screenshots.yaml
canaveral screenshots frame --template white
```

### Running Tests

**Fastlane:**
```ruby
lane :test do
  scan(
    scheme: "MyApp",
    output_types: "junit"
  )
end
```

**Canaveral:**
```bash
canaveral test --type unit --output-format junit
```

## Step 4: Migrate CI/CD

### GitHub Actions

**Fastlane:**
```yaml
- run: bundle exec fastlane ios build
```

**Canaveral:**
```yaml
- name: Install Canaveral
  run: curl -fsSL https://get.canaveral.dev | sh

- name: Build iOS
  run: canaveral build --platform ios --output-format github-actions
```

Canaveral's `github-actions` output format automatically sets environment variables and outputs for downstream steps.

### GitLab CI

**Fastlane:**
```yaml
build:
  script:
    - bundle exec fastlane android build
```

**Canaveral:**
```yaml
build:
  script:
    - curl -fsSL https://get.canaveral.dev | sh
    - canaveral build --platform android --output-format gitlab-ci
```

## Step 5: Environment Variables

### Mapping Fastlane Environment Variables

| Fastlane | Canaveral |
|----------|-----------|
| `MATCH_PASSWORD` | `MATCH_PASSWORD` (same) |
| `MATCH_GIT_URL` | `MATCH_GIT_URL` (same) |
| `FASTLANE_USER` | Not needed (uses API keys) |
| `FASTLANE_PASSWORD` | Not needed (uses API keys) |
| `APP_STORE_CONNECT_API_KEY_*` | `ASC_API_KEY_*` |
| `SUPPLY_JSON_KEY` | `GOOGLE_PLAY_SERVICE_ACCOUNT` |

### App Store Connect API Key

Canaveral uses App Store Connect API keys exclusively (no password-based auth):

```bash
# Set environment variables
export ASC_API_KEY_ID="XXXXXXXXXX"
export ASC_API_KEY_ISSUER="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
export ASC_API_KEY_PATH="/path/to/AuthKey_XXXXXXXXXX.p8"
```

### Google Play Service Account

```bash
export GOOGLE_PLAY_SERVICE_ACCOUNT="/path/to/service-account.json"
# Or base64 encoded for CI
export GOOGLE_PLAY_SERVICE_ACCOUNT_JSON="$(base64 -i service-account.json)"
```

## Step 6: Delete Fastlane

Once you've verified Canaveral works:

```bash
# Remove fastlane files
rm -rf fastlane/
rm Gemfile Gemfile.lock
```

## Framework-Specific Notes

### Flutter

Canaveral auto-detects Flutter projects and uses `flutter build` under the hood:

```bash
# Build both platforms
canaveral build --platform ios
canaveral build --platform android
```

### React Native / Expo

For Expo managed workflow:
```bash
# Uses EAS Build if available, falls back to local build
canaveral build --platform ios
```

For bare React Native:
```bash
# Uses native build tools directly
canaveral build --platform ios
```

### Native iOS (Xcode)

```bash
# Auto-detects scheme from .xcworkspace or .xcodeproj
canaveral build --platform ios

# Specify scheme if needed
canaveral build --platform ios --config scheme=MyScheme
```

### Native Android (Gradle)

```bash
# Auto-detects gradle wrapper
canaveral build --platform android

# Specify flavor
canaveral build --platform android --config flavor=production
```

### Tauri

```bash
# Builds for current platform
canaveral build --platform macos  # or windows, linux
```

## Common Migration Issues

### "Framework not detected"

Ensure your project has the expected framework markers:
- **Flutter**: `pubspec.yaml` with `flutter` dependency
- **Expo**: `app.json` with `expo` field
- **React Native**: `package.json` with `react-native` dependency
- **Native iOS**: `.xcodeproj` or `.xcworkspace`
- **Native Android**: `build.gradle` in root or `android/` directory
- **Tauri**: `src-tauri/tauri.conf.json`

### "Certificate not found"

Ensure match is initialized:
```bash
canaveral match init --storage git --git-url git@github.com:org/certs.git
canaveral match sync --profile-type development
```

### "Build failed"

Run the doctor command to check prerequisites:
```bash
canaveral doctor
```

## Getting Help

- Run `canaveral --help` for command reference
- Run `canaveral <command> --help` for command-specific help
- Check [CI/CD Templates](../../../templates/README.md) for ready-to-use workflows
- Report issues at https://github.com/anthropics/canaveral/issues
