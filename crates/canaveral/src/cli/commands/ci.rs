//! CI pipeline management command

use clap::{Args, Subcommand};
use console::style;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_core::templates::{CITemplateRegistry, TemplateOptions};

use crate::cli::{Cli, OutputFormat};

/// CI/CD pipeline management
#[derive(Debug, Args)]
pub struct CICommand {
    #[command(subcommand)]
    pub action: CIAction,
}

/// CI subcommands
#[derive(Debug, Subcommand)]
pub enum CIAction {
    /// Generate CI configuration files
    Generate(CIGenerateCommand),
    /// Run CI pipeline locally
    Run(CIRunCommand),
    /// Validate CI configuration
    Validate(CIValidateCommand),
}

/// Generate CI configuration
#[derive(Debug, Args)]
pub struct CIGenerateCommand {
    /// CI platform (github, gitlab)
    #[arg(short, long, default_value = "github")]
    pub platform: String,

    /// Use native mode (thin wrapper calling canaveral)
    #[arg(long)]
    pub native: bool,

    /// Output path (default: auto-detect from platform)
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,

    /// Dry run - print config without writing
    #[arg(long)]
    pub dry_run: bool,
}

/// Run CI pipeline locally
#[derive(Debug, Args)]
pub struct CIRunCommand {
    /// CI event to simulate (pull_request, push, tag)
    #[arg(short, long, default_value = "push")]
    pub event: String,

    /// Base branch for comparison
    #[arg(long, default_value = "main")]
    pub base: String,

    /// Only run on affected packages
    #[arg(long)]
    pub affected: bool,

    /// Dry run - show what would be run
    #[arg(long)]
    pub dry_run: bool,
}

/// Validate CI configuration
#[derive(Debug, Args)]
pub struct CIValidateCommand {
    /// Path to CI config file
    #[arg(short, long)]
    pub config: Option<std::path::PathBuf>,
}

impl CICommand {
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let action_name = match &self.action {
            CIAction::Generate(_) => "generate",
            CIAction::Run(_) => "run",
            CIAction::Validate(_) => "validate",
        };
        info!(action = action_name, "executing ci command");
        match &self.action {
            CIAction::Generate(cmd) => cmd.execute(cli),
            CIAction::Run(cmd) => cmd.execute(cli),
            CIAction::Validate(cmd) => cmd.execute(cli),
        }
    }
}

impl CIGenerateCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        if !cli.quiet {
            println!(
                "{} Generating {} CI configuration...",
                style("→").blue(),
                style(&self.platform).cyan()
            );
        }

        let mut options = TemplateOptions::new();

        if let Some(name) = &config.name {
            options = options.with_project_name(name);
        }

        options.default_branch = config.git.branch.clone();

        // Detect package type
        if let Some(pkg_type) = canaveral_core::templates::detect_package_type(&cwd) {
            options = options.with_package_type(&pkg_type);
        }

        if self.native {
            // Native mode: generate thin wrapper that calls canaveral
            let content = generate_native_workflow(&config, &options)?;

            if self.dry_run {
                println!("{}", content);
            } else {
                let output_path = self.output.clone().unwrap_or_else(|| {
                    cwd.join(".github/workflows/canaveral.yml")
                });
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&output_path, &content)?;

                if !cli.quiet {
                    println!(
                        "{} Generated native CI config at {}",
                        style("✓").green(),
                        style(output_path.display()).cyan()
                    );
                }
            }
        } else {
            // Traditional mode: use existing template generators
            let registry = CITemplateRegistry::new();
            let template = registry.get(&self.platform)
                .ok_or_else(|| anyhow::anyhow!(
                    "Unsupported CI platform: {}. Available: {}",
                    self.platform,
                    registry.platform_names().join(", ")
                ))?;

            let content = template.generate(&options)?;

            if self.dry_run {
                println!("{}", content);
            } else {
                let output_path = self.output.clone().unwrap_or_else(|| {
                    cwd.join(template.config_path())
                });
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&output_path, &content)?;

                if !cli.quiet {
                    println!(
                        "{} Generated {} CI config at {}",
                        style("✓").green(),
                        template.platform_name(),
                        style(output_path.display()).cyan()
                    );
                }
            }
        }

        Ok(())
    }
}

