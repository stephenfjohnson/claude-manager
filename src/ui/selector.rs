use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState},
    Frame,
};

pub struct RepoSelector {
    pub visible: bool,
    pub repos: Vec<(String, String)>, // (name, url)
    pub state: ListState,
    pub filter: String,
}

impl RepoSelector {
    pub fn new() -> Self {
        Self {
            visible: false,
            repos: Vec::new(),
            state: ListState::default(),
            filter: String::new(),
        }
    }

    pub fn show(&mut self, repos: Vec<(String, String)>) {
        self.repos = repos;
        self.visible = true;
        self.filter.clear();
        self.state.select(Some(0));
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.repos.clear();
        self.filter.clear();
    }

    fn filtered_repos(&self) -> Vec<&(String, String)> {
        if self.filter.is_empty() {
            self.repos.iter().collect()
        } else {
            let filter_lower = self.filter.to_lowercase();
            self.repos
                .iter()
                .filter(|(name, _)| name.to_lowercase().contains(&filter_lower))
                .collect()
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) -> Option<(String, String)> {
        let filtered = self.filtered_repos();
        let len = filtered.len();

        match key {
            KeyCode::Enter => {
                if let Some(idx) = self.state.selected() {
                    if idx < len {
                        let (name, url) = filtered[idx].clone();
                        self.hide();
                        return Some((name, url));
                    }
                }
                None
            }
            KeyCode::Esc => {
                self.hide();
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if len > 0 {
                    let idx = self.state.selected().unwrap_or(0);
                    let new_idx = if idx == 0 { len - 1 } else { idx - 1 };
                    self.state.select(Some(new_idx));
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    let idx = self.state.selected().unwrap_or(0);
                    let new_idx = if idx >= len - 1 { 0 } else { idx + 1 };
                    self.state.select(Some(new_idx));
                }
                None
            }
            KeyCode::Backspace => {
                self.filter.pop();
                // Reset selection when filter changes
                if !self.filtered_repos().is_empty() {
                    self.state.select(Some(0));
                }
                None
            }
            KeyCode::Char(c) => {
                // Only filter on alphanumeric and common chars, not j/k when used for navigation
                if c != 'j' && c != 'k' {
                    self.filter.push(c);
                    // Reset selection when filter changes
                    if !self.filtered_repos().is_empty() {
                        self.state.select(Some(0));
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        if !self.visible {
            return;
        }

        let filtered = self.filtered_repos();

        // Make dialog larger to show more repos
        let width = 70.min(area.width.saturating_sub(4));
        let height = 20.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let dialog_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, dialog_area);

        let items: Vec<ListItem> = filtered
            .iter()
            .map(|(name, _url)| {
                ListItem::new(Line::from(vec![Span::raw(name.clone())]))
            })
            .collect();

        let title = if self.filter.is_empty() {
            " Select Repository (type to filter) ".to_string()
        } else {
            format!(" Filter: {} ", self.filter)
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::Cyan)),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("> ");

        frame.render_stateful_widget(list, dialog_area, &mut self.state);
    }
}
