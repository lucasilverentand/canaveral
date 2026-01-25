# Supported Frameworks

Canaveral provides framework-agnostic build, test, and release automation. Each framework adapter auto-detects your project type and provides optimized workflows.

## Framework Detection

Canaveral automatically detects your framework based on project markers:

| Framework | Detection Method | Confidence |
|-----------|-----------------|------------|
| Flutter | `pubspec.yaml` with `flutter:` | 95% |
| Expo | `app.json` with `expo` field | 95% |
| React Native | `package.json` with `react-native` | 90% |
| Native iOS | `.xcworkspace` or `.xcodeproj` | 95% |
| Native Android | `build.gradle` with `android` plugin | 95% |
| Tauri | `src-tauri/tauri.conf.json` | 95% |

To manually specify a framework:

```bash
canaveral build --framework flutter --platform ios
```

---

## Flutter

**Supported platforms:** iOS, Android, macOS, Windows, Linux, Web

### Build Commands

```bash
# Debug build
canaveral build --platform ios --profile debug
canaveral build --platform android --profile debug

# Release build
canaveral build --platform ios --profile release
canaveral build --platform android --profile release

# All platforms
canaveral build --profile release
```

### Version Management

Version is read from and written to `pubspec.yaml`:

```yaml
version: 1.2.3+42  # version+build_number
```

```bash
canaveral version              # Shows 1.2.3 (build 42)
canaveral version --bump patch # 1.2.3+42 -> 1.2.4+43
```

### Testing

```bash
# Unit tests
canaveral test --type unit

# Widget tests
canaveral test --type widget

# Integration tests
canaveral test --type integration

# With coverage
canaveral test --type unit --coverage
```

### Screenshots

Integrates with `integration_test` for screenshot capture:

```bash
canaveral screenshots capture --config screenshots.yaml
```

---

## Expo

**Supported platforms:** iOS, Android

### Build Modes

Canaveral supports both EAS Build and local builds:

```bash
# Local build (uses expo prebuild + native tools)
canaveral build --platform ios --profile release

# EAS Build (cloud-based)
canaveral build --platform ios --profile release --config eas=true
```

### Version Management

Version is managed in `app.json` or `app.config.js`:

```json
{
  "expo": {
    "version": "1.2.3",
    "ios": {
      "buildNumber": "42"
    },
    "android": {
      "versionCode": 42
    }
  }
}
```

```bash
canaveral version --bump patch  # Updates all version fields
```

### OTA Updates

Push updates via EAS Update:

```bash
canaveral ota publish --channel production --message "Bug fixes"
```

---

## React Native

**Supported platforms:** iOS, Android

### Build Commands

```bash
# iOS (uses Xcode under the hood)
canaveral build --platform ios --profile release

# Android (uses Gradle)
canaveral build --platform android --profile release
```

### Version Management

Version is read from multiple sources:
- `package.json` - JavaScript version
- `ios/[App]/Info.plist` - iOS version
- `android/app/build.gradle` - Android version

```bash
canaveral version              # Shows version from package.json
canaveral version --bump patch # Updates all sources
```

### Metro Bundler

Canaveral automatically starts Metro bundler if needed or uses the running instance.

### Testing

```bash
# Jest tests
canaveral test --type unit

# Detox E2E tests
canaveral test --type e2e --platform ios
```

---

## Native iOS (Xcode)

**Supported platforms:** iOS, macOS

### Build Commands

```bash
# Auto-detects scheme from .xcworkspace or .xcodeproj
canaveral build --platform ios --profile release

# Specify scheme
canaveral build --platform ios --profile release --config scheme=MyAppRelease

# Build for simulator
canaveral build --platform ios --profile debug --config destination=simulator
```

### Project Types

Supports:
- Xcode workspaces (`.xcworkspace`) - with CocoaPods/SPM
- Xcode projects (`.xcodeproj`) - standalone
- Swift Package Manager projects

### Version Management

Version is read from `Info.plist`:

```xml
<key>CFBundleShortVersionString</key>
<string>1.2.3</string>
<key>CFBundleVersion</key>
<string>42</string>
```

```bash
canaveral version --bump patch  # Updates both keys
```

