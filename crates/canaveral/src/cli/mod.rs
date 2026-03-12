//! CLI definition and command handling

pub mod commands;
pub mod output;

use clap::{Parser, Subcommand};
use std::fmt::Write;
use tracing::info;

use commands::{
    ArchiveCommand, BuildCommand, CICommand, CacheCommand, ChangelogCommand, CheckCommand,
    CompletionsCommand, DoctorCommand, FirebaseCommand, FmtCommand, HooksCommand, InitCommand,
    LintCommand, MatchCommand, MetadataCommand, PrCommand, PublishCommand, ReleaseCommand,
    RunCommand, ScaffoldCommand, ScreenshotsCommand, SigningCommand, StatusCommand, TestCommand,
    TestFlightCommand, ToolsCommand, ValidateCommand, VersionCommand,
};

/// Canaveral - Build, release, and ship software from a single CLI
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

/// Available commands grouped by project lifecycle stage.
#[derive(Debug, Subcommand)]
pub enum Commands {
    // ── Setup ──────────────────────────────────────────────
    /// Scaffold a starter project (Astro, Expo, Hono, etc.)
    Scaffold(ScaffoldCommand),

    /// Initialize a new Canaveral configuration
    Init(InitCommand),

    /// Check environment for required tools and configurations
    Doctor(DoctorCommand),

    /// Manage tool versions (bun, node, etc.)
    Tools(ToolsCommand),

    // ── Develop ───────────────────────────────────────────
    /// Format source code
    Fmt(FmtCommand),

    /// Run linter
    Lint(LintCommand),

    /// Build project for a platform
    Build(BuildCommand),

    /// Archive an iOS/macOS app (Xcode archive + export)
    Archive(ArchiveCommand),

    /// Run tests for a project
    Test(TestCommand),

    /// Run tasks across the workspace
    Run(RunCommand),

    /// Run all checks (fmt, lint, test)
    Check(CheckCommand),

    // ── Code Quality ──────────────────────────────────────
    /// Git hook management (install, uninstall, run, status)
    Hooks(HooksCommand),

    /// Validate configuration and repository state
    Validate(ValidateCommand),

    /// Show repository status
    Status(StatusCommand),

    /// CI pipeline management
    #[command(name = "ci")]
    CI(CICommand),

    /// Pull request validation and preview
    Pr(PrCommand),

    // ── Release ───────────────────────────────────────────
    /// Calculate the next version
    Version(VersionCommand),

    /// Generate changelog
    Changelog(ChangelogCommand),

    /// Create a new release
    Release(ReleaseCommand),

    /// Publish to app stores or package registries
    Publish(PublishCommand),

    // ── Distribute ────────────────────────────────────────
    /// Code signing operations
    Signing(SigningCommand),

    /// Certificate and profile synchronization (match)
    Match(MatchCommand),

    /// TestFlight beta testing management
    TestFlight(TestFlightCommand),

    /// Firebase App Distribution management
    Firebase(FirebaseCommand),

    // ── Store Presence ────────────────────────────────────
    /// App store metadata management
    Metadata(MetadataCommand),

    /// Screenshot capture and framing
    Screenshots(ScreenshotsCommand),

    // ── Utility ───────────────────────────────────────────
    /// Task cache management
    Cache(CacheCommand),

    /// Generate shell completions
    Completions(CompletionsCommand),
}

const COMMAND_GROUPS: &[(&str, &[&str])] = &[
    ("Setup", &["scaffold", "init", "doctor", "tools"]),
    (
        "Develop",
        &["fmt", "lint", "build", "archive", "test", "run", "check"],
    ),
    ("Code Quality", &["hooks", "validate", "status", "ci", "pr"]),
    ("Release", &["version", "changelog", "release", "publish"]),
    (
        "Distribute",
        &["signing", "match", "test-flight", "firebase"],
    ),
    ("Store Presence", &["metadata", "screenshots"]),
    ("Utility", &["cache", "completions"]),
];

