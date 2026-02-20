# Local-Only Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the synced SQLite database with a local TOML-based project store, removing all cross-machine sync infrastructure.

**Architecture:** Single `projects.toml` file at `~/.claude-manager/projects.toml` stores both config (install_dir) and project list. No database, no sync repo, no machine IDs. The `gh` CLI becomes optional — app works without it.

**Tech Stack:** Rust, ratatui, serde + toml for storage, crossterm for terminal

---

### Task 1: Create `store.rs` — the ProjectStore module

This is the foundation that replaces `db.rs`, `config.rs`, `sync.rs`, and `machine.rs`.

**Files:**
- Create: `src/store.rs`

**Step 1: Write `store.rs` with data types and load/save logic**

```rust
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStore {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,

    #[serde(skip)]
    file_path: PathBuf,
}

impl ProjectStore {
    pub fn load() -> Result<Self> {
        let file_path = Self::store_path()?;

        let mut store = if file_path.exists() {
            let content = fs::read_to_string(&file_path)?;
            let mut store: ProjectStore = toml::from_str(&content)?;
            store.file_path = file_path;
            store
        } else {
            ProjectStore {
                file_path,
                ..Default::default()
            }
        };

        // Sort projects by name
        store.projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&self.file_path, content)?;
        Ok(())
    }

    pub fn is_first_run(&self) -> bool {
        !self.file_path.exists()
    }

    pub fn add(&mut self, entry: ProjectEntry) {
        // Don't add duplicates by name
        if !self.projects.iter().any(|p| p.name == entry.name) {
            self.projects.push(entry);
            self.projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.projects.retain(|p| p.name != name);
    }

    pub fn get(&self, name: &str) -> Option<&ProjectEntry> {
        self.projects.iter().find(|p| p.name == name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut ProjectEntry> {
        self.projects.iter_mut().find(|p| p.name == name)
    }

    pub fn get_install_dir(&self) -> Option<PathBuf> {
        self.install_dir.as_ref().map(|dir| {
            if dir.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    return home.join(&dir[2..]);
                }
            }
            PathBuf::from(dir)
        })
    }

    fn store_path() -> Result<PathBuf> {
        let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
        Ok(home.join(".claude-manager").join("projects.toml"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_remove() {
        let mut store = ProjectStore::default();
        store.add(ProjectEntry {
            name: "test".to_string(),
            repo_url: Some("https://github.com/user/test.git".to_string()),
            path: "/home/user/test".to_string(),
            run_command: None,
        });
        assert_eq!(store.projects.len(), 1);

        // Duplicate name should not add
        store.add(ProjectEntry {
            name: "test".to_string(),
            repo_url: None,
            path: "/other/path".to_string(),
            run_command: None,
        });
        assert_eq!(store.projects.len(), 1);

        store.remove("test");
        assert!(store.projects.is_empty());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let store = ProjectStore {
            install_dir: Some("~/Projects".to_string()),
            projects: vec![ProjectEntry {
                name: "my-app".to_string(),
                repo_url: Some("https://github.com/user/my-app.git".to_string()),
                path: "/home/user/my-app".to_string(),
                run_command: Some("npm run dev".to_string()),
            }],
            file_path: PathBuf::new(),
        };

        let toml_str = toml::to_string_pretty(&store).unwrap();
        let loaded: ProjectStore = toml::from_str(&toml_str).unwrap();

        assert_eq!(loaded.install_dir, store.install_dir);
        assert_eq!(loaded.projects.len(), 1);
        assert_eq!(loaded.projects[0].name, "my-app");
    }
}
```

**Step 2: Run tests to verify**

Run: `cargo test --lib store`
Expected: All tests pass

**Step 3: Commit**

```
git add src/store.rs
git commit -m "Add ProjectStore module for TOML-based local storage"
```

---

### Task 2: Update `Cargo.toml` — remove unused dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Remove `rusqlite`, `hostname`, and `uuid` dependencies**

