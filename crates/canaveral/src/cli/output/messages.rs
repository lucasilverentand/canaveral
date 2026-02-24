//! Status message methods on Ui

use console::style;

use super::theme;
use super::Ui;

impl Ui {
    /// Print a success message: ✓ message
    pub fn success(&self, message: &str) {
        if self.is_text() {
            println!("{} {}", theme::success_icon(), message);
        }
    }

    /// Print an error message to stderr. Always shown except in JSON mode.
    pub fn error(&self, message: &str) {
        if !self.is_json() {
            eprintln!("{} {}", theme::error_icon(), message);
        }
    }

    /// Print a warning message: ! message
    pub fn warning(&self, message: &str) {
        if self.is_text() {
            println!("{} {}", theme::warning_icon(), message);
        }
    }

    /// Print an info message: → message
    pub fn info(&self, message: &str) {
        if self.is_text() {
            println!("{} {}", theme::info_icon(), message);
        }
    }

    /// Print a step message: ▸ message
    pub fn step(&self, message: &str) {
        if self.is_text() {
            println!("{} {}", style(theme::ICON_STEP).cyan(), message);
        }
    }

    /// Print a hint in dim text
    pub fn hint(&self, message: &str) {
        if self.is_text() {
            println!("  {}", style(message).dim());
        }
    }

    /// Print only when verbose is enabled
    pub fn verbose(&self, message: &str) {
        if self.is_text() && self.is_verbose() {
            println!("  {}", style(message).dim());
        }
    }

    /// Print a blank line
    pub fn blank(&self) {
        if self.is_text() {
            println!();
        }
    }

    /// Print a JSON value (only in JSON mode)
    pub fn json(&self, value: &impl serde::Serialize) -> anyhow::Result<()> {
        if self.is_json() {
            println!("{}", serde_json::to_string_pretty(value)?);
        }
        Ok(())
    }
}