### Code Signing

Uses Canaveral's match-style certificate sync:

```bash
# Sync certificates
canaveral match sync --profile-type appstore

# Build with automatic signing
canaveral build --platform ios --profile release
```

---

## Native Android (Gradle)

**Supported platforms:** Android

### Build Commands

```bash
# Debug APK
canaveral build --platform android --profile debug

# Release AAB (for Play Store)
canaveral build --platform android --profile release

# Specific flavor
canaveral build --platform android --profile release --config flavor=production
```

### Gradle Support

Supports:
- Gradle wrapper (`./gradlew`)
- Kotlin DSL (`build.gradle.kts`)
- Groovy DSL (`build.gradle`)
- Build flavors and variants

### Version Management

Version is read from `app/build.gradle` or `app/build.gradle.kts`:

```kotlin
android {
    defaultConfig {
        versionCode = 42
        versionName = "1.2.3"
    }
}
```

```bash
canaveral version --bump patch  # Updates versionName and versionCode
```

### Signing

Configure signing via environment variables:

```bash
export ANDROID_KEYSTORE_PATH=/path/to/release.keystore
export ANDROID_KEYSTORE_PASSWORD=password
export ANDROID_KEY_ALIAS=release
export ANDROID_KEY_PASSWORD=password
```

---

## Tauri

**Supported platforms:** macOS, Windows, Linux

### Build Commands

```bash
# Build for current platform
canaveral build --platform macos
canaveral build --platform windows
canaveral build --platform linux

# Debug build
canaveral build --platform macos --profile debug
```

### Bundle Types

Tauri produces different bundle types per platform:

| Platform | Bundles |
|----------|---------|
| macOS | `.app`, `.dmg`, `.pkg` |
| Windows | `.exe`, `.msi` |
| Linux | `.deb`, `.rpm`, `.AppImage` |

### Version Management

Version is read from `src-tauri/tauri.conf.json` and `src-tauri/Cargo.toml`:

```json
{
  "version": "1.2.3"
}
```

```bash
canaveral version --bump patch  # Updates both files
```

### Package Manager Support

Auto-detects and uses the project's package manager:
- npm (`package-lock.json`)
- yarn (`yarn.lock`)
- pnpm (`pnpm-lock.yaml`)
- bun (`bun.lockb`)

---

## Framework Comparison

### Build Capabilities

| Capability | Flutter | Expo | React Native | iOS | Android | Tauri |
|------------|---------|------|--------------|-----|---------|-------|
| iOS build | ✓ | ✓ | ✓ | ✓ | - | - |
| Android build | ✓ | ✓ | ✓ | - | ✓ | - |
| macOS build | ✓ | - | - | ✓ | - | ✓ |
| Windows build | ✓ | - | - | - | - | ✓ |
| Linux build | ✓ | - | - | - | - | ✓ |
| Debug build | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Release build | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Build flavors | ✓ | - | - | - | ✓ | - |

### Testing Capabilities

| Capability | Flutter | Expo | React Native | iOS | Android | Tauri |
|------------|---------|------|--------------|-----|---------|-------|
| Unit tests | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Widget tests | ✓ | - | - | - | - | - |
| Integration tests | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ |
| Screenshot tests | ✓ | ✓ | ✓ | ✓ | ✓ | - |

### Distribution Capabilities

| Capability | Flutter | Expo | React Native | iOS | Android | Tauri |
|------------|---------|------|--------------|-----|---------|-------|
| TestFlight | ✓ | ✓ | ✓ | ✓ | - | - |
| App Store | ✓ | ✓ | ✓ | ✓ | - | - |
| Play Store | ✓ | ✓ | ✓ | - | ✓ | - |
| Firebase | ✓ | ✓ | ✓ | ✓ | ✓ | - |
| OTA updates | ✓ | ✓ | ✓ | - | - | ✓ |

---

## Adding Custom Framework Support

Canaveral's framework adapter system is extensible. To add support for a new framework:

1. Implement the `BuildAdapter` trait
2. Register the adapter with the framework registry
3. Provide detection logic

See the [Adapter Development Guide](../../design/adapters.md) for details.
