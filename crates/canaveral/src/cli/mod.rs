//! CLI definition and command handling

pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};

use commands::{
    BuildCommand, ChangelogCommand, CompletionsCommand, DoctorCommand, FirebaseCommand,
    InitCommand, MatchCommand, MetadataCommand, PublishCommand, ReleaseCommand, ScreenshotsCommand,
    SigningCommand, StatusCommand, TestCommand, TestFlightCommand, ValidateCommand,
    VersionCommand,
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

    /// Publish to app stores or package registries
    Publish(PublishCommand),

    /// App store metadata management
    Metadata(MetadataCommand),

    /// Build project for a platform
    Build(BuildCommand),

    /// Generate shell completions
    Completions(CompletionsCommand),

    /// Check environment for required tools and configurations
    Doctor(DoctorCommand),

    /// TestFlight beta testing management
    TestFlight(TestFlightCommand),

    /// Firebase App Distribution management
    Firebase(FirebaseCommand),

    /// Run tests for a project
    Test(TestCommand),

    /// Screenshot capture and framing
    Screenshots(ScreenshotsCommand),

    /// Certificate and profile synchronization (match)
    Match(MatchCommand),
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
            Commands::Publish(ref cmd) => cmd.execute(&self),
            Commands::Metadata(ref cmd) => cmd.execute(&self),
            Commands::Build(ref cmd) => cmd.execute(&self),
            Commands::Completions(ref cmd) => cmd.execute(&self),
            Commands::Doctor(ref cmd) => cmd.execute(&self),
            Commands::TestFlight(ref cmd) => cmd.execute(&self),
            Commands::Firebase(ref cmd) => cmd.execute(&self),
            Commands::Test(ref cmd) => cmd.execute(&self),
            Commands::Screenshots(ref cmd) => cmd.execute(&self),
            Commands::Match(ref cmd) => cmd.execute(&self),
        }
    }
}
