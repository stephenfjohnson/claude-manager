use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::theme;

pub struct InputDialog {
    pub title: String,
    pub value: String,
    pub visible: bool,
    /// Hint text shown when input is empty (e.g. current value or "not set")
    pub hint: Option<String>,
}

impl InputDialog {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            value: String::new(),
            visible: false,
            hint: None,
        }
    }

    pub fn show(&mut self) {
        self.value.clear();
        self.visible = true;
    }

    pub fn set_value(&mut self, value: &str) {
        self.value = value.to_string();
    }

    pub fn set_hint(&mut self, hint: &str) {
        self.hint = Some(hint.to_string());
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.hint = None;
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Option<String> {
        match key {
            KeyCode::Enter => {
                let val = self.value.clone();
                self.hide();
                Some(val)
            }
            KeyCode::Esc => {
                self.hide();
                None
            }
            KeyCode::Backspace => {
                self.value.pop();
                None
            }
            KeyCode::Char(c) => {
                self.value.push(c);
                None
            }
            _ => None,
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        // Center the dialog
        let width = 60.min(area.width.saturating_sub(4));
        let height = if self.hint.is_some() && self.value.is_empty() { 4 } else { 3 };
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let dialog_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, dialog_area);

        let lines: Vec<Line> = if self.value.is_empty() {
            if let Some(ref hint) = self.hint {
                vec![
                    Line::from(Span::styled(
                        hint.as_str(),
                        Style::default().fg(theme::FG_DIM),
                    )),
                    Line::from(Span::styled("_", Style::default().fg(Color::White))),
                ]
            } else {
                vec![Line::from(Span::styled("_", Style::default().fg(Color::White)))]
            }
        } else {
            vec![Line::from(Span::styled(
                self.value.as_str(),
                Style::default().fg(Color::White),
            ))]
        };

        let input = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" {} ", self.title))
                .title_style(theme::accent_title())
                .border_style(theme::active_border()),
        );
        frame.render_widget(input, dialog_area);
    }
}