Remove these three lines from `[dependencies]`:
```
hostname = "0.4"
rusqlite = { version = "0.32", features = ["bundled"] }
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Verify it still parses**

Run: `cargo check 2>&1 | head -5`
Expected: Will show errors about missing modules (expected — we haven't updated the code yet). The Cargo.toml itself should be valid.

**Step 3: Commit**

```
git add Cargo.toml
git commit -m "Remove rusqlite, hostname, and uuid dependencies"
```

---

### Task 3: Update `errors.rs` — simplify error types

**Files:**
- Modify: `src/errors.rs`

**Step 1: Remove database and sync-related error variants**

Replace the entire file with:

```rust
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    IoError(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::IoError(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for AppError {}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::IoError(e)
    }
}
```

**Step 2: Commit**

```
git add src/errors.rs
git commit -m "Simplify error types, remove DB and sync variants"
```

---

### Task 4: Update `gh.rs` — make auth check non-fatal

**Files:**
- Modify: `src/gh.rs`

**Step 1: Change `check_auth` to return bool without requiring gh to be installed**

Replace the `check_auth` function:

```rust
/// Check if gh CLI is authenticated. Returns false if gh is not installed or not authed.
pub fn check_auth() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
```

Also remove the `bail` import if it's no longer used (check after — `list_repos`, `create_repo`, `clone_repo`, `get_username` still use it, so keep it).

**Step 2: Commit**

```
git add src/gh.rs
git commit -m "Make gh auth check non-fatal, return bool"
```

---

### Task 5: Update `main.rs` — single startup flow, no --init

**Files:**
- Modify: `src/main.rs`

**Step 1: Rewrite main.rs with new startup flow**

Replace the entire file:

```rust
mod app;
mod detect;
mod errors;
mod gh;
mod git_status;
mod ports;
mod process;
mod scanner;
mod store;
mod tui;
mod ui;

use crate::store::{ProjectEntry, ProjectStore};

fn main() -> anyhow::Result<()> {
    let mut store = ProjectStore::load()?;
    let first_run = store.is_first_run();

    // On first run, offer to scan for projects
    if first_run {
        run_first_time_setup(&mut store)?;
    }

    // Check gh auth (non-fatal)
    let gh_available = gh::check_auth();

    let mut app = app::App::new(store, gh_available)?;
    let mut tui = tui::Tui::new()?;
    tui.run(&mut app)?;

    Ok(())
}

fn run_first_time_setup(store: &mut ProjectStore) -> anyhow::Result<()> {
    use std::io::{self, Write};

    println!("Welcome to Claude Manager!\n");
    print!("Scan for existing git repos in common directories? [Y/n] ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    if input.trim().is_empty() || input.trim().to_lowercase() == "y" {
        println!("\nScanning...");
        let found = scanner::scan_directories();

        if found.is_empty() {
            println!("No git repositories found.");
        } else {
            println!("\nFound {} repositories:\n", found.len());

            for (i, proj) in found.iter().enumerate() {
                let url = proj.remote_url.as_deref().unwrap_or("(no remote)");
                println!("  [{}] {} - {}", i + 1, proj.name, url);
            }

            print!("\nEnter numbers to import (comma-separated), 'all', or 'none': ");
            io::stdout().flush()?;
            input.clear();
            io::stdin().read_line(&mut input)?;

            let to_import: Vec<usize> = if input.trim() == "all" {
                (0..found.len()).collect()
            } else if input.trim() == "none" {
                Vec::new()
            } else {
                input
                    .trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .filter(|&n| n > 0 && n <= found.len())
                    .map(|n| n - 1)
                    .collect()
            };

            for idx in to_import {
                let proj = &found[idx];
                store.add(ProjectEntry {
                    name: proj.name.clone(),
                    repo_url: proj.remote_url.clone(),
                    path: proj.path.to_string_lossy().to_string(),
                    run_command: None,
                });
                println!("  Imported: {}", proj.name);
            }
        }
    }

    store.save()?;
    println!("\nSetup complete! Starting Claude Manager...\n");
    Ok(())
}
```

**Step 2: Commit**

```
git add src/main.rs
git commit -m "Rewrite main.rs with single startup flow, no --init"
```

---

### Task 6: Rewrite `app.rs` — use ProjectStore, add scan keybind

This is the largest change. Replace all database/sync usage with the ProjectStore.

**Files:**
- Modify: `src/app.rs`

**Step 1: Update imports and struct definition**

Replace the top of the file (lines 1-61) — remove db, sync, config imports and replace with store:

```rust
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
    EditRunCmd,
    ImportPath,
    SetInstallDir,
    ClonePath,
    ConfirmQuit,
    SelectScan,
}

