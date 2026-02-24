//! Structural output: headers, sections, key-value pairs, dividers, lists, badges

use console::style;

use super::theme;
use super::Ui;

impl Ui {
    /// Print a bold header
    pub fn header(&self, text: &str) {
        if self.is_text() {
            println!("{}", style(text).bold());
        }
    }

    /// Print an underlined section heading
    pub fn section(&self, text: &str) {
        if self.is_text() {
            println!("{}", style(text).underlined());
        }
    }

    /// Print an indented key-value pair
    pub fn key_value(&self, key: &str, value: &str) {
        if self.is_text() {
            println!("  {}: {}", style(key).dim(), value);
        }
    }

    /// Print a key with a pre-styled value
    pub fn key_value_styled(
        &self,
        key: &str,
        value: console::StyledObject<impl std::fmt::Display>,
    ) {
        if self.is_text() {
            println!("  {}: {}", style(key).dim(), value);
        }
    }

    /// Print a thin divider line
    pub fn divider(&self) {
        if self.is_text() {
            println!("{}", style("─".repeat(50)).dim());
        }
    }

    /// Print a heavy divider line
    pub fn heavy_divider(&self) {
        if self.is_text() {
            println!("{}", style("═".repeat(70)).dim());
        }
    }

    /// Print a bulleted list
    pub fn list(&self, items: &[&str]) {
        if self.is_text() {
            for item in items {
                println!("  {} {}", style(theme::ICON_BULLET).dim(), item);
            }
        }
    }

    /// Print a badge like `[OK]`, `[WARN]`, `[FAIL]`, `[SKIP]` with inline text.
    pub fn badge_line(&self, badge: BadgeStyle, label: &str, name: &str, detail: &str) {
        if self.is_text() {
            let styled_badge = match badge {
                BadgeStyle::Ok => style(format!("[{label}]")).green(),
                BadgeStyle::Warn => style(format!("[{label}]")).yellow(),
                BadgeStyle::Fail => style(format!("[{label}]")).red(),
                BadgeStyle::Skip => style(format!("[{label}]")).dim(),
            };
            let styled_name = match badge {
                BadgeStyle::Ok => style(name).green(),
                BadgeStyle::Warn => style(name).yellow(),
                BadgeStyle::Fail => style(name).red(),
                BadgeStyle::Skip => style(name).dim(),
            };
            println!("  {} {} {}", styled_badge, styled_name, style(detail).dim());
        }
    }

    /// Format a path for display (cyan)
    pub fn fmt_path(&self, path: &impl std::fmt::Display) -> String {
        if self.is_text() {
            style(path).cyan().to_string()
        } else {
            path.to_string()
        }
    }

    /// Format a version for display (green bold)
    pub fn fmt_version(&self, version: &str) -> String {
        if self.is_text() {
            style(version).green().bold().to_string()
        } else {
            version.to_string()
        }
    }

    /// Format a tag for display (yellow)
    pub fn fmt_tag(&self, tag: &str) -> String {
        if self.is_text() {
            style(tag).yellow().to_string()
        } else {
            tag.to_string()
        }
    }
}

/// Badge style variants
#[derive(Debug, Clone, Copy)]
pub enum BadgeStyle {
    Ok,
    Warn,
    Fail,
    Skip,
}
