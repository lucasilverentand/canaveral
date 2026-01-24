# App Store Metadata Management Plan

## Overview

Add comprehensive metadata management for App Store Connect (Apple) and Google Play Store, enabling teams to manage app descriptions, screenshots, keywords, categories, and other store listing content directly through canaveral.

## Goals

1. **Centralized metadata storage** - Single source of truth for all app store metadata
2. **Localization support** - Full multi-language/locale support matching store requirements
3. **Validation** - Pre-upload validation against store requirements and constraints
4. **Synchronization** - Pull/push metadata from/to app stores
5. **Version control friendly** - File-based storage that works well with git
6. **CI/CD integration** - Automation-friendly commands and workflows

## Architecture

### New Crate: `canaveral-metadata`

```
canaveral-metadata/
├── src/
│   ├── lib.rs                    # Public API
│   ├── error.rs                  # MetadataError enum
│   ├── types/
│   │   ├── mod.rs
│   │   ├── common.rs             # Shared types (Locale, MediaAsset, etc.)
│   │   ├── apple.rs              # Apple-specific metadata types
│   │   └── google_play.rs        # Google Play-specific metadata types
│   ├── storage/
│   │   ├── mod.rs                # Storage trait and factory
│   │   ├── fastlane.rs           # Fastlane-compatible directory structure
│   │   └── unified.rs            # Single YAML/JSON file format
│   ├── validation/
│   │   ├── mod.rs                # Validation orchestration
│   │   ├── apple.rs              # Apple validation rules
│   │   └── google_play.rs        # Google Play validation rules
│   ├── sync/
│   │   ├── mod.rs                # Sync orchestration
│   │   ├── apple.rs              # App Store Connect sync
│   │   └── google_play.rs        # Google Play Console sync
│   └── diff.rs                   # Metadata diffing utilities
```

### Core Types

```rust
/// Represents all metadata for an app across all locales
pub struct AppMetadata {
    pub app_id: String,
    pub platform: Platform,
    pub default_locale: Locale,
    pub localizations: HashMap<Locale, LocalizedMetadata>,
    pub assets: MetadataAssets,
    pub category: AppCategory,
    pub age_rating: AgeRatingInfo,
    pub pricing: Option<PricingInfo>,
}

/// Locale-specific text content
pub struct LocalizedMetadata {
    pub locale: Locale,
    pub name: String,                           // 30 chars (Apple), 50 chars (Google)
    pub subtitle: Option<String>,               // 30 chars (Apple only)
    pub description: String,                    // 4000 chars
    pub short_description: Option<String>,      // 80 chars (Google only)
    pub keywords: Option<String>,               // 100 chars (Apple only)
    pub whats_new: Option<String>,              // Release notes
    pub promotional_text: Option<String>,       // 170 chars (Apple only)
    pub support_url: Option<String>,
    pub marketing_url: Option<String>,
    pub privacy_policy_url: Option<String>,
}

/// Media assets organized by type and locale
pub struct MetadataAssets {
    pub icon: Option<PathBuf>,
    pub screenshots: HashMap<Locale, ScreenshotSet>,
    pub previews: HashMap<Locale, PreviewSet>,
    pub feature_graphic: Option<PathBuf>,       // Google Play
}

/// Screenshots for different device types
pub struct ScreenshotSet {
    pub iphone_6_5: Vec<PathBuf>,               // iPhone 14 Pro Max, etc.
    pub iphone_5_5: Vec<PathBuf>,               // iPhone 8 Plus, etc.
    pub ipad_pro_12_9: Vec<PathBuf>,
    pub ipad_pro_11: Vec<PathBuf>,
    pub mac: Vec<PathBuf>,
    pub apple_tv: Vec<PathBuf>,
    pub apple_watch: Vec<PathBuf>,
    // Google Play
    pub phone: Vec<PathBuf>,
    pub tablet_7: Vec<PathBuf>,
    pub tablet_10: Vec<PathBuf>,
    pub tv: Vec<PathBuf>,
    pub wear: Vec<PathBuf>,
}
```

### Storage Trait

