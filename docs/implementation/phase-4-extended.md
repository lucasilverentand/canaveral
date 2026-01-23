# Phase 4: Extended Ecosystem ✅

**Status:** Complete

**Goal**: Support additional ecosystems and versioning strategies.

## Tasks

### 4.1 Go Adapter

- [x] Parse go.mod for module path
- [x] Version via git tags (Go convention)
- [x] Handle major version in path (v2+)
- [x] Support private modules

```rust
// crates/canaveral-adapters/src/go/mod.rs
use crate::traits::{PackageAdapter, PublishOptions, PublishResult};
use async_trait::async_trait;

pub struct GoAdapter;

#[async_trait]
impl PackageAdapter for GoAdapter {
    fn name(&self) -> &str { "go" }
    fn ecosystem(&self) -> &str { "go" }
    fn manifest_file(&self) -> &str { "go.mod" }

    async fn detect(&self, project_path: &Path) -> Result<bool> {
        Ok(project_path.join("go.mod").exists())
    }

    async fn read_version(&self, _manifest_path: &Path) -> Result<String> {
        // Go versions come from git tags
        let tag = self.git.find_latest_tag("v*")?;
        Ok(tag.map(|t| t.trim_start_matches('v').to_string())
            .unwrap_or_else(|| "0.0.0".to_string()))
    }

    async fn write_version(&self, manifest_path: &Path, version: &str) -> Result<()> {
        // For v2+, update module path
        let major: u64 = version.split('.').next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);

        if major >= 2 {
            self.update_module_path(manifest_path, major).await?;
        }
        Ok(())
    }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        // Go modules are "published" via git tags
        let tag = format!("v{}", options.version);

        if !options.dry_run {
            self.git.create_tag(&tag, &format!("Release {}", options.version))?;
            self.git.push("origin", &[&tag])?;
        }

        Ok(PublishResult {
            success: true,
            package_name: self.read_module_path(&options.path).await?,
            version: options.version.clone(),
            registry: "git".to_string(),
            url: Some(format!("https://pkg.go.dev/{}@{}", self.module_path, tag)),
            error: None,
        })
    }

    async fn validate_credentials(&self) -> Result<bool> {
        // No registry auth needed, uses git
        Ok(true)
    }
}
```

### 4.2 Maven Adapter

- [x] Parse pom.xml (XML)
- [x] Handle parent POMs
- [x] Execute mvn deploy
- [x] Support Maven Central

```rust
// crates/canaveral-adapters/src/maven/mod.rs
use quick_xml::{Reader, Writer, events::Event};

pub struct MavenAdapter;

#[async_trait]
impl PackageAdapter for MavenAdapter {
    fn name(&self) -> &str { "maven" }
    fn ecosystem(&self) -> &str { "maven" }
    fn manifest_file(&self) -> &str { "pom.xml" }

    async fn read_version(&self, manifest_path: &Path) -> Result<String> {
        let content = tokio::fs::read_to_string(manifest_path).await?;
        let doc = roxmltree::Document::parse(&content)?;

        // Try direct version first
        if let Some(version) = doc.descendants()
            .find(|n| n.tag_name().name() == "version" && n.parent().map(|p| p.tag_name().name()) == Some("project"))
            .and_then(|n| n.text())
        {
            return Ok(version.to_string());
        }

        // Try parent version
        if let Some(version) = doc.descendants()
            .find(|n| n.tag_name().name() == "version" && n.parent().map(|p| p.tag_name().name()) == Some("parent"))
            .and_then(|n| n.text())
        {
            return Ok(version.to_string());
        }

        Err(anyhow::anyhow!("No version found in pom.xml"))
    }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        let mut cmd = tokio::process::Command::new("mvn");
        cmd.arg("deploy");
        cmd.arg("-DskipTests");
        cmd.current_dir(&options.path);

        if options.dry_run {
            cmd.arg("-DdryRun=true");
        }

        let output = cmd.output().await?;

        Ok(PublishResult {
            success: output.status.success(),
            package_name: self.read_artifact_id(&options.path).await?,
            version: options.version.clone(),
            registry: "maven-central".to_string(),
            url: None,
            error: if !output.status.success() {
                Some(String::from_utf8_lossy(&output.stderr).to_string())
            } else {
                None
            },
        })
    }
}
```

### 4.3 Docker Adapter

- [x] Build Docker images
- [x] Push to registries
- [x] Handle multi-platform builds
- [x] Support multiple tag strategies

```rust
// crates/canaveral-adapters/src/docker/mod.rs

pub struct DockerAdapter {
    image: String,
    registries: Vec<String>,
}

#[async_trait]
impl PackageAdapter for DockerAdapter {
    fn name(&self) -> &str { "docker" }
    fn ecosystem(&self) -> &str { "docker" }
    fn manifest_file(&self) -> &str { "Dockerfile" }

    async fn publish(&self, options: &PublishOptions) -> Result<PublishResult> {
        let tags = self.generate_tags(&options.version);

        // Build
        let mut build_cmd = tokio::process::Command::new("docker");
        build_cmd.arg("build");
        for tag in &tags {
            build_cmd.arg("-t").arg(tag);
        }
        build_cmd.arg(&options.path);

        if !options.dry_run {
            let build_output = build_cmd.output().await?;
            if !build_output.status.success() {
                return Ok(PublishResult {
                    success: false,
                    error: Some(String::from_utf8_lossy(&build_output.stderr).to_string()),
                    ..Default::default()
                });
            }

            // Push each tag
            for tag in &tags {
                let push_output = tokio::process::Command::new("docker")
                    .args(["push", tag])
                    .output()
                    .await?;

                if !push_output.status.success() {
                    return Ok(PublishResult {
                        success: false,
                        error: Some(String::from_utf8_lossy(&push_output.stderr).to_string()),
                        ..Default::default()
                    });
                }
            }
        }

        Ok(PublishResult {
            success: true,
            package_name: self.image.clone(),
            version: options.version.clone(),
            registry: self.registries.first().cloned().unwrap_or_default(),
            url: None,
            error: None,
        })
    }
}

impl DockerAdapter {
    fn generate_tags(&self, version: &str) -> Vec<String> {
        let mut tags = Vec::new();
        let v: semver::Version = version.parse().unwrap_or_default();

        for registry in &self.registries {
            let base = format!("{}/{}", registry, self.image);
            tags.push(format!("{}:{}", base, version));
            tags.push(format!("{}:{}.{}", base, v.major, v.minor));

            if v.pre.is_empty() {
                tags.push(format!("{}:latest", base));
            }
        }

        tags
    }
}
```

