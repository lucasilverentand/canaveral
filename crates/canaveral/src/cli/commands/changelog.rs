//! Changelog command

use clap::Args;
use console::style;
use tracing::info;

use canaveral_core::config::load_config_or_default;
use canaveral_changelog::ChangelogGenerator;
use canaveral_git::GitRepo;

use crate::cli::{Cli, OutputFormat};

/// Generate changelog
#[derive(Debug, Args)]
pub struct ChangelogCommand {
    /// Version to generate changelog for
    #[arg(long = "for-version", value_name = "VERSION")]
    pub for_version: Option<String>,

    /// Write to file (default: print to stdout)
    #[arg(short, long)]
    pub write: bool,

    /// Output file (defaults to configured changelog file)
    #[arg(short, long)]
    pub output: Option<std::path::PathBuf>,

    /// Include all commits (don't filter by type)
    #[arg(long)]
    pub all: bool,
}

impl ChangelogCommand {
    /// Execute the changelog command
    pub fn execute(&self, cli: &Cli) -> anyhow::Result<()> {
        info!(version = ?self.for_version, write = self.write, all = self.all, "executing changelog command");
        let cwd = std::env::current_dir()?;
        let (config, _) = load_config_or_default(&cwd);

        let repo = GitRepo::discover(&cwd)?;

        // Find the latest tag
        let latest_tag = repo.find_latest_tag(None)?;

        // Determine version
        let version = self.for_version.clone().unwrap_or_else(|| {
            latest_tag
                .as_ref()
                .and_then(|t| t.version.clone())
                .map(|v| format!("{} (unreleased)", v))
                .unwrap_or_else(|| "Unreleased".to_string())
        });

        // Get commits
        let commits = if let Some(tag) = &latest_tag {
            repo.commits_since_tag(&tag.name)?
        } else {
            repo.all_commits()?
        };

        if commits.is_empty() {
            if !cli.quiet {
                println!("{}", style("No commits found since last release.").yellow());
            }
            return Ok(());
        }

        // Generate changelog
        let generator = ChangelogGenerator::new(config.changelog.clone());
        let changelog = generator.generate_formatted(&version, &commits);

        // Output
        if self.write {
            let output_path = self
                .output
                .clone()
                .unwrap_or_else(|| cwd.join(&config.changelog.file));

            // Prepend to existing file or create new
            if output_path.exists() {
                let existing = std::fs::read_to_string(&output_path)?;
                let combined = format!("{}\n{}", changelog, existing);
                std::fs::write(&output_path, combined)?;
            } else {
                std::fs::write(&output_path, &changelog)?;
            }

            if !cli.quiet {
                println!(
                    "{} Changelog written to {}",
                    style("✓").green().bold(),
                    style(output_path.display()).cyan()
                );
            }
        } else {
            match cli.format {
                OutputFormat::Json => {
                    let entry = generator.generate(&version, &commits);
                    println!("{}", serde_json::to_string_pretty(&entry)?);
                }
                OutputFormat::Text => {
                    println!("{}", changelog);
                }
            }
        }

        Ok(())
    }
}
