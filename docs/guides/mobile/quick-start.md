# Mobile App Quick Start

Get started with Canaveral for mobile app CI/CD in under 5 minutes.

## Installation

```bash
# macOS/Linux
curl -fsSL https://get.canaveral.dev | sh

# Add to PATH (or restart terminal)
export PATH="$HOME/.canaveral/bin:$PATH"
```

## Verify Installation

```bash
canaveral --version
canaveral doctor  # Check prerequisites
```

## Project Setup

### 1. Initialize Configuration

Navigate to your mobile project:

```bash
cd my-app
canaveral init
```

This creates `canaveral.toml` with auto-detected settings.

### 2. Review Configuration

```toml
# canaveral.toml
[project]
name = "MyApp"
# Framework auto-detected: flutter, expo, react-native, native-ios, native-android, tauri

[ios]
bundle_id = "com.example.myapp"
team_id = "YOUR_TEAM_ID"

[android]
package_name = "com.example.myapp"
```

## Basic Commands

### Build

```bash
# Build for iOS
canaveral build --platform ios --profile debug
canaveral build --platform ios --profile release

# Build for Android
canaveral build --platform android --profile debug
canaveral build --platform android --profile release
```

### Test

```bash
# Run unit tests
canaveral test --type unit

# Run integration tests
canaveral test --type integration

# With coverage
canaveral test --type unit --coverage
```

### Version Management

```bash
# Show current version
canaveral version

# Bump version
canaveral version --bump patch  # 1.0.0 -> 1.0.1
canaveral version --bump minor  # 1.0.0 -> 1.1.0
canaveral version --bump major  # 1.0.0 -> 2.0.0

# Set specific version
canaveral version --set 2.0.0

# Also bump build number
canaveral version --bump patch --build
```

## Certificate Management (iOS)

### Initialize Certificate Storage

```bash
# Using Git storage
canaveral match init --storage git --git-url git@github.com:org/certs.git --team-id TEAMID

# Using S3
canaveral match init --storage s3 --bucket my-certs-bucket --team-id TEAMID
```

### Sync Certificates

```bash
# Development certificates
canaveral match sync --profile-type development

# App Store certificates
canaveral match sync --profile-type appstore

# Read-only mode (for CI)
canaveral match sync --profile-type appstore --readonly
```

## Distribution

### TestFlight (iOS)

```bash
# Upload to TestFlight
canaveral testflight upload build/ios/App.ipa

# With changelog
canaveral testflight upload build/ios/App.ipa --changelog "Bug fixes"

# Check processing status
canaveral testflight status
```

### Google Play (Android)

```bash
# Upload to internal track
canaveral store upload android --artifact build/android/app.aab --track internal

# Upload to production
canaveral store upload android --artifact build/android/app.aab --track production
```

### Firebase App Distribution

```bash
# Upload to Firebase
canaveral firebase upload build/android/app.apk --app APP_ID --groups "testers"
```

## CI/CD Integration

### GitHub Actions

Create `.github/workflows/build.yml`:

```yaml
name: Build
on: [push]

jobs:
  build-ios:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4

      - name: Install Canaveral
        run: |
          curl -fsSL https://get.canaveral.dev | sh
          echo "$HOME/.canaveral/bin" >> $GITHUB_PATH

      - name: Sync Certificates
        run: canaveral match sync --profile-type appstore --readonly
        env:
          MATCH_GIT_URL: ${{ vars.MATCH_GIT_URL }}
          MATCH_PASSWORD: ${{ secrets.MATCH_PASSWORD }}

      - name: Build iOS
        run: canaveral build --platform ios --profile release --output-format github-actions

      - name: Upload to TestFlight
        run: canaveral testflight upload build/ios/*.ipa
        env:
          APP_STORE_CONNECT_API_KEY: ${{ secrets.APP_STORE_CONNECT_API_KEY }}

  build-android:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Canaveral
        run: |
          curl -fsSL https://get.canaveral.dev | sh
          echo "$HOME/.canaveral/bin" >> $GITHUB_PATH

      - name: Setup Keystore
        run: echo "${{ secrets.ANDROID_KEYSTORE_BASE64 }}" | base64 -d > android/app/release.keystore

      - name: Build Android
        run: canaveral build --platform android --profile release --output-format github-actions
        env:
          ANDROID_KEYSTORE_PASSWORD: ${{ secrets.ANDROID_KEYSTORE_PASSWORD }}
          ANDROID_KEY_ALIAS: ${{ secrets.ANDROID_KEY_ALIAS }}
          ANDROID_KEY_PASSWORD: ${{ secrets.ANDROID_KEY_PASSWORD }}

      - name: Upload to Play Store
        run: canaveral store upload android --artifact build/android/*.aab --track internal
        env:
          GOOGLE_PLAY_SERVICE_ACCOUNT: ${{ secrets.GOOGLE_PLAY_SERVICE_ACCOUNT }}
```