### 4.4 CalVer Strategy

- [x] Support multiple CalVer formats
- [x] Handle date-based versioning
- [x] Micro version incrementing

```rust
// crates/canaveral-strategies/src/calver.rs
use chrono::{Utc, Datelike};

pub struct CalVerStrategy {
    format: CalVerFormat,
    micro_start: u32,
}

#[derive(Debug, Clone)]
pub enum CalVerFormat {
    YearMonthDay,      // 2026.01.15
    YearMonthMicro,    // 2026.01.3
    ShortYearMicro,    // 26.01.3
    YearWeekMicro,     // 2026.03.1
}

impl CalVerStrategy {
    pub fn calculate(&self, current: &str) -> String {
        let now = Utc::now();
        let date_part = self.format_date(&now);
        let current_date = self.extract_date(current);

        if self.is_same_period(&current_date, &now) {
            let micro = self.extract_micro(current);
            format!("{}.{}", date_part, micro + 1)
        } else {
            format!("{}.{}", date_part, self.micro_start)
        }
    }

    fn format_date(&self, date: &chrono::DateTime<Utc>) -> String {
        match self.format {
            CalVerFormat::YearMonthDay => {
                format!("{}.{:02}.{:02}", date.year(), date.month(), date.day())
            }
            CalVerFormat::YearMonthMicro => {
                format!("{}.{:02}", date.year(), date.month())
            }
            CalVerFormat::ShortYearMicro => {
                format!("{:02}.{:02}", date.year() % 100, date.month())
            }
            CalVerFormat::YearWeekMicro => {
                format!("{}.{:02}", date.year(), date.iso_week().week())
            }
        }
    }
}
```

### 4.5 Build Number Strategy

- [x] Monotonic build numbers
- [x] iOS/Android compatibility
- [x] Hybrid with SemVer
- [x] CI environment integration

```rust
// crates/canaveral-strategies/src/buildnum.rs

pub struct BuildNumberStrategy {
    format: BuildNumFormat,
    counter_source: CounterSource,
}

#[derive(Debug, Clone)]
pub enum BuildNumFormat {
    BuildOnly,           // 456
    SemVerDotBuild,      // 1.2.3.456
    SemVerPlusBuild,     // 1.2.3+456
    MajorMinorBuild,     // 1.2.456
}

#[derive(Debug, Clone)]
pub enum CounterSource {
    Git,        // Count commits
    File,       // .buildnum file
    Ci,         // CI environment variable
}

impl BuildNumberStrategy {
    pub async fn calculate(&self, current: &str) -> Result<String> {
        let build = self.get_next_build_number().await?;

        Ok(match self.format {
            BuildNumFormat::BuildOnly => build.to_string(),
            BuildNumFormat::SemVerDotBuild => {
                let semver = self.extract_semver(current);
                format!("{}.{}", semver, build)
            }
            BuildNumFormat::SemVerPlusBuild => {
                let semver = self.extract_semver(current);
                format!("{}+{}", semver, build)
            }
            BuildNumFormat::MajorMinorBuild => {
                let (major, minor) = self.extract_major_minor(current);
                format!("{}.{}.{}", major, minor, build)
            }
        })
    }

    async fn get_next_build_number(&self) -> Result<u64> {
        match self.counter_source {
            CounterSource::Git => {
                // Count commits on current branch
                let output = tokio::process::Command::new("git")
                    .args(["rev-list", "--count", "HEAD"])
                    .output()
                    .await?;
                let count: u64 = String::from_utf8_lossy(&output.stdout)
                    .trim()
                    .parse()?;
                Ok(count)
            }
            CounterSource::File => {
                let path = Path::new(".buildnum");
                let current: u64 = if path.exists() {
                    tokio::fs::read_to_string(path).await?
                        .trim()
                        .parse()
                        .unwrap_or(0)
                } else {
                    0
                };
                let next = current + 1;
                tokio::fs::write(path, next.to_string()).await?;
                Ok(next)
            }
            CounterSource::Ci => {
                let num = std::env::var("GITHUB_RUN_NUMBER")
                    .or_else(|_| std::env::var("CI_PIPELINE_IID"))
                    .or_else(|_| std::env::var("BUILD_NUMBER"))
                    .unwrap_or_else(|_| "0".to_string());
                Ok(num.parse()?)
            }
        }
    }
}
```

## Definition of Done

Phase 4 is complete when:

1. [x] Go adapter creates tags correctly
2. [x] Go v2+ modules update module path
3. [x] Maven adapter reads/writes pom.xml
4. [x] Maven adapter deploys packages
5. [x] Docker adapter builds images
6. [x] Docker adapter pushes with multiple tags
7. [x] CalVer generates correct versions
8. [x] Build numbers increment correctly
9. [x] All new adapters have >80% test coverage