```rust
#[async_trait]
pub trait MetadataStorage: Send + Sync {
    /// Load metadata from storage
    async fn load(&self, app_id: &str, platform: Platform) -> Result<AppMetadata>;

    /// Save metadata to storage
    async fn save(&self, metadata: &AppMetadata) -> Result<()>;

    /// Check if metadata exists
    async fn exists(&self, app_id: &str, platform: Platform) -> Result<bool>;

    /// List available localizations
    async fn list_locales(&self, app_id: &str, platform: Platform) -> Result<Vec<Locale>>;

    /// Get asset path for a specific asset type
    fn asset_path(&self, app_id: &str, platform: Platform, asset_type: AssetType) -> PathBuf;
}
```

### Storage Formats

#### 1. Fastlane-Compatible (Default)

```
metadata/
├── apple/
│   └── com.example.app/
│       ├── en-US/
│       │   ├── name.txt
│       │   ├── subtitle.txt
│       │   ├── description.txt
│       │   ├── keywords.txt
│       │   ├── release_notes.txt
│       │   ├── promotional_text.txt
│       │   ├── support_url.txt
│       │   ├── marketing_url.txt
│       │   └── privacy_url.txt
│       ├── de-DE/
│       │   └── ...
│       ├── screenshots/
│       │   ├── en-US/
│       │   │   ├── iphone_6_5_01.png
│       │   │   ├── iphone_6_5_02.png
│       │   │   └── ...
│       │   └── de-DE/
│       │       └── ...
│       └── app_store_info.yaml   # category, age rating, etc.
└── google_play/
    └── com.example.app/
        ├── en-US/
        │   ├── title.txt
        │   ├── short_description.txt
        │   ├── full_description.txt
        │   └── changelogs/
        │       ├── 100.txt       # Version code specific
        │       └── default.txt
        ├── screenshots/
        │   ├── en-US/
        │   │   ├── phone/
        │   │   ├── tablet/
        │   │   └── ...
        │   └── ...
        └── store_info.yaml
```

#### 2. Unified YAML Format

```yaml
# metadata/com.example.app.yaml
app_id: com.example.app
platform: apple
default_locale: en-US

category:
  primary: games
  secondary: puzzle

age_rating:
  alcohol_tobacco_drugs: none
  contests: none
  gambling: none
  horror: none
  mature_suggestive: none
  medical: none
  profanity: none
  sexual_content_nudity: none
  violence_cartoon: none
  violence_realistic: none

localizations:
  en-US:
    name: "My Awesome App"
    subtitle: "The best app ever"
    description: |
      Long description goes here.

      Multiple paragraphs supported.
    keywords: "keyword1,keyword2,keyword3"
    whats_new: "Bug fixes and performance improvements"
    promotional_text: "Now with new features!"
    support_url: "https://example.com/support"
    marketing_url: "https://example.com"
    privacy_policy_url: "https://example.com/privacy"

  de-DE:
    name: "Meine tolle App"
    subtitle: "Die beste App überhaupt"
    # ... other fields

assets:
  icon: assets/icon.png
  screenshots:
    en-US:
      iphone_6_5:
        - assets/screenshots/en-US/iphone_6_5_01.png
        - assets/screenshots/en-US/iphone_6_5_02.png
    de-DE:
      iphone_6_5:
        - assets/screenshots/de-DE/iphone_6_5_01.png
```

### Validation Rules

#### Apple App Store

| Field | Max Length | Required | Notes |
|-------|------------|----------|-------|
| name | 30 | Yes | App name |
| subtitle | 30 | No | iOS 11+ |
| description | 4000 | Yes | |
| keywords | 100 | No | Comma-separated |
| whats_new | 4000 | Yes* | Required for updates |
| promotional_text | 170 | No | Can be changed without new build |
| support_url | - | Yes | Must be valid URL |
| privacy_policy_url | - | Yes* | Required for most categories |

Screenshot requirements:
- iPhone 6.5": 1242 x 2688 or 1284 x 2778 pixels
- iPhone 5.5": 1242 x 2208 pixels
- iPad Pro 12.9": 2048 x 2732 pixels
- Minimum 1, maximum 10 per device type