### GitLab CI

Create `.gitlab-ci.yml`:

```yaml
stages:
  - build
  - deploy

build-ios:
  stage: build
  tags: [macos]
  script:
    - curl -fsSL https://get.canaveral.dev | sh
    - export PATH="$HOME/.canaveral/bin:$PATH"
    - canaveral match sync --profile-type appstore --readonly
    - canaveral build --platform ios --profile release --output-format gitlab-ci
  artifacts:
    paths:
      - build/ios/*.ipa

build-android:
  stage: build
  image: cimg/android:2024.01
  script:
    - curl -fsSL https://get.canaveral.dev | sh
    - export PATH="$HOME/.canaveral/bin:$PATH"
    - canaveral build --platform android --profile release --output-format gitlab-ci
  artifacts:
    paths:
      - build/android/*.aab
```

## Supported Frameworks

Canaveral auto-detects your framework:

| Framework | Detection | iOS | Android | Desktop |
|-----------|-----------|-----|---------|---------|
| Flutter | `pubspec.yaml` | ✓ | ✓ | ✓ |
| Expo | `app.json` with expo | ✓ | ✓ | - |
| React Native | `package.json` with react-native | ✓ | ✓ | - |
| Native iOS | `.xcodeproj`/`.xcworkspace` | ✓ | - | - |
| Native Android | `build.gradle` | - | ✓ | - |
| Tauri | `tauri.conf.json` | - | - | ✓ |

## Environment Variables

### iOS / App Store

| Variable | Description |
|----------|-------------|
| `MATCH_GIT_URL` | Git URL for certificate storage |
| `MATCH_PASSWORD` | Password for certificate decryption |
| `ASC_API_KEY_ID` | App Store Connect API key ID |
| `ASC_API_KEY_ISSUER` | App Store Connect API key issuer |
| `ASC_API_KEY_PATH` | Path to App Store Connect API key file |
| `CANAVERAL_TEAM_ID` | Apple Developer Team ID |
| `CANAVERAL_BUNDLE_ID` | iOS bundle identifier |

### Android / Play Store

| Variable | Description |
|----------|-------------|
| `GOOGLE_PLAY_SERVICE_ACCOUNT` | Path to service account JSON |
| `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` | Base64-encoded service account |
| `ANDROID_KEYSTORE_PATH` | Path to Android keystore |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Signing key alias |
| `ANDROID_KEY_PASSWORD` | Signing key password |

### Firebase

| Variable | Description |
|----------|-------------|
| `FIREBASE_TOKEN` | Firebase CLI token |
| `GOOGLE_APPLICATION_CREDENTIALS` | Path to Firebase service account |

## Next Steps

- [Migration from Fastlane](./migration-from-fastlane.md) - If you're coming from fastlane
- [CI/CD Templates](../../../templates/README.md) - Pre-built workflow templates
- [Screenshot Automation](./screenshots.md) - Capture app store screenshots
- [Metadata Management](./metadata.md) - Manage app store metadata

## Troubleshooting

Run the doctor command to diagnose issues:

```bash
canaveral doctor
```

Common fixes:
- **Missing Xcode**: `xcode-select --install`
- **Missing Android SDK**: Set `ANDROID_HOME` environment variable
- **Missing Flutter**: Install from https://flutter.dev
- **Certificate issues**: Run `canaveral match sync --force`
