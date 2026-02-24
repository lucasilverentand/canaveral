//! Visual theme: icons, styles, dialoguer theme

use console::{style, Style, StyledObject};
use dialoguer::theme::ColorfulTheme;

// ── Icons ──────────────────────────────────────────────────────────

pub const ICON_SUCCESS: &str = "✓";
pub const ICON_ERROR: &str = "✗";
pub const ICON_WARNING: &str = "!";
pub const ICON_INFO: &str = "→";
pub const ICON_STEP: &str = "▸";
pub const ICON_BULLET: &str = "•";

// ── Styled icons ───────────────────────────────────────────────────

pub fn success_icon() -> StyledObject<&'static str> {
    style(ICON_SUCCESS).green().bold()
}

pub fn error_icon() -> StyledObject<&'static str> {
    style(ICON_ERROR).red().bold()
}

pub fn warning_icon() -> StyledObject<&'static str> {
    style(ICON_WARNING).yellow().bold()
}

pub fn info_icon() -> StyledObject<&'static str> {
    style(ICON_INFO).blue()
}

// ── Dialoguer theme ────────────────────────────────────────────────

pub fn prompt_theme() -> ColorfulTheme {
    ColorfulTheme {
        defaults_style: Style::new().dim(),
        prompt_style: Style::new().bold(),
        prompt_prefix: style("?".to_string()).cyan().bold(),
        success_prefix: style(ICON_SUCCESS.to_string()).green().bold(),
        error_prefix: style(ICON_ERROR.to_string()).red().bold(),
        active_item_style: Style::new().cyan().bold(),
        inactive_item_style: Style::new(),
        picked_item_prefix: style("✓ ".to_string()).green(),
        unpicked_item_prefix: style("○ ".to_string()).dim(),
        ..ColorfulTheme::default()
    }
}
