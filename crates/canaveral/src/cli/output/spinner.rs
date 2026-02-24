//! Spinner and progress bar wrapper around indicatif

use std::time::Duration;

use indicatif::{ProgressBar, ProgressStyle};

use super::Ui;

/// Wrapper around an optional `ProgressBar`. No-op when `None` (quiet/JSON mode).
pub struct Spinner {
    bar: Option<ProgressBar>,
}

impl Spinner {
    fn new(bar: Option<ProgressBar>) -> Self {
        Self { bar }
    }

    /// Update the spinner message.
    pub fn set_message(&self, msg: impl Into<std::borrow::Cow<'static, str>>) {
        if let Some(bar) = &self.bar {
            bar.set_message(msg);
        }
    }

    /// Finish with a success message.
    pub fn finish(&self, msg: &str) {
        if let Some(bar) = &self.bar {
            bar.finish_with_message(format!("✓ {msg}"));
        }
    }

    /// Finish with a failure message.
    pub fn fail(&self, msg: &str) {
        if let Some(bar) = &self.bar {
            bar.finish_with_message(format!("✗ {msg}"));
        }
    }

    /// Clear the spinner without a final message.
    pub fn finish_and_clear(&self) {
        if let Some(bar) = &self.bar {
            bar.finish_and_clear();
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(bar) = self.bar.take() {
            if !bar.is_finished() {
                bar.finish_and_clear();
            }
        }
    }
}

impl Ui {
    /// Create a braille spinner with a message. No-op in quiet/JSON mode.
    pub fn spinner(&self, message: &str) -> Spinner {
        if !self.is_text() {
            return Spinner::new(None);
        }
        let bar = ProgressBar::new_spinner();
        bar.set_style(
            ProgressStyle::default_spinner()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "])
                .template("{spinner} {msg}")
                .expect("valid template"),
        );
        bar.set_message(message.to_string());
        bar.enable_steady_tick(Duration::from_millis(80));
        Spinner::new(Some(bar))
    }

    /// Create a progress bar with a total count. No-op in quiet/JSON mode.
    pub fn progress(&self, total: u64, message: &str) -> Spinner {
        if !self.is_text() {
            return Spinner::new(None);
        }
        let bar = ProgressBar::new(total);
        bar.set_style(
            ProgressStyle::default_bar()
                .template("{msg} [{bar:30.cyan/dim}] {pos}/{len}")
                .expect("valid template")
                .progress_chars("━╸─"),
        );
        bar.set_message(message.to_string());
        Spinner::new(Some(bar))
    }
}
