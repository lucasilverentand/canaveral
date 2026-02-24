//! Centralized CLI output, prompts, and progress

mod messages;
mod prompts;
mod spinner;
mod structure;
mod theme;

pub use spinner::Spinner;
pub use structure::BadgeStyle;
pub use theme::prompt_theme;

use crate::cli::{Cli, OutputFormat};

/// Output mode derived from CLI flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Text,
    Quiet,
    Json,
}

/// Centralized UI context for all CLI output.
///
/// Cheap value type — construct at the top of each `execute()`.
/// All output methods are no-ops when the mode doesn't match.
pub struct Ui {
    mode: OutputMode,
    verbose: bool,
}

impl Ui {
    /// Create from CLI flags. Priority: JSON > quiet > text.
    pub fn new(cli: &Cli) -> Self {
        let mode = if cli.format == OutputFormat::Json {
            OutputMode::Json
        } else if cli.quiet {
            OutputMode::Quiet
        } else {
            OutputMode::Text
        };
        Self {
            mode,
            verbose: cli.verbose,
        }
    }

    pub fn is_text(&self) -> bool {
        self.mode == OutputMode::Text
    }

    pub fn is_json(&self) -> bool {
        self.mode == OutputMode::Json
    }

    pub fn is_quiet(&self) -> bool {
        self.mode == OutputMode::Quiet
    }

    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    /// True when interactive prompts are safe: text mode + TTY.
    pub fn is_interactive(&self) -> bool {
        self.mode == OutputMode::Text && console::Term::stderr().is_term()
    }
}
