//! Output formatting utilities

use console::{style, Style};

/// Print a success message
pub fn success(message: &str) {
    println!("{} {}", style("✓").green().bold(), message);
}

/// Print an error message
pub fn error(message: &str) {
    eprintln!("{} {}", style("✗").red().bold(), message);
}

/// Print a warning message
pub fn warning(message: &str) {
    println!("{} {}", style("!").yellow().bold(), message);
}

/// Print an info message
pub fn info(message: &str) {
    println!("{} {}", style("→").blue(), message);
}

/// Create a styled header
pub fn header(text: &str) -> String {
    style(text).bold().to_string()
}

/// Create a styled key-value line
pub fn key_value(key: &str, value: &str) -> String {
    format!("  {}: {}", style(key).dim(), value)
}

/// Style for version numbers
pub fn version_style() -> Style {
    Style::new().green().bold()
}

/// Style for tags
pub fn tag_style() -> Style {
    Style::new().yellow()
}

/// Style for paths
pub fn path_style() -> Style {
    Style::new().cyan()
}
