//! CLI definition and command handling

pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

use commands::{
    ChangelogCommand, InitCommand, MetadataCommand, ReleaseCommand, SigningCommand, StatusCommand,
    StoreCommand, ValidateCommand, VersionCommand,
};

/// Canaveral - Universal release management CLI
#[derive(Debug, Parser)]
#[command(name = "canaveral")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Suppress output except errors
    #[arg(short, long, global = true)]
    pub quiet: bool,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    pub format: OutputFormat,

    /// Working directory
    #[arg(short = 'C', long, global = true)]
    pub directory: Option<std::path::PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

/// Output format for CLI
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    #[default]
    Text,
    /// JSON output
    Json,
}

/// Available commands
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize a new Canaveral configuration
    Init(InitCommand),

    /// Calculate the next version
    Version(VersionCommand),

    /// Generate changelog
    Changelog(ChangelogCommand),

    /// Create a new release
    Release(ReleaseCommand),

    /// Show repository status
    Status(StatusCommand),

    /// Validate configuration and repository state
    Validate(ValidateCommand),

    /// Code signing operations
    Signing(SigningCommand),

    /// App store upload operations
    Store(StoreCommand),

    /// App store metadata management
    Metadata(MetadataCommand),
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> anyhow::Result<()> {
        // Change to specified directory if provided
        if let Some(dir) = &self.directory {
            std::env::set_current_dir(dir)?;
        }

        match self.command {
            Commands::Init(ref cmd) => cmd.execute(&self),
            Commands::Version(ref cmd) => cmd.execute(&self),
            Commands::Changelog(ref cmd) => cmd.execute(&self),
            Commands::Release(ref cmd) => cmd.execute(&self),
            Commands::Status(ref cmd) => cmd.execute(&self),
            Commands::Validate(ref cmd) => cmd.execute(&self),
            Commands::Signing(ref cmd) => cmd.execute(&self),
            Commands::Store(ref cmd) => cmd.execute(&self),
            Commands::Metadata(ref cmd) => cmd.execute(&self),
        }
    }
}
