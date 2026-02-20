use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};
use std::path::Path;

use crate::detect;
use crate::gh;
use crate::git_status::{self, GitStatus};
use crate::ports::{self, PortInfo};
use crate::process::ProcessManager;
use crate::scanner;
use crate::store::{ProjectEntry, ProjectStore};
use crate::ui::input::InputDialog;
use crate::ui::selector::RepoSelector;

#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    SelectRepo,
    SelectScan,
    EditRunCmd,
    ImportPath,
    SetInstallDir,
    ClonePath,
    ConfirmQuit,
}

pub struct App {
    pub store: ProjectStore,
    pub list_state: ListState,
    pub selected_detection: Option<detect::DetectedProject>,
    pub selected_git_status: Option<GitStatus>,
    // Input dialogs
    input_mode: InputMode,
    run_cmd_input: InputDialog,
    import_path_input: InputDialog,
    install_dir_input: InputDialog,
    clone_path_input: InputDialog,
    repo_selector: RepoSelector,
    scan_selector: RepoSelector,
    // Process management
    pub process_manager: ProcessManager,
    show_logs: bool,
    // Port scanning
    pub port_info: Vec<PortInfo>,
    last_port_scan: std::time::Instant,
    // GitHub availability
    pub gh_available: bool,
    // Quit state
    should_quit: bool,
}