#### Google Play Store

| Field | Max Length | Required | Notes |
|-------|------------|----------|-------|
| title | 50 | Yes | App name |
| short_description | 80 | Yes | |
| full_description | 4000 | Yes | |
| changelogs | 500 | No | Per version code |

Screenshot requirements:
- Minimum 2, maximum 8 per device type
- Phone: 320-3840px, 16:9 or 9:16 aspect ratio
- Tablet 7": 320-3840px
- Feature graphic: 1024 x 500 pixels (required)

### CLI Commands

```
canaveral metadata
├── init                          # Initialize metadata directory structure
│   ├── --platform <apple|google_play|both>
│   ├── --app-id <bundle_id>
│   ├── --format <fastlane|unified>
│   └── --locales <en-US,de-DE,...>
│
├── validate                      # Validate metadata against store requirements
│   ├── --platform <apple|google_play>
│   ├── --app-id <bundle_id>
│   ├── --strict                  # Fail on warnings
│   └── --fix                     # Auto-fix common issues (trim whitespace, etc.)
│
├── sync
│   ├── pull                      # Download metadata from store
│   │   ├── --platform <apple|google_play>
│   │   ├── --app-id <bundle_id>
│   │   ├── --locales <all|en-US,...>
│   │   └── --include-assets      # Also download screenshots
│   │
│   └── push                      # Upload metadata to store
│       ├── --platform <apple|google_play>
│       ├── --app-id <bundle_id>
│       ├── --locales <all|en-US,...>
│       ├── --include-assets      # Also upload screenshots
│       ├── --skip-screenshots    # Only update text
│       └── --dry-run             # Preview changes
│
├── diff                          # Compare local vs remote metadata
│   ├── --platform <apple|google_play>
│   ├── --app-id <bundle_id>
│   └── --locales <all|en-US,...>
│
├── add-locale                    # Add a new localization
│   ├── --platform <apple|google_play>
│   ├── --app-id <bundle_id>
│   ├── --locale <de-DE>
│   └── --copy-from <en-US>       # Copy content from existing locale
│
├── remove-locale                 # Remove a localization
│   ├── --platform <apple|google_play>
│   ├── --app-id <bundle_id>
│   └── --locale <de-DE>
│
├── screenshots
│   ├── add                       # Add screenshot
│   │   ├── --platform <apple|google_play>
│   │   ├── --app-id <bundle_id>
│   │   ├── --locale <en-US>
│   │   ├── --device <iphone_6_5>
│   │   └── <path>
│   │
│   ├── remove                    # Remove screenshot
│   │   └── ...
│   │
│   ├── reorder                   # Reorder screenshots
│   │   └── ...
│   │
│   └── validate                  # Validate screenshot dimensions
│       └── ...
│
└── export                        # Export metadata to different formats
    ├── --platform <apple|google_play>
    ├── --app-id <bundle_id>
    ├── --format <json|yaml|csv>
    └── --output <path>
```

### Configuration

Add to `canaveral.yaml`:

```yaml
metadata:
  enabled: true
  storage:
    format: fastlane              # fastlane | unified
    path: ./metadata              # Base path for metadata files

  defaults:
    default_locale: en-US
    support_url: https://example.com/support
    privacy_policy_url: https://example.com/privacy

  validation:
    strict: false                 # Treat warnings as errors
    required_locales:             # Locales that must be present
      - en-US
    max_description_length: 4000  # Override default limits

  sync:
    auto_pull: false              # Pull metadata before release
    include_assets: true          # Include screenshots in sync
    backup_before_push: true      # Create backup before pushing
```

### Integration with Existing Stores

Extend `canaveral-stores` upload workflow:

```rust
// In upload workflow
pub async fn upload_with_metadata(
    &self,
    artifact: &Path,
    options: &UploadOptions,
    metadata_storage: &dyn MetadataStorage,
) -> Result<UploadResult> {
    // 1. Validate artifact
    let validation = self.validate_artifact(artifact).await?;

    // 2. Load and validate metadata
    let metadata = metadata_storage.load(&validation.app_info.bundle_id, self.platform()).await?;
    let metadata_validation = validate_metadata(&metadata, self.platform())?;

    if !metadata_validation.valid {
        return Err(StoreError::MetadataInvalid(metadata_validation.errors));
    }

    // 3. Upload artifact
    let upload_result = self.upload(artifact, options).await?;

    // 4. Sync metadata if requested
    if options.sync_metadata {
        self.sync_metadata(&metadata).await?;
    }

    Ok(upload_result)
}
```

## Implementation Phases

### Phase 1: Core Types and Storage (Foundation)

1. Create `canaveral-metadata` crate
2. Define core types (`AppMetadata`, `LocalizedMetadata`, etc.)
3. Implement `MetadataStorage` trait
4. Implement Fastlane-compatible storage backend
5. Add basic error types

**Deliverables:**
- Can load/save metadata from file system
- Fastlane directory structure support
- Basic type validation

### Phase 2: Validation

1. Implement Apple validation rules
2. Implement Google Play validation rules
3. Add validation CLI command
4. Screenshot dimension validation
5. Character count validation with locale awareness

**Deliverables:**
- `canaveral metadata validate` command
- Detailed validation error messages
- Auto-fix for common issues

### Phase 3: CLI Commands

1. `metadata init` command
2. `metadata add-locale` / `remove-locale`
3. `metadata screenshots` subcommands
4. `metadata export` command
5. Configuration integration

**Deliverables:**
- Full CLI for local metadata management
- Integration with main config file

### Phase 4: Store Synchronization

1. Implement Apple App Store Connect sync
   - Pull metadata from store
   - Push metadata to store
   - Screenshot upload/download
2. Implement Google Play Console sync
3. `metadata sync pull/push` commands
4. `metadata diff` command

**Deliverables:**
- Bi-directional sync with app stores
- Diff visualization
- Dry-run support

### Phase 5: Upload Integration

1. Integrate metadata validation into upload workflow
2. Optional metadata sync during upload
3. Release notes from metadata
4. Phased rollout support

**Deliverables:**
- Seamless upload + metadata workflow
- CI/CD friendly automation

### Phase 6: Advanced Features

1. Unified YAML storage format
2. Metadata templating (shared content across locales)
3. AI-assisted translation suggestions (optional)
4. Screenshot auto-resize/optimization
5. Metadata change history/audit log

**Deliverables:**
- Alternative storage format
- Power user features

## Testing Strategy

1. **Unit tests** - Type validation, storage read/write
2. **Integration tests** - Full workflow tests with mock stores
3. **Fixture-based tests** - Real metadata samples for validation
4. **CLI tests** - Command output and behavior verification

## Dependencies

New dependencies for `canaveral-metadata`:
- `image` - Screenshot dimension validation
- `reqwest` - HTTP client for store APIs (already in stores)
- `tokio` - Async runtime (already in workspace)
- `serde` / `serde_yaml` / `serde_json` - Serialization (already in workspace)

## Open Questions

1. **Fastlane compatibility level** - Should we aim for 100% fastlane deliver compatibility, or create our own optimized format?
   - Recommendation: Start with fastlane compatibility for easy migration, add unified format later

2. **Screenshot processing** - Should we auto-resize screenshots to required dimensions?
   - Recommendation: Validate dimensions, provide resize command, don't auto-process

3. **Metadata versioning** - Should metadata be versioned alongside app versions?
   - Recommendation: Git handles versioning, we just manage files

4. **Translation workflow** - Should we integrate with translation services?
   - Recommendation: Out of scope for initial release, consider as future enhancement

## Success Criteria

- [ ] Can initialize metadata structure for new app
- [ ] Can validate metadata against all store requirements
- [ ] Can sync metadata bidirectionally with Apple App Store
- [ ] Can sync metadata bidirectionally with Google Play
- [ ] Can manage screenshots with proper validation
- [ ] Works in CI/CD pipelines without manual intervention
- [ ] Migration path from fastlane deliver
- [ ] Comprehensive documentation and examples