impl CIRunCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let tasks_to_run = match self.event.as_str() {
            "pull_request" | "pr" => &config.ci.on_pr,
            "push" => &config.ci.on_push_main,
            "tag" => &config.ci.on_tag,
            other => anyhow::bail!("Unknown CI event: {}. Use pull_request, push, or tag.", other),
        };

        if !cli.quiet {
            println!(
                "{} CI pipeline for event: {}",
                style("→").blue(),
                style(&self.event).cyan()
            );
            println!("  Tasks: {}", style(tasks_to_run.join(", ")).yellow());
            if self.affected {
                println!("  Mode: {}", style("affected only").dim());
            }
            if self.dry_run {
                println!("  {}", style("[DRY RUN]").yellow().bold());
            }
            println!();
        }

        // In a full implementation, this would invoke the task scheduler
        // For now, display the plan
        if cli.format == OutputFormat::Json {
            let plan = serde_json::json!({
                "event": self.event,
                "tasks": tasks_to_run,
                "affected": self.affected,
                "base": self.base,
                "dry_run": self.dry_run,
            });
            println!("{}", serde_json::to_string_pretty(&plan)?);
        } else if !cli.quiet {
            for task in tasks_to_run {
                println!("  {} {}", style("▸").dim(), task);
            }
            println!();
            println!(
                "{} Run {} to execute these tasks across the workspace.",
                style("→").blue(),
                style(format!("canaveral run {}", tasks_to_run.join(" "))).cyan()
            );
        }

        Ok(())
    }
}

impl CIValidateCommand {
    fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let (config, config_path) = load_config_or_default(&cwd);

        if !cli.quiet {
            println!("{} Validating CI configuration...", style("→").blue());
        }

        let mut issues = Vec::new();

        // Check if CI config exists
        if config.ci.on_pr.is_empty() && config.ci.on_push_main.is_empty() {
            issues.push("No CI tasks configured for any event".to_string());
        }

        // Check for CI workflow file
        let workflow_path = match config.ci.platform.as_str() {
            "github" => cwd.join(".github/workflows"),
            "gitlab" => cwd.join(".gitlab-ci.yml"),
            _ => cwd.join(".ci"),
        };

        if !workflow_path.exists() {
            issues.push(format!(
                "CI workflow path does not exist: {}",
                workflow_path.display()
            ));
        }

        if issues.is_empty() {
            if !cli.quiet {
                println!("{} CI configuration is valid.", style("✓").green());
                if let Some(path) = config_path {
                    println!("  Config: {}", style(path.display()).dim());
                }
                println!("  Platform: {}", style(&config.ci.platform).cyan());
                println!(
                    "  PR tasks: {}",
                    style(config.ci.on_pr.join(", ")).dim()
                );
                println!(
                    "  Push tasks: {}",
                    style(config.ci.on_push_main.join(", ")).dim()
                );
            }
        } else {
            for issue in &issues {
                println!("  {} {}", style("✗").red(), issue);
            }
            if !issues.is_empty() {
                println!();
                println!(
                    "{} Run {} to generate a CI configuration.",
                    style("→").blue(),
                    style("canaveral ci generate").cyan()
                );
            }
        }

        Ok(())
    }
}

/// Generate a native-mode CI workflow that delegates to canaveral
fn generate_native_workflow(
    config: &canaveral_core::config::Config,
    options: &TemplateOptions,
) -> anyhow::Result<String> {
    let branch = &options.default_branch;
    let pr_tasks = config.ci.on_pr.join(" ");
    let push_tasks = config.ci.on_push_main.join(" ");
    let tag_tasks = config.ci.on_tag.join(" ");

    let workflow = format!(
        r#"# Generated by canaveral ci generate --native
# This is a thin wrapper that delegates all logic to canaveral.

name: Canaveral CI

on:
  push:
    branches: [{branch}]
    tags: ['v*']
  pull_request:
    branches: [{branch}]

permissions:
  contents: write
  packages: write

jobs:
  ci:
    name: Canaveral CI
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0

      - name: Install Canaveral
        run: cargo install canaveral

      - name: Run CI
        run: |
          if [ "${{{{ github.event_name }}}}" = "pull_request" ]; then
            canaveral ci run --event=pull_request --affected --base=${{{{ github.base_ref }}}}
          elif [[ "${{{{ github.ref }}}}" == refs/tags/* ]]; then
            canaveral ci run --event=tag
          else
            canaveral ci run --event=push
          fi
        env:
          GITHUB_TOKEN: ${{{{ secrets.GITHUB_TOKEN }}}}

# Task configuration (from canaveral.toml):
#   PR tasks: {pr_tasks}
#   Push tasks: {push_tasks}
#   Tag tasks: {tag_tasks}
"#
    );

    Ok(workflow)
}
