//! GitHub Actions templates

use crate::error::Result;

use super::{CITemplate, TemplateOptions};

/// GitHub Actions workflow template generator
#[derive(Debug, Clone, Default)]
pub struct GitHubActionsTemplate;

impl GitHubActionsTemplate {
    /// Create a new template
    pub fn new() -> Self {
        Self
    }

    /// Generate the setup step based on package type
    fn generate_setup_step(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => {
                let node_version = options.node_version.as_deref().unwrap_or("20");
                format!(
                    r#"      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '{}'
          cache: 'npm'

      - name: Install dependencies
        run: npm ci"#,
                    node_version
                )
            }
            Some("cargo") => {
                let rust_version = options.rust_version.as_deref().unwrap_or("stable");
                format!(
                    r#"      - name: Setup Rust
        uses: dtolnay/rust-action@stable
        with:
          toolchain: {}

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/bin/
            ~/.cargo/registry/index/
            ~/.cargo/registry/cache/
            ~/.cargo/git/db/
            target/
          key: ${{{{ runner.os }}}}-cargo-${{{{ hashFiles('**/Cargo.lock') }}}}"#,
                    rust_version
                )
            }
            Some("python") => {
                let python_version = options.python_version.as_deref().unwrap_or("3.11");
                format!(
                    r#"      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: '{}'
          cache: 'pip'

      - name: Install dependencies
        run: pip install -e .[dev]"#,
                    python_version
                )
            }
            Some("go") => {
                let go_version = options.go_version.as_deref().unwrap_or("1.21");
                format!(
                    r#"      - name: Setup Go
        uses: actions/setup-go@v5
        with:
          go-version: '{}'

      - name: Download dependencies
        run: go mod download"#,
                    go_version
                )
            }
            Some("maven") => {
                let java_version = options.java_version.as_deref().unwrap_or("17");
                format!(
                    r#"      - name: Setup Java
        uses: actions/setup-java@v4
        with:
          java-version: '{}'
          distribution: 'temurin'
          cache: 'maven'"#,
                    java_version
                )
            }
            _ => String::new(),
        }
    }

    /// Generate the test step based on package type
    fn generate_test_step(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => r#"      - name: Run tests
        run: npm test

      - name: Run linting
        run: npm run lint"#
                .to_string(),
            Some("cargo") => r#"      - name: Run tests
        run: cargo test

      - name: Run clippy
        run: cargo clippy -- -D warnings

      - name: Check formatting
        run: cargo fmt -- --check"#
                .to_string(),
            Some("python") => r#"      - name: Run tests
        run: pytest

      - name: Run linting
        run: ruff check ."#
                .to_string(),
            Some("go") => r#"      - name: Run tests
        run: go test ./...

      - name: Run linting
        uses: golangci/golangci-lint-action@v4"#
                .to_string(),
            Some("maven") => r#"      - name: Run tests
        run: mvn test

      - name: Verify
        run: mvn verify"#
                .to_string(),
            _ => r#"      - name: Run tests
        run: echo "Add your test command here""#
                .to_string(),
        }
    }

    /// Generate the publish step based on package type
    fn generate_publish_step(&self, options: &TemplateOptions) -> String {
        match options.package_type.as_deref() {
            Some("npm") => r#"      - name: Publish to npm
        run: npm publish
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}"#
                .to_string(),
            Some("cargo") => r#"      - name: Publish to crates.io
        run: cargo publish
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CRATES_IO_TOKEN }}"#
                .to_string(),
            Some("python") => r#"      - name: Build package
        run: python -m build

      - name: Publish to PyPI
        uses: pypa/gh-action-pypi-publish@release/v1
        with:
          password: ${{ secrets.PYPI_TOKEN }}"#
                .to_string(),
            Some("go") => r#"      # Go modules are published via git tags
      - name: Create release
        uses: softprops/action-gh-release@v1
        with:
          generate_release_notes: true"#
                .to_string(),
            Some("maven") => r#"      - name: Deploy to Maven Central
        run: mvn deploy -P release
        env:
          MAVEN_USERNAME: ${{ secrets.MAVEN_USERNAME }}
          MAVEN_PASSWORD: ${{ secrets.MAVEN_PASSWORD }}"#
                .to_string(),
            _ => String::new(),
        }
    }
}

