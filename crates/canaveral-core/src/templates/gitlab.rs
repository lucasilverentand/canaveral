//! GitLab CI templates

use crate::error::Result;

use super::{CITemplate, TemplateOptions};

/// GitLab CI template generator
#[derive(Debug, Clone, Default)]
pub struct GitLabCITemplate;

impl GitLabCITemplate {
    /// Create a new template
    pub fn new() -> Self {
        Self
    }

    /// Get the Docker image for the package type
    fn get_image(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => {
                let version = options.node_version.as_deref().unwrap_or("20");
                format!("node:{}-alpine", version)
            }
            Some("cargo") => "rust:latest".to_string(),
            Some("python") => {
                let version = options.python_version.as_deref().unwrap_or("3.11");
                format!("python:{}-slim", version)
            }
            Some("go") => {
                let version = options.go_version.as_deref().unwrap_or("1.21");
                format!("golang:{}", version)
            }
            Some("maven") => {
                let version = options.java_version.as_deref().unwrap_or("17");
                format!("maven:3-eclipse-temurin-{}", version)
            }
            _ => "alpine:latest".to_string(),
        }
    }

    /// Generate cache configuration
    fn generate_cache(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => r#"cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - node_modules/"#
                .to_string(),
            Some("cargo") => r#"cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - target/
    - .cargo/"#
                .to_string(),
            Some("python") => r#"cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - .venv/"#
                .to_string(),
            Some("go") => r#"cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - /go/pkg/mod/"#
                .to_string(),
            Some("maven") => r#"cache:
  key: ${CI_COMMIT_REF_SLUG}
  paths:
    - .m2/repository/"#
                .to_string(),
            _ => String::new(),
        }
    }

    /// Generate before_script
    fn generate_before_script(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => "  before_script:\n    - npm ci".to_string(),
            Some("cargo") => "  before_script:\n    - rustup component add clippy rustfmt".to_string(),
            Some("python") => r#"  before_script:
    - python -m venv .venv
    - source .venv/bin/activate
    - pip install -e .[dev]"#
                .to_string(),
            Some("go") => "  before_script:\n    - go mod download".to_string(),
            Some("maven") => String::new(),
            _ => String::new(),
        }
    }

    /// Generate test job
    fn generate_test_job(&self, options: &TemplateOptions) -> String {
        let before_script = self.generate_before_script(options);
        let script = match options.package_type.as_deref() {
            Some("npm") => r#"  script:
    - npm test
    - npm run lint"#
                .to_string(),
            Some("cargo") => r#"  script:
    - cargo test
    - cargo clippy -- -D warnings
    - cargo fmt -- --check"#
                .to_string(),
            Some("python") => r#"  script:
    - pytest
    - ruff check ."#
                .to_string(),
            Some("go") => r#"  script:
    - go test ./...
    - golangci-lint run"#
                .to_string(),
            Some("maven") => r#"  script:
    - mvn test
    - mvn verify"#
                .to_string(),
            _ => r#"  script:
    - echo "Add your test command here""#
                .to_string(),
        };

        format!(
            r#"test:
  stage: test
{before_script}
{script}
  rules:
    - if: $CI_PIPELINE_SOURCE == 'merge_request_event'
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH"#
        )
    }

    /// Generate release job
    fn generate_release_job(&self, options: &TemplateOptions) -> String {
        let publish_script = match options.package_type.as_deref() {
            Some("npm") if options.include_publish => r#"    - echo "//registry.npmjs.org/:_authToken=${NPM_TOKEN}" > .npmrc
    - npm publish"#
                .to_string(),
            Some("cargo") if options.include_publish => {
                "    - cargo publish".to_string()
            }
            Some("python") if options.include_publish => r#"    - pip install build twine
    - python -m build
    - twine upload dist/*"#
                .to_string(),
            Some("maven") if options.include_publish => {
                "    - mvn deploy -P release".to_string()
            }
            _ => String::new(),
        };

        let changelog_script = if options.include_changelog {
            "    - canaveral changelog --output CHANGELOG.md"
        } else {
            ""
        };

        format!(
            r#"release:
  stage: release
  script:
    - cargo install canaveral
    - VERSION=$(canaveral version --print)
{changelog_script}
    - canaveral version --set $VERSION
    - git config user.name "GitLab CI"
    - git config user.email "ci@gitlab.com"
    - git add -A
    - git commit -m "chore(release): $VERSION"
    - git tag "v$VERSION"
    - git push origin $CI_COMMIT_BRANCH --tags
{publish_script}
  rules:
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
      when: manual
  allow_failure: false"#
        )
    }
}

impl CITemplate for GitLabCITemplate {
    fn platform_name(&self) -> &'static str {
        "GitLab CI"
    }

    fn config_path(&self) -> &'static str {
        ".gitlab-ci.yml"
    }

    fn generate(&self, options: &TemplateOptions) -> Result<String> {
        let image = self.get_image(options);
        let cache = self.generate_cache(options);
        let test_job = self.generate_test_job(options);
        let release_job = self.generate_release_job(options);

        let workflow = format!(
            r#"image: {image}

stages:
  - test
  - release

{cache}

{test_job}

{release_job}
"#
        );

        Ok(workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_npm_gitlab_ci() {
        let template = GitLabCITemplate::new();
        let options = TemplateOptions::new()
            .with_project_name("my-npm-package")
            .with_package_type("npm");

        let config = template.generate(&options).unwrap();

        assert!(config.contains("node:"));
        assert!(config.contains("npm ci"));
        assert!(config.contains("npm test"));
        assert!(config.contains("npm publish"));
    }

    #[test]
    fn test_generate_cargo_gitlab_ci() {
        let template = GitLabCITemplate::new();
        let options = TemplateOptions::new()
            .with_project_name("my-rust-crate")
            .with_package_type("cargo");

        let config = template.generate(&options).unwrap();

        assert!(config.contains("rust:"));
        assert!(config.contains("cargo test"));
        assert!(config.contains("cargo clippy"));
        assert!(config.contains("cargo publish"));
    }

    #[test]
    fn test_config_path() {
        let template = GitLabCITemplate::new();
        assert_eq!(template.config_path(), ".gitlab-ci.yml");
    }

    #[test]
    fn test_get_image() {
        let template = GitLabCITemplate::new();

        let npm_opts = TemplateOptions::new().with_package_type("npm");
        assert!(template.get_image(&npm_opts).contains("node:"));

        let cargo_opts = TemplateOptions::new().with_package_type("cargo");
        assert_eq!(template.get_image(&cargo_opts), "rust:latest");

        let python_opts = TemplateOptions::new().with_package_type("python");
        assert!(template.get_image(&python_opts).contains("python:"));
    }
}
