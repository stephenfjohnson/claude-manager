use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::path::Path;

use crate::config::MachineConfig;
use crate::db::{Database, MachineLocation, Project};
use crate::detect;
use crate::gh;
use crate::git_status::{self, GitStatus};
use crate::ports::{self, PortInfo};
use crate::process::ProcessManager;
use crate::sync;
use crate::ui::input::InputDialog;
use crate::ui::selector::RepoSelector;

#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    SelectRepo,
    SetPath,
    EditRunCmd,
    ImportPath,
    SetInstallDir,
    ClonePath,
    ConfirmQuit,
}

pub struct App {
    pub projects: Vec<Project>,
    pub machine_id: String,
    pub db: Database,
    pub list_state: ListState,
    pub selected_location: Option<MachineLocation>,
    pub selected_detection: Option<detect::DetectedProject>,
    pub selected_git_status: Option<GitStatus>,
    // Input dialogs
    input_mode: InputMode,
    path_input: InputDialog,
    run_cmd_input: InputDialog,
    import_path_input: InputDialog,
    install_dir_input: InputDialog,
    clone_path_input: InputDialog,
    repo_selector: RepoSelector,
    // Process management
    pub process_manager: ProcessManager,
    show_logs: bool,
    // Port scanning
    pub port_info: Vec<PortInfo>,
    last_port_scan: std::time::Instant,
    // Machine config
    pub config: MachineConfig,
    // Quit state
    should_quit: bool,
}

