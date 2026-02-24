//! Themed dialoguer prompt wrappers with non-interactive fallbacks

use super::theme::prompt_theme;
use super::Ui;
use dialoguer::{Confirm, Input, MultiSelect, Select};

impl Ui {
    /// Confirm prompt. Falls back to `default` when non-interactive.
    pub fn confirm(&self, prompt: &str, default: bool) -> anyhow::Result<bool> {
        if !self.is_interactive() {
            return Ok(default);
        }
        Ok(Confirm::with_theme(&prompt_theme())
            .with_prompt(prompt)
            .default(default)
            .interact()?)
    }

    /// Single-select prompt. Falls back to `default` index when non-interactive.
    pub fn select(&self, prompt: &str, items: &[&str], default: usize) -> anyhow::Result<usize> {
        if !self.is_interactive() {
            return Ok(default);
        }
        Ok(Select::with_theme(&prompt_theme())
            .with_prompt(prompt)
            .items(items)
            .default(default)
            .interact()?)
    }

    /// Text input prompt. Falls back to `default` when non-interactive.
    pub fn input(&self, prompt: &str, default: &str) -> anyhow::Result<String> {
        if !self.is_interactive() {
            return Ok(default.to_string());
        }
        Ok(Input::with_theme(&prompt_theme())
            .with_prompt(prompt)
            .default(default.to_string())
            .interact_text()?)
    }

    /// Multi-select prompt. Falls back to empty selection when non-interactive.
    pub fn multi_select(&self, prompt: &str, items: &[&str]) -> anyhow::Result<Vec<usize>> {
        if !self.is_interactive() {
            return Ok(Vec::new());
        }
        Ok(MultiSelect::with_theme(&prompt_theme())
            .with_prompt(prompt)
            .items(items)
            .interact()?)
    }
}
