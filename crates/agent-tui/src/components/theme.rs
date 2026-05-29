//! Shared TUI theme tokens.
//!
//! The terminal UI mirrors the desktop GUI design language: cool neutral
//! surfaces, blue/cyan focus, green success, amber warning, and rose danger.

use ratatui::style::{Color, Modifier, Style};

pub const ACCENT: Color = Color::Rgb(96, 165, 250);
pub const ACCENT_STRONG: Color = Color::Rgb(34, 211, 238);
pub const SUCCESS: Color = Color::Rgb(52, 211, 153);
pub const WARNING: Color = Color::Rgb(251, 191, 36);
pub const DANGER: Color = Color::Rgb(251, 113, 133);
pub const INFO: Color = Color::Rgb(34, 211, 238);
pub const MUTED: Color = Color::Rgb(133, 148, 171);
pub const BORDER: Color = Color::Rgb(64, 81, 107);
pub const SURFACE_SELECTED: Color = Color::Rgb(29, 59, 99);
pub const TEXT_INVERTED: Color = Color::Rgb(11, 18, 32);

pub fn border(focused: bool) -> Style {
    if focused {
        Style::default().fg(ACCENT)
    } else {
        Style::default().fg(BORDER)
    }
}

pub fn title() -> Style {
    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn muted() -> Style {
    Style::default().fg(MUTED)
}

pub fn key() -> Style {
    Style::default().fg(WARNING).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(SURFACE_SELECTED)
        .add_modifier(Modifier::BOLD)
}

pub fn badge(bg: Color) -> Style {
    Style::default()
        .bg(bg)
        .fg(TEXT_INVERTED)
        .add_modifier(Modifier::BOLD)
}