impl App {
    pub fn new(db: Database, machine_id: String) -> anyhow::Result<Self> {
        let projects = db.list_projects()?;
        let mut list_state = ListState::default();
        if !projects.is_empty() {
            list_state.select(Some(0));
        }

        let config = MachineConfig::load().unwrap_or_default();

        let mut app = Self {
            projects,
            machine_id,
            db,
            list_state,
            selected_location: None,
            selected_detection: None,
            selected_git_status: None,
            input_mode: InputMode::Normal,
            path_input: InputDialog::new("Local Path"),
            run_cmd_input: InputDialog::new("Run Command"),
            import_path_input: InputDialog::new("Import Path"),
            install_dir_input: InputDialog::new("Install Directory"),
            clone_path_input: InputDialog::new("Clone to Directory"),
            repo_selector: RepoSelector::new(),
            process_manager: ProcessManager::new(),
            show_logs: true,
            port_info: ports::scan_ports(),
            last_port_scan: std::time::Instant::now(),
            config,
            should_quit: false,
        };
        app.update_selected_details();
        Ok(app)
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn request_quit(&mut self) {
        self.input_mode = InputMode::ConfirmQuit;
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match self.input_mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::SelectRepo => {
                if let Some((name, url)) = self.repo_selector.handle_key(key) {
                    self.add_project(&name, &url);
                }
                if !self.repo_selector.visible {
                    self.input_mode = InputMode::Normal;
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
            InputMode::EditRunCmd => {
                if let Some(cmd) = self.run_cmd_input.handle_key(key) {
                    self.set_run_command(if cmd.is_empty() { None } else { Some(&cmd) });
                    self.input_mode = InputMode::Normal;
                }
                if !self.run_cmd_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::ImportPath => {
                if let Some(path) = self.import_path_input.handle_key(key) {
                    if !path.is_empty() {
                        self.import_from_path(&path);
                    }
                    self.input_mode = InputMode::Normal;
                }
                if !self.import_path_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::SetInstallDir => {
                if let Some(dir) = self.install_dir_input.handle_key(key) {
                    let dir_opt = if dir.is_empty() { None } else { Some(dir) };
                    let _ = self.config.set_install_dir(dir_opt);
                    self.input_mode = InputMode::Normal;
                }
                if !self.install_dir_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::ClonePath => {
                if let Some(dir) = self.clone_path_input.handle_key(key) {
                    if !dir.is_empty() {
                        let path = if dir.starts_with("~/") {
                            if let Some(home) = dirs::home_dir() {
                                home.join(&dir[2..])
                            } else {
                                std::path::PathBuf::from(&dir)
                            }
                        } else {
                            std::path::PathBuf::from(&dir)
                        };
                        self.clone_selected_to(path);
                    }
                    self.input_mode = InputMode::Normal;
                }
                if !self.clone_path_input.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::ConfirmQuit => match key {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.should_quit = true;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                _ => {}
            },
        }
    }

    pub fn handle_paste(&mut self, text: &str) {
        match self.input_mode {
            InputMode::SetPath => self.path_input.value.push_str(text),
            InputMode::EditRunCmd => self.run_cmd_input.value.push_str(text),
            InputMode::ImportPath => self.import_path_input.value.push_str(text),
            InputMode::SetInstallDir => self.install_dir_input.value.push_str(text),
            InputMode::ClonePath => self.clone_path_input.value.push_str(text),
            _ => {}
        }
    }

    fn handle_normal_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            KeyCode::Char('a') => {
                // Fetch repos from GitHub and show selector
                if let Ok(repos) = gh::list_repos() {
                    self.repo_selector.show(repos);
                    self.input_mode = InputMode::SelectRepo;
                }
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
            KeyCode::Char('e') => {
                if self.selected_location.is_some() {
                    self.run_cmd_input.show();
                    self.input_mode = InputMode::EditRunCmd;
                }
            }
            KeyCode::Char('i') => {
                self.import_path_input.show();
                self.input_mode = InputMode::ImportPath;
            }
            KeyCode::Char('c') => {
                // Prefill with current value
                if let Some(ref dir) = self.config.install_dir {
                    self.install_dir_input.set_value(dir);
                }
                self.install_dir_input.show();
                self.input_mode = InputMode::SetInstallDir;
            }
            KeyCode::Char('g') => {
                // Clone repo for selected project (only when path not set)
                if let Some(project) = self.selected_project() {
                    if self.selected_location.is_none() && !project.repo_url.is_empty() {
                        if let Some(install_dir) = self.config.get_install_dir() {
                            // Clone directly to install_dir
                            self.clone_selected_to(install_dir);
                        } else {
                            // Prompt for directory
                            self.clone_path_input.show();
                            self.input_mode = InputMode::ClonePath;
                        }
                    }
                }
            }
            KeyCode::F(5) => self.full_refresh(),
            KeyCode::Enter => self.update_selected_details(),
            _ => {}
        }
    }

    fn add_project(&mut self, name: &str, url: &str) {
        if let Ok(id) = self.db.add_project(name, url) {
            // Clone to install directory if configured
            if let Some(install_dir) = self.config.get_install_dir() {
                if !url.is_empty() {
                    let dest = install_dir.join(name);
                    if self.clone_repo(url, &dest) {
                        // Set the location for this machine
                        let _ = self.db.set_location(
                            id,
                            &self.machine_id,
                            dest.to_str().unwrap_or(""),
                        );
                    }
                }
            }

            let _ = sync::push(&format!("Add project: {}", name));
            self.projects = self.db.list_projects().unwrap_or_default();
            // Select the new project
            if let Some(idx) = self.projects.iter().position(|p| p.id == id) {
                self.list_state.select(Some(idx));
            }
            self.update_selected_details();
        }
    }

    fn clone_repo(&self, url: &str, dest: &Path) -> bool {
        use std::process::Command;

        // Skip if destination already exists
        if dest.exists() {
            return true;
        }

        // Ensure parent directory exists
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        Command::new("git")
            .args(["clone", url, dest.to_str().unwrap_or("")])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
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

    fn clone_selected_to(&mut self, base_dir: std::path::PathBuf) {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;
            let project_name = project.name.clone();
            let repo_url = project.repo_url.clone();

            if repo_url.is_empty() {
                return;
            }

            let dest = base_dir.join(&project_name);
            if self.clone_repo(&repo_url, &dest) {
                // Set the location for this machine
                if let Some(dest_str) = dest.to_str() {
                    if self
                        .db
                        .set_location(project_id, &self.machine_id, dest_str)
                        .is_ok()
                    {
                        let _ = sync::push(&format!(
                            "Cloned {} to {} on {}",
                            project_name, dest_str, self.machine_id
                        ));
                        self.update_selected_details();
                    }
                }
            }
        }
    }

    fn set_run_command(&mut self, cmd: Option<&str>) {
        if let Some(project) = self.selected_project() {
            let project_id = project.id;
            let project_name = project.name.clone();
            if self
                .db
                .set_run_command(project_id, &self.machine_id, cmd)
                .is_ok()
            {
                let _ = sync::push(&format!(
                    "Set run command for {} on {}",
                    project_name, self.machine_id
                ));
                self.update_selected_details();
            }
        }
    }

    fn import_from_path(&mut self, path_str: &str) {
        use std::process::Command;

        let path = Path::new(path_str);
        if !path.exists() || !path.is_dir() {
            return;
        }

        // Get project name from directory
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => return,
        };

        // Try to get git remote URL
        let remote_url = Command::new("git")
            .current_dir(path)
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        // Add project
        if let Ok(id) = self.db.add_project(&name, &remote_url) {
            // Set location
            if self.db.set_location(id, &self.machine_id, path_str).is_ok() {
                let _ = sync::push(&format!("Import project: {}", name));
                self.projects = self.db.list_projects().unwrap_or_default();
                // Select the new project
                if let Some(idx) = self.projects.iter().position(|p| p.id == id) {
                    self.list_state.select(Some(idx));
                }
                self.update_selected_details();
            }
        }
    }

    fn full_refresh(&mut self) {
        // Pull latest from GitHub
        let _ = sync::pull();

        // Reload projects
        self.projects = self.db.list_projects().unwrap_or_default();

        // Refresh port scan
        self.port_info = ports::scan_ports();
        self.last_port_scan = std::time::Instant::now();

        // Update selected details
        self.update_selected_details();
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

            if let Some(ref loc) = self.selected_location {
                let path = Path::new(&loc.path);

                // Git fetch before running (blocking)
                self.git_fetch(path);

                // Install dependencies for JS projects before starting dev server
                if self.is_js_project() {
                    self.install_node_modules(path);
                }

                // Spawn a new terminal with claude
                self.spawn_terminal_with_claude(path);

                // Also start any dev server in background if not already running
                if !self.process_manager.is_running(project_id) {
                    let cmd = loc
                        .run_command
                        .clone()
                        .or_else(|| self.selected_detection.as_ref().and_then(|d| d.run_command.clone()));

                    if let Some(cmd) = cmd {
                        // For JavaScript projects, find an available port
                        let port = if self.is_js_project() {
                            ports::find_available_port()
                        } else {
                            None
                        };

                        let _ = self.process_manager.start_with_port(project_id, path, &cmd, port);
                    }
                }
            }
        }
    }

    fn git_fetch(&self, path: &Path) {
        use std::process::Command;

        // Run git fetch and wait for completion
        let _ = Command::new("git")
            .args(["fetch", "--all", "--prune"])
            .current_dir(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status(); // .status() waits for completion
    }

    fn install_node_modules(&self, path: &Path) {
        use std::process::Command;

        // Get the package manager from detection
        let pm = self
            .selected_detection
            .as_ref()
            .and_then(|d| d.package_manager)
            .unwrap_or(detect::PackageManager::Npm);

        let install_cmd = match pm {
            detect::PackageManager::Pnpm => "pnpm",
            detect::PackageManager::Yarn => "yarn",
            detect::PackageManager::Bun => "bun",
            detect::PackageManager::Npm => "npm",
            _ => return, // Not a JS package manager
        };

        // Run install and wait for completion
        let _ = Command::new(install_cmd)
            .arg("install")
            .current_dir(path)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
    }

    fn is_js_project(&self) -> bool {
        self.selected_detection
            .as_ref()
            .map(|d| d.project_type == detect::ProjectType::JavaScript)
            .unwrap_or(false)
    }

    fn spawn_terminal_with_claude(&self, path: &Path) {
        use std::process::Command;

        let path_str = path.to_string_lossy().to_string();

        // Try various terminal emulators in order of preference
        #[cfg(target_os = "linux")]
        {
            // Try common Linux terminal emulators in order of preference
            if Command::new("which").arg("ghostty").output().map(|o| o.status.success()).unwrap_or(false) {
                let shell_cmd = format!("cd '{}' && claude; exec $SHELL", path_str);
                let _ = Command::new("ghostty")
                    .args(["-e", "bash", "-c", &shell_cmd])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("alacritty").output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new("alacritty")
                    .args(["--working-directory", &path_str, "-e", "claude"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("kitty").output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new("kitty")
                    .args(["--directory", &path_str, "claude"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("gnome-terminal").output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new("gnome-terminal")
                    .args(["--working-directory", &path_str, "--", "claude"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("konsole").output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new("konsole")
                    .args(["--workdir", &path_str, "-e", "claude"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("xfce4-terminal").output().map(|o| o.status.success()).unwrap_or(false) {
                let _ = Command::new("xfce4-terminal")
                    .args(["--working-directory", &path_str, "-e", "claude"])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            } else if Command::new("which").arg("xterm").output().map(|o| o.status.success()).unwrap_or(false) {
                let xterm_cmd = format!("cd '{}' && claude", path_str);
                let _ = Command::new("xterm")
                    .args(["-e", &xterm_cmd])
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }

        #[cfg(target_os = "macos")]
        {
            // Use osascript to open Terminal.app
            let script = format!(
                r#"tell application "Terminal"
                    activate
                    do script "cd '{}' && claude"
                end tell"#,
                path_str
            );
            let _ = Command::new("osascript")
                .args(["-e", &script])
                .spawn();
        }

        #[cfg(target_os = "windows")]
        {
            // Try Windows Terminal first, fall back to cmd
            if Command::new("where")
                .arg("wt")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                let _ = Command::new("wt")
                    .args(["-d", &path_str, "cmd", "/k", "claude"])
                    .spawn();
            } else {
                let _ = Command::new("cmd")
                    .args(["/c", "start", "cmd", "/k", &format!("cd /d \"{}\" && claude", path_str)])
                    .spawn();
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
        self.selected_git_status = None;

        if let Some(idx) = self.list_state.selected() {
            if let Some(project) = self.projects.get(idx) {
                self.selected_location = self
                    .db
                    .get_location(project.id, &self.machine_id)
                    .ok()
                    .flatten();

                if let Some(ref loc) = self.selected_location {
                    let path = Path::new(&loc.path);
                    self.selected_detection = detect::detect(path).ok();
                    self.selected_git_status = git_status::get_status(path).ok();
                }
            }
        }
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.list_state
            .selected()
            .and_then(|i| self.projects.get(i))
    }

    fn maybe_refresh_ports(&mut self) {
        if self.last_port_scan.elapsed() > std::time::Duration::from_secs(30) {
            self.port_info = ports::scan_ports();
            self.last_port_scan = std::time::Instant::now();
        }
    }

    pub fn render(&mut self, frame: &mut Frame) {
        self.maybe_refresh_ports();

        let has_running = !self.process_manager.running_projects().is_empty();

        let main_chunks = if has_running && self.show_logs {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(10),
                    Constraint::Length(10),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
                .split(frame.area())
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(0),
                    Constraint::Length(1),
                    Constraint::Length(1),
                ])
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
            self.render_ports_bar(frame, main_chunks[2]);
            self.render_help_bar(frame, main_chunks[3]);
        } else {
            self.render_ports_bar(frame, main_chunks[1]);
            self.render_help_bar(frame, main_chunks[2]);
        }

        // Render input dialogs on top
        let area = frame.area();
        self.path_input.render(frame, area);
        self.run_cmd_input.render(frame, area);
        self.import_path_input.render(frame, area);
        self.install_dir_input.render(frame, area);
        self.clone_path_input.render(frame, area);
        self.repo_selector.render(frame, area);

        // Render quit confirmation dialog
        if self.input_mode == InputMode::ConfirmQuit {
            self.render_quit_dialog(frame, area);
        }
    }

    fn render_quit_dialog(&self, frame: &mut Frame, area: Rect) {
        use ratatui::widgets::Clear;

        let width = 40.min(area.width.saturating_sub(4));
        let height = 3;
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let dialog_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, dialog_area);

        let text = Paragraph::new("Quit? (y/n)")
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Confirm ")
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(text, dialog_area);
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

    fn render_ports_bar(&self, frame: &mut Frame, area: Rect) {
        let port_spans: Vec<Span> = self
            .port_info
            .iter()
            .map(|p| {
                let label = match (&p.process_name, p.pid) {
                    (Some(name), Some(pid)) => format!("{}({},{})", p.port, name, pid),
                    (Some(name), None) => format!("{}({})", p.port, name),
                    (None, Some(pid)) => format!("{}(PID:{})", p.port, pid),
                    (None, None) => format!("{}", p.port),
                };
                Span::styled(format!(" {} ", label), Style::default().fg(Color::Yellow))
            })
            .collect();

        let content = if port_spans.is_empty() {
            Line::from(Span::styled(
                " No ports in use ",
                Style::default().fg(Color::DarkGray),
            ))
        } else {
            let mut spans = vec![Span::styled("Ports:", Style::default().fg(Color::DarkGray))];
            spans.extend(port_spans);
            Line::from(spans)
        };

        let para = Paragraph::new(content);
        frame.render_widget(para, area);
    }

    fn render_help_bar(&self, frame: &mut Frame, area: Rect) {
        let help_text = " [a]dd  [i]mport  [p]ath  [g]it  [e]dit  [r]un  [s]top  [d]el  [c]fg  [F5]  [q]uit ";
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

                // Git status
                if let Some(ref git) = self.selected_git_status {
                    lines.push(Line::from(""));
                    lines.push(Line::from(vec![
                        Span::styled("Branch: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(&git.branch),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("Staged: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(git.staged.to_string()),
                        Span::raw("  "),
                        Span::styled("Modified: ", Style::default().fg(Color::DarkGray)),
                        Span::raw(git.modified.to_string()),
                    ]));
                    lines.push(Line::from(vec![
                        Span::styled("Ahead: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            git.ahead.to_string(),
                            Style::default().fg(if git.ahead > 0 {
                                Color::Yellow
                            } else {
                                Color::White
                            }),
                        ),
                        Span::raw("  "),
                        Span::styled("Behind: ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            git.behind.to_string(),
                            Style::default().fg(if git.behind > 0 {
                                Color::Red
                            } else {
                                Color::White
                            }),
                        ),
                    ]));
                }

                lines.push(Line::from(""));

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
                if !project.repo_url.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Press 'g' to clone from repo",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
                lines.push(Line::from(Span::styled(
                    "Press 'p' to set path manually",
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