impl CITemplate for GitHubActionsTemplate {
    fn platform_name(&self) -> &'static str {
        "GitHub Actions"
    }

    fn config_path(&self) -> &'static str {
        ".github/workflows/release.yml"
    }

    fn generate(&self, options: &TemplateOptions) -> Result<String> {
        let _project_name = options
            .project_name
            .as_deref()
            .unwrap_or("project");
        let default_branch = &options.default_branch;

        let setup_step = self.generate_setup_step(options);
        let test_step = self.generate_test_step(options);

        let mut workflow = format!(
            r#"name: Release

on:
  push:
    branches:
      - {default_branch}
  pull_request:
    branches:
      - {default_branch}

permissions:
  contents: write
  packages: write

jobs:"#
        );

        // Add CI job for PR checks
        if options.include_pr_checks {
            workflow.push_str(&format!(
                r#"
  ci:
    name: CI
    runs-on: ubuntu-latest
    steps:
      - name: Checkout
        uses: actions/checkout@v4

{setup_step}

{test_step}
"#
            ));
        }

        // Add release job
        if options.include_auto_release {
            let publish_step = if options.include_publish {
                self.generate_publish_step(options)
            } else {
                String::new()
            };

            let changelog_step = if options.include_changelog {
                r#"
      - name: Generate changelog
        run: canaveral changelog --output CHANGELOG.md"#
            } else {
                ""
            };

            workflow.push_str(&format!(
                r#"
  release:
    name: Release
    runs-on: ubuntu-latest
    if: github.event_name == 'push' && github.ref == 'refs/heads/{default_branch}'
    needs: [ci]
    steps:
      - name: Checkout
        uses: actions/checkout@v4
        with:
          fetch-depth: 0
          token: ${{{{ secrets.GITHUB_TOKEN }}}}

{setup_step}

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Calculate version
        id: version
        run: |
          VERSION=$(canaveral version --print)
          echo "version=$VERSION" >> $GITHUB_OUTPUT
{changelog_step}

      - name: Bump version
        run: canaveral version --set ${{{{ steps.version.outputs.version }}}}

      - name: Commit changes
        run: |
          git config user.name "github-actions[bot]"
          git config user.email "github-actions[bot]@users.noreply.github.com"
          git add -A
          git commit -m "chore(release): ${{{{ steps.version.outputs.version }}}}"
          git tag "v${{{{ steps.version.outputs.version }}}}"
          git push --follow-tags

{publish_step}

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: v${{{{ steps.version.outputs.version }}}}
          generate_release_notes: true
"#
            ));
        }

        Ok(workflow)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_npm_workflow() {
        let template = GitHubActionsTemplate::new();
        let options = TemplateOptions::new()
            .with_project_name("my-npm-package")
            .with_package_type("npm");

        let workflow = template.generate(&options).unwrap();

        assert!(workflow.contains("Setup Node.js"));
        assert!(workflow.contains("npm ci"));
        assert!(workflow.contains("npm test"));
        assert!(workflow.contains("NPM_TOKEN"));
    }

    #[test]
    fn test_generate_cargo_workflow() {
        let template = GitHubActionsTemplate::new();
        let options = TemplateOptions::new()
            .with_project_name("my-rust-crate")
            .with_package_type("cargo");

        let workflow = template.generate(&options).unwrap();

        assert!(workflow.contains("Setup Rust"));
        assert!(workflow.contains("cargo test"));
        assert!(workflow.contains("cargo clippy"));
        assert!(workflow.contains("CRATES_IO_TOKEN"));
    }

    #[test]
    fn test_generate_python_workflow() {
        let template = GitHubActionsTemplate::new();
        let options = TemplateOptions::new()
            .with_project_name("my-python-package")
            .with_package_type("python");

        let workflow = template.generate(&options).unwrap();

        assert!(workflow.contains("Setup Python"));
        assert!(workflow.contains("pytest"));
        assert!(workflow.contains("PYPI_TOKEN"));
    }

    #[test]
    fn test_config_path() {
        let template = GitHubActionsTemplate::new();
        assert_eq!(template.config_path(), ".github/workflows/release.yml");
    }
}