/// Build a help template that groups subcommands under lifecycle headings.
pub fn grouped_help_template(cmd: &clap::Command) -> String {
    let mut tpl = String::from("{before-help}{about-with-newline}\n{usage-heading} {usage}\n");

    for (heading, names) in COMMAND_GROUPS {
        let mut section = String::new();
        for name in *names {
            if let Some(sub) = cmd.find_subcommand(name) {
                let about = sub.get_about().map(|s| s.to_string()).unwrap_or_default();
                let _ = writeln!(section, "  {name:<13}{about}");
            }
        }
        if !section.is_empty() {
            let _ = write!(tpl, "\n{heading}:\n{section}");
        }
    }

    tpl.push_str("\n  help         Print this message or the help of the given subcommand(s)\n");
    tpl.push_str("\nOptions:\n{options}\n{after-help}");
    tpl
}

impl Cli {
    /// Execute the CLI command
    pub fn execute(self) -> anyhow::Result<()> {
        // Change to specified directory if provided
        if let Some(dir) = &self.directory {
            info!(directory = %dir.display(), "changing working directory");
            std::env::set_current_dir(dir)?;
        }

        let command_name = match &self.command {
            // Setup
            Commands::Scaffold(_) => "scaffold",
            Commands::Init(_) => "init",
            Commands::Doctor(_) => "doctor",
            Commands::Tools(_) => "tools",
            // Develop
            Commands::Fmt(_) => "fmt",
            Commands::Lint(_) => "lint",
            Commands::Build(_) => "build",
            Commands::Archive(_) => "archive",
            Commands::Test(_) => "test",
            Commands::Run(_) => "run",
            Commands::Check(_) => "check",
            // Code Quality
            Commands::Hooks(_) => "hooks",
            Commands::Validate(_) => "validate",
            Commands::Status(_) => "status",
            Commands::CI(_) => "ci",
            Commands::Pr(_) => "pr",
            // Release
            Commands::Version(_) => "version",
            Commands::Changelog(_) => "changelog",
            Commands::Release(_) => "release",
            Commands::Publish(_) => "publish",
            // Distribute
            Commands::Signing(_) => "signing",
            Commands::Match(_) => "match",
            Commands::TestFlight(_) => "testflight",
            Commands::Firebase(_) => "firebase",
            // Store Presence
            Commands::Metadata(_) => "metadata",
            Commands::Screenshots(_) => "screenshots",
            // Utility
            Commands::Cache(_) => "cache",
            Commands::Completions(_) => "completions",
        };
        info!(
            command = command_name,
            verbose = self.verbose,
            quiet = self.quiet,
            "executing command"
        );

        match self.command {
            // Setup
            Commands::Scaffold(ref cmd) => cmd.execute(&self),
            Commands::Init(ref cmd) => cmd.execute(&self),
            Commands::Doctor(ref cmd) => cmd.execute(&self),
            Commands::Tools(ref cmd) => cmd.execute(&self),
            // Develop
            Commands::Fmt(ref cmd) => cmd.execute(&self),
            Commands::Lint(ref cmd) => cmd.execute(&self),
            Commands::Build(ref cmd) => cmd.execute(&self),
            Commands::Archive(ref cmd) => cmd.execute(&self),
            Commands::Test(ref cmd) => cmd.execute(&self),
            Commands::Run(ref cmd) => cmd.execute(&self),
            Commands::Check(ref cmd) => cmd.execute(&self),
            // Code Quality
            Commands::Hooks(ref cmd) => cmd.execute(&self),
            Commands::Validate(ref cmd) => cmd.execute(&self),
            Commands::Status(ref cmd) => cmd.execute(&self),
            Commands::CI(ref cmd) => cmd.execute(&self),
            Commands::Pr(ref cmd) => cmd.execute(&self),
            // Release
            Commands::Version(ref cmd) => cmd.execute(&self),
            Commands::Changelog(ref cmd) => cmd.execute(&self),
            Commands::Release(ref cmd) => cmd.execute(&self),
            Commands::Publish(ref cmd) => cmd.execute(&self),
            // Distribute
            Commands::Signing(ref cmd) => cmd.execute(&self),
            Commands::Match(ref cmd) => cmd.execute(&self),
            Commands::TestFlight(ref cmd) => cmd.execute(&self),
            Commands::Firebase(ref cmd) => cmd.execute(&self),
            // Store Presence
            Commands::Metadata(ref cmd) => cmd.execute(&self),
            Commands::Screenshots(ref cmd) => cmd.execute(&self),
            // Utility
            Commands::Cache(ref cmd) => cmd.execute(&self),
            Commands::Completions(ref cmd) => cmd.execute(&self),
        }
    }
}
