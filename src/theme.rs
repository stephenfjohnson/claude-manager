use ratatui::style::{Color, Modifier, Style};

// Base colors
pub const FG: Color = Color::Gray;
pub const FG_DIM: Color = Color::DarkGray;
pub const ACCENT: Color = Color::Cyan;

// Status colors
pub const STATUS_RUNNING: Color = Color::Green;
pub const STATUS_STOPPED: Color = Color::DarkGray;
pub const STATUS_PORT: Color = Color::Cyan;

// Git colors
pub const GIT_DIRTY: Color = Color::Yellow;
pub const GIT_AHEAD: Color = Color::Yellow;
pub const GIT_BEHIND: Color = Color::Red;
pub const GIT_CLEAN: Color = Color::Green;

// Border colors
pub const BORDER_ACTIVE: Color = Color::Cyan;
pub const BORDER_INACTIVE: Color = Color::DarkGray;

// Semantic colors
pub const DANGER: Color = Color::Red;
pub const WARNING: Color = Color::Yellow;

// Prebuilt styles
pub fn accent_title() -> Style {
    Style::default().fg(ACCENT)
}

pub fn label() -> Style {
    Style::default().fg(FG_DIM)
}

pub fn active_border() -> Style {
    Style::default().fg(BORDER_ACTIVE)
}

pub fn inactive_border() -> Style {
    Style::default().fg(BORDER_INACTIVE)
}

pub fn highlight() -> Style {
    Style::default()
        .bg(Color::Rgb(40, 44, 52))
        .add_modifier(Modifier::BOLD)
}

pub fn status_running() -> Style {
    Style::default().fg(STATUS_RUNNING)
}

pub fn status_stopped() -> Style {
    Style::default().fg(STATUS_STOPPED)
}