impl App {
    pub fn new(store: ProjectStore, gh_available: bool) -> anyhow::Result<Self> {
        let mut list_state = ListState::default();
        if !store.projects.is_empty() {
            list_state.select(Some(0));
        }

        let mut app = Self {
            store,
            list_state,
            selected_detection: None,
            selected_git_status: None,
            input_mode: InputMode::Normal,
            run_cmd_input: InputDialog::new("Run Command"),
            import_path_input: InputDialog::new("Import Path"),
            install_dir_input: InputDialog::new("Install Directory"),
            clone_path_input: InputDialog::new("Clone to Directory"),
            repo_selector: RepoSelector::new(),
            scan_selector: RepoSelector::new(),
            process_manager: ProcessManager::new(),
            show_logs: true,
            port_info: ports::scan_ports(),
            last_port_scan: std::time::Instant::now(),
            gh_available,
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
                    self.add_from_github(&name, &url);
                }
                if !self.repo_selector.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::SelectScan => {
                if let Some((_display, data)) = self.scan_selector.handle_key(key) {
                    // data is "path\nremote_url"
                    let mut parts = data.splitn(2, '\n');
                    let path = parts.next().unwrap_or("").to_string();
                    let remote_url = parts.next().unwrap_or("").to_string();
                    let remote_opt = if remote_url.is_empty() {
                        None
                    } else {
                        Some(remote_url)
                    };

                    // Derive name from path
                    let name = Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    self.store.add(ProjectEntry {
                        name: name.clone(),
                        repo_url: remote_opt,
                        path,
                        run_command: None,
                    });
                    let _ = self.store.save();

                    // Select the newly added project
                    if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
                        self.list_state.select(Some(idx));
                    }
                    self.update_selected_details();
                }
                if !self.scan_selector.visible {
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
                    self.store.install_dir = if dir.is_empty() { None } else { Some(dir) };
                    let _ = self.store.save();
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
                if self.gh_available {
                    if let Ok(repos) = gh::list_repos() {
                        self.repo_selector.show(repos);
                        self.input_mode = InputMode::SelectRepo;
                    }
                }
            }
            KeyCode::Char('s') => self.scan_for_projects(),
            KeyCode::Char('d') => self.delete_selected(),
            KeyCode::Char('r') => self.run_selected(),
            KeyCode::Char('x') => self.stop_selected(),
            KeyCode::Char('e') => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(project) = self.store.projects.get(idx) {
                        if !project.path.is_empty() && Path::new(&project.path).exists() {
                            self.run_cmd_input.show();
                            self.input_mode = InputMode::EditRunCmd;
                        }
                    }
                }
            }
            KeyCode::Char('i') => {
                self.import_path_input.show();
                self.input_mode = InputMode::ImportPath;
            }
            KeyCode::Char('c') => {
                if let Some(ref dir) = self.store.install_dir {
                    self.install_dir_input.set_value(dir);
                }
                self.install_dir_input.show();
                self.input_mode = InputMode::SetInstallDir;
            }
            KeyCode::Char('g') => {
                if let Some(idx) = self.list_state.selected() {
                    if let Some(project) = self.store.projects.get(idx) {
                        let path_empty = project.path.is_empty() || !Path::new(&project.path).exists();
                        let has_repo = project.repo_url.is_some();
                        if path_empty && has_repo {
                            if let Some(install_dir) = self.store.get_install_dir() {
                                self.clone_selected_to(install_dir);
                            } else {
                                self.clone_path_input.show();
                                self.input_mode = InputMode::ClonePath;
                            }
                        }
                    }
                }
            }
            KeyCode::F(5) => self.full_refresh(),
            KeyCode::Enter => self.update_selected_details(),
            _ => {}
        }
    }

    fn add_from_github(&mut self, name: &str, url: &str) {
        // Determine path: clone to install_dir if configured
        let path = if let Some(install_dir) = self.store.get_install_dir() {
            if !url.is_empty() {
                let dest = install_dir.join(name);
                if self.clone_repo(url, &dest) {
                    dest.to_str().unwrap_or("").to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let repo_url = if url.is_empty() {
            None
        } else {
            Some(url.to_string())
        };

        self.store.add(ProjectEntry {
            name: name.to_string(),
            repo_url,
            path,
            run_command: None,
        });
        let _ = self.store.save();

        // Select the new project
        if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
            self.list_state.select(Some(idx));
        }
        self.update_selected_details();
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

    fn clone_selected_to(&mut self, base_dir: std::path::PathBuf) {
        if let Some(idx) = self.list_state.selected() {
            let (name, repo_url) = {
                let project = match self.store.projects.get(idx) {
                    Some(p) => p,
                    None => return,
                };
                let repo_url = match &project.repo_url {
                    Some(u) => u.clone(),
                    None => return,
                };
                (project.name.clone(), repo_url)
            };

            let dest = base_dir.join(&name);
            if self.clone_repo(&repo_url, &dest) {
                if let Some(dest_str) = dest.to_str() {
                    if let Some(project) = self.store.get_mut(&name) {
                        project.path = dest_str.to_string();
                    }
                    let _ = self.store.save();
                    self.update_selected_details();
                }
            }
        }
    }

    fn set_run_command(&mut self, cmd: Option<&str>) {
        if let Some(idx) = self.list_state.selected() {
            let name = match self.store.projects.get(idx) {
                Some(p) => p.name.clone(),
                None => return,
            };
            if let Some(project) = self.store.get_mut(&name) {
                project.run_command = cmd.map(|s| s.to_string());
            }
            let _ = self.store.save();
            self.update_selected_details();
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
            .filter(|s| !s.is_empty());

        self.store.add(ProjectEntry {
            name: name.clone(),
            repo_url: remote_url,
            path: path_str.to_string(),
            run_command: None,
        });
        let _ = self.store.save();

        // Select the new project
        if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
            self.list_state.select(Some(idx));
        }
        self.update_selected_details();
    }

    fn scan_for_projects(&mut self) {
        let found = scanner::scan_directories();

        // Filter out projects already in the store
        let new_projects: Vec<_> = found
            .into_iter()
            .filter(|sp| self.store.get(&sp.name).is_none())
            .collect();

        if new_projects.is_empty() {
            return;
        }

        // Build items for the scan selector
        // display = "name (path)", data = "path\nremote_url"
        let repos: Vec<(String, String)> = new_projects
            .iter()
            .map(|sp| {
                let display = format!("{} ({})", sp.name, sp.path.display());
                let remote = sp.remote_url.as_deref().unwrap_or("");
                let data = format!("{}\n{}", sp.path.display(), remote);
                (display, data)
            })
            .collect();

        self.scan_selector.show(repos);
        self.input_mode = InputMode::SelectScan;
    }

    fn full_refresh(&mut self) {
        // Reload store from file
        if let Ok(reloaded) = ProjectStore::load() {
            self.store = reloaded;
        }

        // Refresh port scan
        self.port_info = ports::scan_ports();
        self.last_port_scan = std::time::Instant::now();

        // Update selected details
        self.update_selected_details();
    }

    fn delete_selected(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            let name = match self.store.projects.get(idx) {
                Some(p) => p.name.clone(),
                None => return,
            };
            self.store.remove(&name);
            let _ = self.store.save();

            if self.store.projects.is_empty() {
                self.list_state.select(None);
            } else if idx >= self.store.projects.len() {
                self.list_state.select(Some(self.store.projects.len() - 1));
            }
            self.update_selected_details();
        }
    }

    fn run_selected(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            let project = match self.store.projects.get(idx) {
                Some(p) => p,
                None => return,
            };

            if project.path.is_empty() {
                return;
            }

            let path = Path::new(&project.path);
            if !path.exists() {
                return;
            }

            let project_id = idx as i64;
            let run_command_override = project.run_command.clone();

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
                let cmd = run_command_override
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
        // On Windows, npm/pnpm/yarn/bun are .cmd files, must run through cmd.exe
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let _ = Command::new("cmd.exe")
                .args(["/c", install_cmd, "install"])
                .current_dir(path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }

        #[cfg(not(windows))]
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
        if let Some(idx) = self.list_state.selected() {
            let project_id = idx as i64;
            let _ = self.process_manager.stop(project_id);
        }
    }

    pub fn is_input_mode(&self) -> bool {
        self.input_mode != InputMode::Normal
    }

    fn next(&mut self) {
        if self.store.projects.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1) % self.store.projects.len(),
            None => 0,
        };
        self.list_state.select(Some(i));
        self.update_selected_details();
    }

    fn previous(&mut self) {
        if self.store.projects.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => {
                if i == 0 {
                    self.store.projects.len() - 1
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
        self.selected_detection = None;
        self.selected_git_status = None;

        if let Some(idx) = self.list_state.selected() {
            if let Some(project) = self.store.projects.get(idx) {
                if !project.path.is_empty() {
                    let path = Path::new(&project.path);
                    if path.exists() {
                        self.selected_detection = detect::detect(path).ok();
                        self.selected_git_status = git_status::get_status(path).ok();
                    }
                }
            }
        }
    }

    fn selected_project(&self) -> Option<&ProjectEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.store.projects.get(i))
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
        self.run_cmd_input.render(frame, area);
        self.import_path_input.render(frame, area);
        self.install_dir_input.render(frame, area);
        self.clone_path_input.render(frame, area);
        self.repo_selector.render(frame, area);
        self.scan_selector.render(frame, area);

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
        let project_id = self.list_state.selected().map(|idx| idx as i64);
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
        let gh_label = if self.gh_available {
            "[a]dd"
        } else {
            "[a]dd(gh unavailable)"
        };

        let help_text = format!(
            " {}  [i]mport  [s]can  [g]it  [e]dit  [r]un  [x]stop  [d]el  [c]fg  [F5]  [q]uit ",
            gh_label
        );

        let help = Paragraph::new(Line::from(vec![
            Span::styled(help_text, Style::default().fg(Color::DarkGray)),
        ]));

        frame.render_widget(help, area);
    }

    fn render_project_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self
            .store
            .projects
            .iter()
            .enumerate()
            .map(|(idx, p)| {
                let has_path = !p.path.is_empty() && Path::new(&p.path).exists();
                let is_running = self.process_manager.is_running(idx as i64);

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
            let repo_display = project.repo_url.as_deref().unwrap_or("");

            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&project.name),
                ]),
                Line::from(vec![
                    Span::styled("Repo: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(repo_display),
                ]),
                Line::from(""),
            ];

            let has_path = !project.path.is_empty() && Path::new(&project.path).exists();

            if has_path {
                lines.push(Line::from(vec![
                    Span::styled("Path: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&project.path),
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

                if let Some(ref cmd) = project.run_command {
                    lines.push(Line::from(vec![
                        Span::styled("Override: ", Style::default().fg(Color::Yellow)),
                        Span::raw(cmd),
                    ]));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    "Path not set",
                    Style::default().fg(Color::Red),
                )));
                if project.repo_url.is_some() {
                    lines.push(Line::from(Span::styled(
                        "Press 'g' to clone from repo",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
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