pub struct App {
    pub store: ProjectStore,
    pub list_state: ListState,
    pub selected_detection: Option<detect::DetectedProject>,
    pub selected_git_status: Option<GitStatus>,
    pub gh_available: bool,
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
    // Quit state
    should_quit: bool,
}
```

**Step 2: Rewrite `App::new` and basic methods**

```rust
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
            gh_available,
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

    pub fn selected_project(&self) -> Option<&ProjectEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.store.projects.get(i))
    }

    pub fn is_input_mode(&self) -> bool {
        self.input_mode != InputMode::Normal
    }
}
```

**Step 3: Rewrite `handle_key` and `handle_normal_key`**

Key changes: remove `SetPath` mode, change `s` from stop to scan, add `x` for stop, and handle `SelectScan` mode.

```rust
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
                if let Some((name, _url)) = self.scan_selector.handle_key(key) {
                    // Find the scanned project and import it
                    // The URL field contains the path for scan results
                    self.import_scanned_project(&name);
                }
                if !self.scan_selector.visible {
                    self.input_mode = InputMode::Normal;
                }
            }
            InputMode::EditRunCmd => {
                if let Some(cmd) = self.run_cmd_input.handle_key(key) {
                    self.set_run_command(if cmd.is_empty() { None } else { Some(cmd) });
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
            KeyCode::Char('x') => self.stop_selected(),
            KeyCode::Char('d') => self.delete_selected(),
            KeyCode::Char('r') => self.run_selected(),
            KeyCode::Char('e') => {
                if self.selected_project().is_some() {
                    self.run_cmd_input.show();
                    self.input_mode = InputMode::EditRunCmd;
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
                if let Some(project) = self.selected_project() {
                    if project.repo_url.is_some() {
                        if let Some(install_dir) = self.store.get_install_dir() {
                            self.clone_selected_to(install_dir);
                        } else {
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
```

**Step 4: Rewrite the action methods (add, delete, import, clone, run, scan)**

```rust
    fn add_from_github(&mut self, name: &str, url: &str) {
        let mut entry = ProjectEntry {
            name: name.to_string(),
            repo_url: Some(url.to_string()),
            path: String::new(),
            run_command: None,
        };

        // Clone to install directory if configured
        if let Some(install_dir) = self.store.get_install_dir() {
            let dest = install_dir.join(name);
            if self.clone_repo(url, &dest) {
                entry.path = dest.to_string_lossy().to_string();
            }
        }

        if !entry.path.is_empty() {
            self.store.add(entry);
            let _ = self.store.save();
            // Select the new project
            if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
                self.list_state.select(Some(idx));
            }
            self.update_selected_details();
        }
    }

    fn clone_repo(&self, url: &str, dest: &Path) -> bool {
        use std::process::Command;

        if dest.exists() {
            return true;
        }

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
        if let Some(project) = self.selected_project() {
            let name = project.name.clone();
            let repo_url = match &project.repo_url {
                Some(url) if !url.is_empty() => url.clone(),
                _ => return,
            };

            let dest = base_dir.join(&name);
            if self.clone_repo(&repo_url, &dest) {
                if let Some(entry) = self.store.get_mut(&name) {
                    entry.path = dest.to_string_lossy().to_string();
                    let _ = self.store.save();
                    self.update_selected_details();
                }
            }
        }
    }

    fn set_run_command(&mut self, cmd: Option<String>) {
        if let Some(project) = self.selected_project() {
            let name = project.name.clone();
            if let Some(entry) = self.store.get_mut(&name) {
                entry.run_command = cmd;
                let _ = self.store.save();
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

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => return,
        };

        let remote_url = Command::new("git")
            .current_dir(path)
            .args(["remote", "get-url", "origin"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

        self.store.add(ProjectEntry {
            name: name.clone(),
            repo_url: remote_url,
            path: path_str.to_string(),
            run_command: None,
        });
        let _ = self.store.save();

        if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
            self.list_state.select(Some(idx));
        }
        self.update_selected_details();
    }

    fn scan_for_projects(&mut self) {
        let found = scanner::scan_directories();

        // Filter out projects already in the store
        let new_repos: Vec<(String, String)> = found
            .into_iter()
            .filter(|p| self.store.get(&p.name).is_none())
            .map(|p| {
                let display = format!("{} ({})", p.name, p.path.display());
                let path = p.path.to_string_lossy().to_string();
                // Pack path and remote_url together separated by \n for later parsing
                let data = format!("{}\n{}", path, p.remote_url.unwrap_or_default());
                (display, data)
            })
            .collect();

        if new_repos.is_empty() {
            return;
        }

        self.scan_selector.show(new_repos);
        self.input_mode = InputMode::SelectScan;
    }

    fn import_scanned_project(&mut self, display_name: &str) {
        // The display_name is "name (path)", extract the name
        let name = display_name.split(" (").next().unwrap_or(display_name).to_string();

        // Find the scan selector's data for this entry
        // The url field contains "path\nremote_url"
        if let Some((_display, data)) = self.scan_selector.items.iter().find(|(d, _)| d == display_name) {
            let mut parts = data.splitn(2, '\n');
            let path = parts.next().unwrap_or("").to_string();
            let remote_url = parts.next().unwrap_or("").to_string();

            self.store.add(ProjectEntry {
                name: name.clone(),
                repo_url: if remote_url.is_empty() { None } else { Some(remote_url) },
                path,
                run_command: None,
            });
            let _ = self.store.save();

            if let Some(idx) = self.store.projects.iter().position(|p| p.name == name) {
                self.list_state.select(Some(idx));
            }
            self.update_selected_details();
        }
    }

    fn full_refresh(&mut self) {
        // Reload from file
        if let Ok(store) = ProjectStore::load() {
            self.store = store;
        }

        self.port_info = ports::scan_ports();
        self.last_port_scan = std::time::Instant::now();
        self.update_selected_details();
    }

    fn delete_selected(&mut self) {
        if let Some(project) = self.selected_project() {
            let name = project.name.clone();
            self.store.remove(&name);
            let _ = self.store.save();

            if self.store.projects.is_empty() {
                self.list_state.select(None);
            } else if let Some(idx) = self.list_state.selected() {
                if idx >= self.store.projects.len() {
                    self.list_state.select(Some(self.store.projects.len() - 1));
                }
            }
            self.update_selected_details();
        }
    }

    fn stop_selected(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            let _ = self.process_manager.stop(idx as i64);
        }
    }
```

**Step 5: Rewrite `run_selected`, navigation, and `update_selected_details`**

Note: the process manager currently uses `project.id` (i64 from SQLite). Since we no longer have IDs, we'll use the list index as the project identifier for process management.

```rust
    fn run_selected(&mut self) {
        if let Some(idx) = self.list_state.selected() {
            if let Some(project) = self.store.projects.get(idx) {
                let path = Path::new(&project.path);
                if !path.exists() {
                    return;
                }

                // Git fetch before running
                self.git_fetch(path);

                // Install dependencies for JS projects
                if self.is_js_project() {
                    self.install_node_modules(path);
                }

                // Spawn terminal with claude
                self.spawn_terminal_with_claude(path);

                // Start dev server if not already running
                let project_id = idx as i64;
                if !self.process_manager.is_running(project_id) {
                    let cmd = project
                        .run_command
                        .clone()
                        .or_else(|| self.selected_detection.as_ref().and_then(|d| d.run_command.clone()));

                    if let Some(cmd) = cmd {
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

        if let Some(project) = self.selected_project() {
            let path = Path::new(&project.path);
            if path.exists() {
                self.selected_detection = detect::detect(path).ok();
                self.selected_git_status = git_status::get_status(path).ok();
            }
        }
    }
```

**Step 6: Keep helper methods unchanged** (git_fetch, install_node_modules, is_js_project, spawn_terminal_with_claude, maybe_refresh_ports)

These methods stay exactly as they are since they don't reference db/sync.

**Step 7: Rewrite `render` and its sub-methods**

Replace `render_project_list`:

```rust
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
```

Replace `render_details`:

```rust
    fn render_details(&self, frame: &mut Frame, area: Rect) {
        let content = if let Some(project) = self.selected_project() {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Name: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(&project.name),
                ]),
            ];

            if let Some(ref url) = project.repo_url {
                lines.push(Line::from(vec![
                    Span::styled("Repo: ", Style::default().fg(Color::DarkGray)),
                    Span::raw(url),
                ]));
            }

            lines.push(Line::from(""));

            let path = Path::new(&project.path);
            if !project.path.is_empty() && path.exists() {
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
            } else if project.path.is_empty() {
                lines.push(Line::from(Span::styled(
                    "No local path set",
                    Style::default().fg(Color::Red),
                )));
                if project.repo_url.is_some() {
                    lines.push(Line::from(Span::styled(
                        "Press 'g' to clone from repo",
                        Style::default().fg(Color::DarkGray),
                    )));
                }
            } else {
                lines.push(Line::from(Span::styled(
                    format!("Path not found: {}", project.path),
                    Style::default().fg(Color::Red),
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
```

Replace `render_help_bar`:

```rust
    fn render_help_bar(&self, frame: &mut Frame, area: Rect) {
        let gh_indicator = if self.gh_available { "" } else { " (gh unavailable)" };
        let help_text = format!(
            " [a]dd  [i]mport  [s]can  [g]it  [e]dit  [r]un  [x]stop  [d]el  [c]fg  [F5]  [q]uit{}",
            gh_indicator
        );

        let help = Paragraph::new(Line::from(
            Span::styled(help_text, Style::default().fg(Color::DarkGray)),
        ));

        frame.render_widget(help, area);
    }
```

Update `render` to also render `scan_selector`:

In the render method, after `self.repo_selector.render(frame, area);` add:
```rust
        self.scan_selector.render(frame, area);
```

**Step 8: Update `render_logs` to use index instead of project.id**

```rust
    fn render_logs(&mut self, frame: &mut Frame, area: Rect) {
        let project_idx = self.list_state.selected();
        let project_name = project_idx.and_then(|i| self.store.projects.get(i)).map(|p| p.name.clone());

        let (title, lines) = if let Some(idx) = project_idx {
            let project_id = idx as i64;
            if self.process_manager.is_running(project_id) {
                let output = self.process_manager.get_output(project_id);
                let title = format!(" Logs ({}) ", project_name.unwrap_or_default());
                (title, output)
            } else {
                (" Logs ".to_string(), vec!["No process running".to_string()])
            }
        } else {
            (" Logs ".to_string(), vec!["Select a project".to_string()])
        };

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
```

**Step 9: Commit**

```
git add src/app.rs
git commit -m "Rewrite app.rs to use ProjectStore, add scan keybind"
```

---

### Task 7: Make `scan_selector.items` accessible

The `scan_for_projects` method in Task 6 references `self.scan_selector.items`. The RepoSelector's items field needs to be public.

**Files:**
- Modify: `src/ui/selector.rs`

**Step 1: Check if `items` is already public**

Read `src/ui/selector.rs` to check the field visibility. If `items` is private, make it `pub`.

**Step 2: Commit if changed**

```
git add src/ui/selector.rs
git commit -m "Make RepoSelector items field public"
```

---

### Task 8: Delete removed modules

**Files:**
- Delete: `src/db.rs`
- Delete: `src/sync.rs`
- Delete: `src/machine.rs`
- Delete: `src/config.rs`

**Step 1: Delete the files**

```bash
rm src/db.rs src/sync.rs src/machine.rs src/config.rs
```

**Step 2: Commit**

```
git add -u src/db.rs src/sync.rs src/machine.rs src/config.rs
git commit -m "Remove db, sync, machine, and config modules"
```

---

### Task 9: Build, fix, and verify

**Step 1: Run `cargo build` and fix any compilation errors**

This is the integration step. There will likely be small issues (missing imports, type mismatches, field visibility). Fix them iteratively.

Common things to watch for:
- `RepoSelector` needs `items` to be `pub`
- `clap` dependency can be removed (no more CLI args)
- Process manager uses i64 IDs — we're now passing `idx as i64` which works
- Remove `use crate::sync` anywhere it still exists

**Step 2: Run `cargo test` to verify unit tests pass**

Run: `cargo test`
Expected: All tests pass (store tests + any remaining tests in other modules)

**Step 3: Run the app to smoke test**

Run: `cargo run`
Expected: App starts, shows empty list or prompts for scan on first run

**Step 4: Commit**

```
git add -A
git commit -m "Fix compilation errors after local-only redesign"
```

---

### Task 10: Remove `clap` dependency (optional cleanup)

Since we removed `--init`, the CLI has no flags. We can drop `clap`.

**Files:**
- Modify: `Cargo.toml` — remove `clap` from dependencies

**Step 1: Remove clap**

Remove from Cargo.toml:
```
clap = { version = "4", features = ["derive"] }
```

**Step 2: Verify**

Run: `cargo build`
Expected: Clean build

**Step 3: Commit**

```
git add Cargo.toml
git commit -m "Remove clap dependency, no CLI args needed"
```
