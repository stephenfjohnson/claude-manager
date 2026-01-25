use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::path::Path;

use crate::db::{Database, MachineLocation, Project};
use crate::detect;
use crate::process::ProcessManager;
use crate::sync;
use crate::ui::input::InputDialog;

#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    AddName,
    AddUrl,
    SetPath,
}

pub struct App {
    pub projects: Vec<Project>,
    pub machine_id: String,
    pub db: Database,
    pub list_state: ListState,
    pub selected_location: Option<MachineLocation>,
    pub selected_detection: Option<detect::DetectedProject>,
    // Input dialogs
    input_mode: InputMode,
    name_input: InputDialog,
    url_input: InputDialog,
    path_input: InputDialog,
    pending_name: Option<String>,
    // Process management
    pub process_manager: ProcessManager,
    show_logs: bool,
}

impl App {
    pub fn new(db: Database, machine_id: String) -> anyhow::Result<Self> {
        let projects = db.list_projects()?;
        let mut list_state = ListState::default();
        if !projects.is_empty() {
            list_state.select(Some(0));
        }

        let mut app = Self {
            projects,
            machine_id,
            db,
            list_state,
            selected_location: None,
            selected_detection: None,
            input_mode: InputMode::Normal,
            name_input: InputDialog::new("Project Name"),
            url_input: InputDialog::new("GitHub URL"),
            path_input: InputDialog::new("Local Path"),
            pending_name: None,
            process_manager: ProcessManager::new(),
            show_logs: true,
        };
        app.update_selected_details();
        Ok(app)
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::AddName => {
                if let Some(name) = self.name_input.handle_key(key) {
                    if !name.is_empty() {
                        self.pending_name = Some(name);
                        self.url_input.show();
                        self.input_mode = InputMode::AddUrl;
                    } else {
                        self.input_mode = InputMode::Normal;
                    }
                }
                if !self.name_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::AddUrl => {
                if let Some(url) = self.url_input.handle_key(key) {
                    if let Some(name) = self.pending_name.take() {
                        if !url.is_empty() {
                            self.add_project(&name, &url);
                        }
                    }
                    self.input_mode = InputMode::Normal;
                }
                if !self.url_input.visible {
                    self.input_mode = InputMode::Normal;
                    self.pending_name = None;
                }
            }
            InputMode::SetPath => {
                if let Some(path) = self.path_input.handle_key(key) {
                    if !path.is_empty() {
                        self.set_path(&path);
                    }
                    self.input_mode = InputMode::Normal;
                }
                if !self.path_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
        }
    }

    fn handle_normal_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Char('a') => {
                self.name_input.show();
                self.input_mode = InputMode::AddName;
            }
            KeyCode::Char('p') => {
                if self.selected_project().is_some() {
                    self.path_input.show();
                    self.input_mode = InputMode::SetPath;
                }
            }
            KeyCode::Char('d') => self.delete_selected(),
            KeyCode::Char('r') => self.run_selected(),
            KeyCode::Char('s') => self.stop_selected(),
            _ => {}
        }
    }

    fn add_project(&mut self, name: &str, url: &str) {
        if let Ok(id) = self.db.add_project(name, url) {
            let _ = sync::push(&format!("Add project: {}", name));
            self.projects = self.db.list_projects().unwrap_or_default();
            // Select the new project
            if let Some(idx) = self.projects.iter().position(|p| p.id == id) {
                self.list_state.select(Some(idx));
            }
            self.update_selected_details();
        }
    }

    fn set_path(&mut self, path: &str) {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;
            let project_name = project.name.clone();
            if self
                .db
                .set_location(project_id, &self.machine_id, path)
                .is_ok()
            {
                let _ = sync::push(&format!(
                    "Set path for {} on {}",
                    project_name, self.machine_id
                ));
                self.update_selected_details();
            }
        }
    }

    fn delete_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let id = project.id;
            let name = project.name.clone();
            if self.db.delete_project(id).is_ok() {
                let _ = sync::push(&format!("Delete project: {}", name));
                self.projects = self.db.list_projects().unwrap_or_default();
                if self.projects.is_empty() {
                    self.list_state.select(None);
                } else if let Some(idx) = self.list_state.selected() {
                    if idx >= self.projects.len() {
                        self.list_state.select(Some(self.projects.len() - 1));
                    }
                }
                self.update_selected_details();
            }
        }
    }

    fn run_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;

            if self.process_manager.is_running(project_id) {
                return; // Already running
            }

            if let Some(ref loc) = self.selected_location {
                let path = Path::new(&loc.path);

                // Determine command: override > detected > none
                let cmd = loc
                    .run_command
                    .clone()
                    .or_else(|| self.selected_detection.as_ref().and_then(|d| d.run_command.clone()));

                if let Some(cmd) = cmd {
                    let _ = self.process_manager.start(project_id, path, &cmd);
                }
            }
        }
    }

    fn stop_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let _ = self.process_manager.stop(project.id);
        }
    }

    pub fn is_input_mode(&self) -> bool {
        self.input_mode != InputMode::Normal
    }

    fn next(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.projects.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_selected_details();
    }

    fn previous(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.projects.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_selected_details();
    }

    fn update_selected_details(&mut self) {
        self.selected_location = None;
        self.selected_detection = None;

        if let Some(idx) = self.list_state.selected() {
            if let Some(project) = self.projects.get(idx) {
                self.selected_location = self
                    .db
                    .get_location(project.id, &self.machine_id)
                    .ok()
                    .flatten();

                if let Some(ref loc) = self.selected_location {
                    self.selected_detection = detect::detect(Path::new(&loc.path)).ok();
                }
            }
        }
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.list_state
            .selected()
            .and_then(|i| self.projects.get(i))
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let has_running = !self.process_manager.running_projects().is_empty();

        let main_chunks = if has_running && self.show_logs {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(10),
                    Constraint::Length(1),
                ])
                .split(frame.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(frame.area())
        };

        let content_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(main_chunks[0]);

        self.render_project_list(frame, content_chunks[0]);
        self.render_details(frame, content_chunks[1]);

        if has_running && self.show_logs {
            self.render_logs(frame, main_chunks[1]);
            self.render_help_bar(frame, main_chunks[2]);
        } else {
            self.render_help_bar(frame, main_chunks[1]);
        }

        // Render input dialogs on top
        let area = frame.area();
        self.name_input.render(frame, area);
        self.url_input.render(frame, area);
        self.path_input.render(frame, area);
    }

    fn render_logs(&mut self, frame: &mut Frame, area: Rect) {
        let project_id = self.selected_project().map(|p| p.id);
        let project_name = self.selected_project().map(|p| p.name.clone());

        let (title, lines) = if let Some(id) = project_id {
            if self.process_manager.is_running(id) {
                let output = self.process_manager.get_output(id);
                let title = format!(" Logs ({}) ", project_name.unwrap_or_default());
                (title, output)
            } else {
                (" Logs ".to_string(), vec!["No process running".to_string()])
            }
        } else {
            (" Logs ".to_string(), vec!["Select a project".to_string()])
        };

        // Show last N lines that fit
        let available_lines = (area.height as usize).saturating_sub(2);
        let start = lines.len().saturating_sub(available_lines);
        let visible_lines: Vec<Line> = lines[start..]
            .iter()
            .map(|s| Line::from(s.as_str()))
            .collect();

        let para = Paragraph::new(visible_lines)
            .block(Block::default().borders(Borders::ALL).title(title));
        frame.render_widget(para, area);
    }

    fn render_help_bar(&self, frame: &mut Frame, area: Rect) {
        let help_text = " [a]dd  [p]ath  [r]un  [s]top  [d]elete  [q]uit ";
        let machine_text = format!(" Machine: {} ", self.machine_id);

        let help = Paragraph::new(Line::from(vec![
            Span::styled(help_text, Style::default().fg(Color::DarkGray)),
            Span::raw(" | "),
            Span::styled(machine_text, Style::default().fg(Color::Cyan)),
        ]));

        frame.render_widget(help, area);
    }

    fn render_project_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .projects
            .iter()
            .map(|p| {
                let has_path = self
                    .db
                    .get_location(p.id, &self.machine_id)
                    .ok()
                    .flatten()
                    .is_some();
                let is_running = self.process_manager.is_running(p.id);

                let status = if is_running {
                    Span::styled(" * ", Style::default().fg(Color::Green))
                } else if has_path {
                    Span::styled(" + ", Style::default().fg(Color::Green))
                } else {
                    Span::styled(" - ", Style::default().fg(Color::Red))
                };

                let line = Line::from(vec![status, Span::raw(&p.name)]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title(" Projects "))
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

        frame.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let content = if let Some(project) = self.selected_project() {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&project.name),
                ]),
                Line::from(vec![
                    Span::styled("Repo: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&project.repo_url),
                ]),
                Line::from(""),
            ];

            if let Some(ref loc) = self.selected_location {
                lines.push(Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&loc.path),
                ]));

                if let Some(ref det) = self.selected_detection {
                    if let Some(pm) = det.package_manager {
                        lines.push(Line::from(vec![
                            Span::styled("Type: ", Style::default().fg(Color::DarkGray)),
                            Span::raw(format!("{:?} ({})", det.project_type, pm.as_str())),
                        ]));
                    }
                    if let Some(ref cmd) = det.run_command {
                        lines.push(Line::from(vec![
                            Span::styled("Run:  ", Style::default().fg(Color::DarkGray)),
                            Span::raw(cmd),
                        ]));
                    }
                }

                if let Some(ref cmd) = loc.run_command {
                    lines.push(Line::from(vec![
                        Span::styled("Override: ", Style::default().fg(Color::Yellow)),
                        Span::raw(cmd),
                    ]));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "Path not set on this machine",
                    Style::default().fg(Color::Red),
                )));
                lines.push(Line::from(Span::styled(
                    "Press 'p' to set path",
                    Style::default().fg(Color::DarkGray),
                )));
            }

            lines
        } else {
            vec![Line::from("No project selected")]
        };

        let para = Paragraph::new(content)
            .block(Block::default().borders(Borders::ALL).title(" Details "));
        frame.render_widget(para, area);
    }
}
