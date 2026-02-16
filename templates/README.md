# Canaveral CI/CD Templates

Pre-built CI/CD configuration templates for popular platforms. Copy the appropriate template to your project and customize as needed.

## Quick Start

1. Choose your CI/CD platform
2. Copy the template to your project
3. Configure required secrets/variables
4. Customize workflow triggers and settings

## Available Templates

### GitHub Actions

Location: `github-actions/`

| Template | Description |
|----------|-------------|
| `build-ios.yml` | iOS build workflow with certificate sync |
| `build-android.yml` | Android build workflow with keystore handling |
| `build-multiplatform.yml` | Combined iOS + Android builds |
| `release.yml` | Full release workflow with version bumping |
| `screenshots.yml` | Screenshot capture automation |

**Usage:**
```bash
mkdir -p .github/workflows
cp templates/github-actions/build-multiplatform.yml .github/workflows/build.yml
```

**Required Secrets:**
- `MATCH_PASSWORD` - Certificate encryption password
- `APP_STORE_CONNECT_API_KEY` - Base64-encoded API key JSON
- `ANDROID_KEYSTORE_BASE64` - Base64-encoded keystore file
- `ANDROID_KEYSTORE_PASSWORD` - Keystore password
- `ANDROID_KEY_ALIAS` - Signing key alias
- `ANDROID_KEY_PASSWORD` - Signing key password
- `GOOGLE_PLAY_SERVICE_ACCOUNT` - Base64-encoded service account JSON

**Required Variables:**
- `APPLE_TEAM_ID` - Apple Developer Team ID
- `IOS_BUNDLE_ID` - iOS app bundle identifier
- `MATCH_GIT_URL` - Git repository URL for certificates

### GitLab CI

Location: `gitlab-ci/`

| Template | Description |
|----------|-------------|
| `.gitlab-ci.yml` | Complete CI/CD configuration |

**Usage:**
```bash
cp templates/gitlab-ci/.gitlab-ci.yml .gitlab-ci.yml
```

**Required CI/CD Variables:**
Same as GitHub Actions, configured in GitLab CI/CD settings.

### Bitrise

Location: `bitrise/`

| Template | Description |
|----------|-------------|
| `bitrise.yml` | Complete Bitrise workflow |

**Usage:**
1. Copy `bitrise.yml` to your project root
2. Configure secrets in Bitrise dashboard
3. Connect repository in Bitrise

### CircleCI

Location: `circleci/`

| Template | Description |
|----------|-------------|
| `config.yml` | Complete CircleCI configuration |

**Usage:**
```bash
mkdir -p .circleci
cp templates/circleci/config.yml .circleci/config.yml
```

### Azure Pipelines

Location: `azure-pipelines/`

| Template | Description |
|----------|-------------|
| `azure-pipelines.yml` | Complete Azure DevOps pipeline |

**Usage:**
```bash
cp templates/azure-pipelines/azure-pipelines.yml azure-pipelines.yml
```

## Configuration

### Encoding Secrets

Many CI/CD platforms require base64-encoded files:

```bash
# Encode keystore
base64 -i android/app/release.keystore | pbcopy

# Encode App Store Connect API key
base64 -i AuthKey_XXXXXX.p8 | pbcopy

# Encode Google Play service account
base64 -i service-account.json | pbcopy
```

### Certificate Management

All templates use Canaveral's match-style certificate sync:

```bash
# Initialize certificate repository
canaveral match init --storage git --git-url git@github.com:org/certs.git --team-id TEAMID

# Sync certificates locally
canaveral match sync --profile-type development

# Sync in CI (readonly mode)
canaveral match sync --profile-type appstore --readonly
```

### Version Management

Templates support automatic version bumping:

```bash
# Get current version
canaveral version --current

# Calculate next version based on commits
canaveral version

# Calculate next version with explicit release type
canaveral version --release-type patch
canaveral version --release-type minor
canaveral version --release-type major
```

## Customization

### Adding Custom Build Steps

All templates use Canaveral's build command which detects your framework automatically:

```yaml
# Basic build
canaveral build --platform ios --profile release

# For specific framework (usually auto-detected)
canaveral build --platform ios --framework flutter
```

### Environment Variables

Common environment variables:

| Variable | Description |
|----------|-------------|
| `CANAVERAL_TEAM_ID` | Apple Developer Team ID |
| `CANAVERAL_BUNDLE_ID` | iOS bundle identifier |
| `MATCH_GIT_URL` | Certificate repository URL |
| `MATCH_PASSWORD` | Certificate encryption password |
| `GOOGLE_PLAY_SERVICE_ACCOUNT_JSON` | Google Play credentials |
| `APP_STORE_CONNECT_API_KEY` | App Store Connect credentials |

### Conditional Builds

Templates include conditions for different scenarios:

```yaml
# Build only on main branch
if: github.ref == 'refs/heads/main'

# Build only for tags
if: startsWith(github.ref, 'refs/tags/v')

# Build for PRs
if: github.event_name == 'pull_request'
```

## Troubleshooting

### iOS Build Issues

1. **Certificate not found**: Ensure `MATCH_GIT_URL` and `MATCH_PASSWORD` are set
2. **Code signing failed**: Run `canaveral match sync` to refresh certificates
3. **Profile expired**: Run `canaveral match nuke` then `canaveral match sync`

### Android Build Issues

1. **Keystore not found**: Ensure `ANDROID_KEYSTORE_BASE64` is properly encoded
2. **Signing failed**: Verify key alias and passwords match the keystore
3. **Build tools missing**: The templates install required tools automatically

### General Issues

1. **Framework not detected**: Ensure your project has standard framework markers (pubspec.yaml, package.json, etc.)
2. **Canaveral not found**: Check that the install script completed successfully
3. **Tests failing**: Run `canaveral test` locally first to debug

## Examples

### Minimal iOS Build (GitHub Actions)

```yaml
name: iOS Build
on: [push]
jobs:
  build:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - run: |
          curl -fsSL https://get.canaveral.dev | sh
          echo "$HOME/.canaveral/bin" >> $GITHUB_PATH
      - run: canaveral build --platform ios --profile debug
```

### Production Release (GitHub Actions)

```yaml
name: Release
on:
  push:
    tags: ['v*']
jobs:
  release:
    runs-on: macos-14
    steps:
      - uses: actions/checkout@v4
      - run: |
          curl -fsSL https://get.canaveral.dev | sh
          echo "$HOME/.canaveral/bin" >> $GITHUB_PATH
      - run: canaveral match sync --profile-type appstore --readonly
      - run: canaveral build --platform ios --profile release
      - run: canaveral test-flight upload build/ios/*.ipa
    env:
      MATCH_GIT_URL: ${{ vars.MATCH_GIT_URL }}
      MATCH_PASSWORD: ${{ secrets.MATCH_PASSWORD }}
      APP_STORE_CONNECT_API_KEY: ${{ secrets.APP_STORE_CONNECT_API_KEY }}
```
