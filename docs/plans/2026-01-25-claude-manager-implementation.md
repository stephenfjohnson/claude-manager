# Claude Manager Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a terminal dashboard that tracks projects across machines, syncs via GitHub, shows git status, and manages dev servers.

**Architecture:** Ratatui component-based TUI with SQLite storage. Sync via `gh`/`git` CLI. Tokio for async process management and port scanning. Event-driven with action dispatch.

**Tech Stack:** Rust, Ratatui, Crossterm, rusqlite, Tokio, clap

---

## Task 1: Project Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/errors.rs`

**Step 1: Create Cargo.toml**

```toml
[package]
name = "claude-manager"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
crossterm = "0.28"
dirs = "5"
hostname = "0.4"
ratatui = "0.29"
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
uuid = { version = "1", features = ["v4"] }
```

**Step 2: Create src/errors.rs**

```rust
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    NotInitialized,
    GhNotAuthenticated,
    SyncFailed(String),
    DatabaseError(String),
    IoError(std::io::Error),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::NotInitialized => write!(f, "Run 'claude-manager --init' first"),
            AppError::GhNotAuthenticated => write!(f, "Run 'gh auth login' first"),
            AppError::SyncFailed(msg) => write!(f, "Sync failed: {}", msg),
            AppError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
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

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::DatabaseError(e.to_string())
    }
}
```

**Step 3: Create src/main.rs**

```rust
mod errors;

use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.init {
        println!("Init mode - not yet implemented");
    } else {
        println!("TUI mode - not yet implemented");
    }

    Ok(())
}
```

**Step 4: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

**Step 5: Test CLI parsing**

Run: `cargo run -- --help`
Expected: Shows help with `--init` option

**Step 6: Commit**

```bash
git add Cargo.toml src/
git commit -m "feat: project scaffold with CLI parsing"
```

---

## Task 2: Machine ID Generation

**Files:**
- Create: `src/machine.rs`
- Modify: `src/main.rs`

**Step 1: Create src/machine.rs**

```rust
use anyhow::Result;
use std::fs;
use std::path::PathBuf;

fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".claude-manager"))
}

pub fn machine_id_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("machine-id"))
}

pub fn get_or_create_machine_id() -> Result<String> {
    let path = machine_id_path()?;

    if path.exists() {
        return Ok(fs::read_to_string(&path)?.trim().to_string());
    }

    // Generate new ID: hostname-random8
    let host = hostname::get()?.to_string_lossy().to_string();
    let suffix: String = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let id = format!("{}-{}", host, suffix);

    // Ensure parent dir exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, &id)?;
    Ok(id)
}

pub fn get_machine_id() -> Result<Option<String>> {
    let path = machine_id_path()?;
    if path.exists() {
        Ok(Some(fs::read_to_string(&path)?.trim().to_string()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_machine_id_format() {
        // This test creates a real file in a temp dir
        let temp = env::temp_dir().join("claude-manager-test");
        fs::create_dir_all(&temp).unwrap();

        // We can't easily test get_or_create without mocking dirs::home_dir
        // So just test the format logic
        let host = "testhost";
        let suffix = &uuid::Uuid::new_v4().to_string()[..8];
        let id = format!("{}-{}", host, suffix);

        assert!(id.starts_with("testhost-"));
        assert_eq!(id.len(), "testhost-".len() + 8);
    }
}
```

**Step 2: Update src/main.rs to use machine module**

```rust
mod errors;
mod machine;

use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if cli.init {
        let machine_id = machine::get_or_create_machine_id()?;
        println!("Machine ID: {}", machine_id);
    } else {
        match machine::get_machine_id()? {
            Some(id) => println!("Machine ID: {}", id),
            None => println!("Not initialized. Run with --init first."),
        }
    }

    Ok(())
}
```

**Step 3: Run tests**

Run: `cargo test`
Expected: PASS

**Step 4: Test machine ID generation**

Run: `cargo run -- --init`
Expected: Prints "Machine ID: {hostname}-{8chars}"

**Step 5: Verify persistence**

Run: `cargo run -- --init` (again)
Expected: Same machine ID (read from file)

**Step 6: Commit**

```bash
git add src/machine.rs src/main.rs
git commit -m "feat: machine ID generation and persistence"
```

---

## Task 3: GitHub CLI Integration

**Files:**
- Create: `src/gh.rs`
- Modify: `src/main.rs`

**Step 1: Create src/gh.rs**

```rust
use anyhow::{bail, Result};
use std::process::Command;

/// Check if gh CLI is authenticated
pub fn check_auth() -> Result<bool> {
    let output = Command::new("gh")
        .args(["auth", "status"])
        .output()?;

    Ok(output.status.success())
}

/// Check if a repo exists on GitHub
pub fn repo_exists(repo_name: &str) -> Result<bool> {
    let output = Command::new("gh")
        .args(["repo", "view", repo_name])
        .output()?;

    Ok(output.status.success())
}

/// Create a private repo
pub fn create_repo(repo_name: &str) -> Result<()> {
    let status = Command::new("gh")
        .args(["repo", "create", repo_name, "--private", "--confirm"])
        .status()?;

    if !status.success() {
        bail!("Failed to create repo {}", repo_name);
    }
    Ok(())
}

/// Clone a repo to a path
pub fn clone_repo(repo_name: &str, dest: &std::path::Path) -> Result<()> {
    let status = Command::new("gh")
        .args(["repo", "clone", repo_name, dest.to_str().unwrap()])
        .status()?;

    if !status.success() {
        bail!("Failed to clone repo {}", repo_name);
    }
    Ok(())
}

/// Get the authenticated user's GitHub username
pub fn get_username() -> Result<String> {
    let output = Command::new("gh")
        .args(["api", "user", "--jq", ".login"])
        .output()?;

    if !output.status.success() {
        bail!("Failed to get GitHub username");
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
```

**Step 2: Update main.rs to check gh auth**

```rust
mod errors;
mod gh;
mod machine;

use crate::errors::AppError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Always check gh auth first
    if !gh::check_auth()? {
        return Err(AppError::GhNotAuthenticated.into());
    }

    if cli.init {
        let machine_id = machine::get_or_create_machine_id()?;
        let username = gh::get_username()?;
        println!("Machine ID: {}", machine_id);
        println!("GitHub user: {}", username);
    } else {
        match machine::get_machine_id()? {
            Some(id) => println!("Machine ID: {}", id),
            None => return Err(AppError::NotInitialized.into()),
        }
    }

    Ok(())
}
```

**Step 3: Test gh auth check**

Run: `cargo run -- --init`
Expected: If authenticated, shows machine ID and GitHub username

**Step 4: Commit**

```bash
git add src/gh.rs src/main.rs
git commit -m "feat: GitHub CLI integration for auth and repo operations"
```

---

## Task 4: Database Schema and Operations

**Files:**
- Create: `src/db.rs`

**Step 1: Create src/db.rs**

```rust
use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
}

#[derive(Debug, Clone)]
pub struct MachineLocation {
    pub id: i64,
    pub project_id: i64,
    pub machine_id: String,
    pub path: String,
    pub run_command: Option<String>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                repo_url TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS machine_locations (
                id INTEGER PRIMARY KEY,
                project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
                machine_id TEXT NOT NULL,
                path TEXT NOT NULL,
                run_command TEXT,
                UNIQUE(project_id, machine_id)
            );"
        )?;
        Ok(())
    }

    pub fn add_project(&self, name: &str, repo_url: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, repo_url) VALUES (?1, ?2)",
            params![name, repo_url],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_project(&self, id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self.conn.prepare("SELECT id, name, repo_url FROM projects ORDER BY name")?;
        let projects = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                repo_url: row.get(2)?,
            })
        })?;
        projects.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_location(&self, project_id: i64, machine_id: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO machine_locations (project_id, machine_id, path)
             VALUES (?1, ?2, ?3)",
            params![project_id, machine_id, path],
        )?;
        Ok(())
    }

    pub fn set_run_command(&self, project_id: i64, machine_id: &str, cmd: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE machine_locations SET run_command = ?1
             WHERE project_id = ?2 AND machine_id = ?3",
            params![cmd, project_id, machine_id],
        )?;
        Ok(())
    }

    pub fn get_location(&self, project_id: i64, machine_id: &str) -> Result<Option<MachineLocation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, machine_id, path, run_command
             FROM machine_locations WHERE project_id = ?1 AND machine_id = ?2"
        )?;

        let mut rows = stmt.query(params![project_id, machine_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MachineLocation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                machine_id: row.get(2)?,
                path: row.get(3)?,
                run_command: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_locations(&self, project_id: i64) -> Result<Vec<MachineLocation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, machine_id, path, run_command
             FROM machine_locations WHERE project_id = ?1"
        )?;

        let locs = stmt.query_map(params![project_id], |row| {
            Ok(MachineLocation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                machine_id: row.get(2)?,
                path: row.get(3)?,
                run_command: row.get(4)?,
            })
        })?;

        locs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_database_operations() {
        let temp_dir = std::env::temp_dir().join("claude-manager-db-test");
        fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test.db");

        // Clean up from previous runs
        let _ = fs::remove_file(&db_path);

        let db = Database::open(&db_path).unwrap();

        // Add a project
        let id = db.add_project("test-project", "github.com/user/test").unwrap();
        assert!(id > 0);

        // List projects
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");

        // Set location
        db.set_location(id, "machine-123", "/home/user/test").unwrap();

        // Get location
        let loc = db.get_location(id, "machine-123").unwrap().unwrap();
        assert_eq!(loc.path, "/home/user/test");
        assert!(loc.run_command.is_none());

        // Set run command
        db.set_run_command(id, "machine-123", Some("npm run dev")).unwrap();
        let loc = db.get_location(id, "machine-123").unwrap().unwrap();
        assert_eq!(loc.run_command, Some("npm run dev".to_string()));

        // Delete project (should cascade delete location)
        db.delete_project(id).unwrap();
        let projects = db.list_projects().unwrap();
        assert!(projects.is_empty());

        // Cleanup
        let _ = fs::remove_file(&db_path);
    }
}
```

**Step 2: Run tests**

Run: `cargo test test_database_operations`
Expected: PASS

**Step 3: Commit**

```bash
git add src/db.rs
git commit -m "feat: SQLite database schema and operations"
```

---

## Task 5: Sync Module

**Files:**
- Create: `src/sync.rs`
- Modify: `src/main.rs`

**Step 1: Create src/sync.rs**

```rust
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::fs;

use crate::gh;

const SYNC_REPO_NAME: &str = "claude-manager-sync";

pub fn sync_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".claude-manager").join("sync"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(sync_dir()?.join("projects.db"))
}

pub fn is_initialized() -> Result<bool> {
    Ok(sync_dir()?.exists())
}

/// Initialize the sync repo (create if not exists on GitHub, clone locally)
pub fn init() -> Result<()> {
    let username = gh::get_username()?;
    let full_repo = format!("{}/{}", username, SYNC_REPO_NAME);
    let sync_path = sync_dir()?;

    // Check if already cloned locally
    if sync_path.exists() {
        println!("Sync directory already exists, pulling latest...");
        return pull();
    }

    // Check if repo exists on GitHub
    if gh::repo_exists(&full_repo)? {
        println!("Cloning existing sync repo...");
        gh::clone_repo(&full_repo, &sync_path)?;
    } else {
        println!("Creating new sync repo...");
        gh::create_repo(&full_repo)?;
        gh::clone_repo(&full_repo, &sync_path)?;

        // Create initial commit with empty db
        let db_file = sync_path.join("projects.db");
        fs::File::create(&db_file)?;

        git_command(&sync_path, &["add", "projects.db"])?;
        git_command(&sync_path, &["commit", "-m", "Initial commit: empty database"])?;
        git_command(&sync_path, &["push", "-u", "origin", "main"])?;
    }

    Ok(())
}

/// Pull latest changes from remote
pub fn pull() -> Result<()> {
    let sync_path = sync_dir()?;
    if !sync_path.exists() {
        bail!("Sync directory does not exist");
    }
    git_command(&sync_path, &["pull", "origin", "main"])?;
    Ok(())
}

/// Commit and push changes
pub fn push(message: &str) -> Result<()> {
    let sync_path = sync_dir()?;
    git_command(&sync_path, &["add", "projects.db"])?;

    // Check if there are changes to commit
    let status = Command::new("git")
        .current_dir(&sync_path)
        .args(["diff", "--cached", "--quiet"])
        .status()?;

    if !status.success() {
        // There are staged changes, commit them
        git_command(&sync_path, &["commit", "-m", message])?;

        // Try to push, pull --rebase if it fails, then push again
        if git_command(&sync_path, &["push", "origin", "main"]).is_err() {
            git_command(&sync_path, &["pull", "--rebase", "origin", "main"])?;
            git_command(&sync_path, &["push", "origin", "main"])?;
        }
    }

    Ok(())
}

fn git_command(cwd: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .current_dir(cwd)
        .args(args)
        .status()?;

    if !status.success() {
        bail!("git {} failed", args.join(" "));
    }
    Ok(())
}
```

**Step 2: Update main.rs with full init flow**

```rust
mod db;
mod errors;
mod gh;
mod machine;
mod sync;

use crate::errors::AppError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Always check gh auth first
    if !gh::check_auth()? {
        return Err(AppError::GhNotAuthenticated.into());
    }

    if cli.init {
        run_init()?;
    } else {
        run_tui()?;
    }

    Ok(())
}

fn run_init() -> anyhow::Result<()> {
    println!("Initializing Claude Manager...\n");

    // 1. Generate machine ID
    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    // 2. Setup sync repo
    sync::init()?;
    println!("Sync repo ready.");

    // 3. Open database to ensure schema exists
    let db_path = sync::db_path()?;
    let _db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    // Check if initialized
    if !sync::is_initialized()? {
        return Err(AppError::NotInitialized.into());
    }

    // Pull latest
    println!("Syncing...");
    sync::pull()?;

    // Load machine ID and database
    let machine_id = machine::get_machine_id()?.ok_or(AppError::NotInitialized)?;
    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;

    let projects = db.list_projects()?;
    println!("\nProjects ({}):", projects.len());
    for p in &projects {
        let loc = db.get_location(p.id, &machine_id)?;
        let status = if loc.is_some() { "✓" } else { "✗" };
        println!("  {} {} ({})", status, p.name, p.repo_url);
    }

    println!("\nTUI not yet implemented. Press Ctrl+C to exit.");

    Ok(())
}
```

**Step 3: Test init flow (requires gh auth)**

Run: `cargo run -- --init`
Expected: Creates sync repo on GitHub, clones locally, initializes DB

**Step 4: Commit**

```bash
git add src/sync.rs src/main.rs src/db.rs
git commit -m "feat: sync module with GitHub repo init and pull/push"
```

---

## Task 6: Run Command Detection

**Files:**
- Create: `src/detect.rs`

**Step 1: Create src/detect.rs**

```rust
use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DetectedProject {
    pub package_manager: Option<PackageManager>,
    pub run_command: Option<String>,
    pub project_type: ProjectType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager {
    Pnpm,
    Yarn,
    Bun,
    Npm,
    Cargo,
    Go,
    Python,
}

impl PackageManager {
    pub fn as_str(&self) -> &'static str {
        match self {
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
            PackageManager::Npm => "npm",
            PackageManager::Cargo => "cargo",
            PackageManager::Go => "go",
            PackageManager::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectType {
    JavaScript,
    Rust,
    Go,
    Python,
    Unknown,
}

#[derive(Deserialize)]
struct PackageJson {
    scripts: Option<HashMap<String, String>>,
}

pub fn detect(path: &Path) -> Result<DetectedProject> {
    // Check for JS project
    if path.join("package.json").exists() {
        return detect_js(path);
    }

    // Check for Rust project
    if path.join("Cargo.toml").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Cargo),
            run_command: Some("cargo run".to_string()),
            project_type: ProjectType::Rust,
        });
    }

    // Check for Go project
    if path.join("go.mod").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Go),
            run_command: Some("go run .".to_string()),
            project_type: ProjectType::Go,
        });
    }

    // Check for Python project
    if path.join("manage.py").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Python),
            run_command: Some("python manage.py runserver".to_string()),
            project_type: ProjectType::Python,
        });
    }
    if path.join("main.py").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Python),
            run_command: Some("python main.py".to_string()),
            project_type: ProjectType::Python,
        });
    }

    Ok(DetectedProject {
        package_manager: None,
        run_command: None,
        project_type: ProjectType::Unknown,
    })
}

fn detect_js(path: &Path) -> Result<DetectedProject> {
    // Detect package manager from lockfile
    let pm = if path.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if path.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else if path.join("bun.lockb").exists() {
        PackageManager::Bun
    } else {
        PackageManager::Npm
    };

    // Read package.json to find best run script
    let pkg_path = path.join("package.json");
    let content = fs::read_to_string(&pkg_path)?;
    let pkg: PackageJson = serde_json::from_str(&content)?;

    let run_cmd = if let Some(scripts) = pkg.scripts {
        // Preference order: dev > start > serve > watch
        let script = ["dev", "start", "serve", "watch"]
            .iter()
            .find(|s| scripts.contains_key(**s))
            .map(|s| *s);

        script.map(|s| format!("{} run {}", pm.as_str(), s))
    } else {
        None
    };

    Ok(DetectedProject {
        package_manager: Some(pm),
        run_command: run_cmd,
        project_type: ProjectType::JavaScript,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_js_with_pnpm() {
        let temp = std::env::temp_dir().join("claude-manager-detect-test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        fs::write(temp.join("pnpm-lock.yaml"), "").unwrap();
        fs::write(temp.join("package.json"), r#"{"scripts": {"dev": "vite"}}"#).unwrap();

        let detected = detect(&temp).unwrap();
        assert_eq!(detected.package_manager, Some(PackageManager::Pnpm));
        assert_eq!(detected.run_command, Some("pnpm run dev".to_string()));
        assert_eq!(detected.project_type, ProjectType::JavaScript);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_detect_rust() {
        let temp = std::env::temp_dir().join("claude-manager-detect-rust");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        fs::write(temp.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let detected = detect(&temp).unwrap();
        assert_eq!(detected.package_manager, Some(PackageManager::Cargo));
        assert_eq!(detected.run_command, Some("cargo run".to_string()));
        assert_eq!(detected.project_type, ProjectType::Rust);

        let _ = fs::remove_dir_all(&temp);
    }
}
```

**Step 2: Run tests**

Run: `cargo test detect`
Expected: PASS

**Step 3: Commit**

```bash
git add src/detect.rs
git commit -m "feat: run command detection for JS, Rust, Go, Python"
```

---

## Task 7: Basic TUI Framework

**Files:**
- Create: `src/tui.rs`
- Create: `src/app.rs`
- Modify: `src/main.rs`

**Step 1: Create src/tui.rs**

```rust
use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io::{self, Stdout};
use std::time::Duration;

use crate::app::App;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }

    pub fn run(&mut self, app: &mut App) -> Result<()> {
        loop {
            self.terminal.draw(|frame| app.render(frame))?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => break,
                            _ => app.handle_key(key.code),
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}
```

**Step 2: Create src/app.rs**

```rust
use crossterm::event::KeyCode;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

use crate::db::{Database, Project, MachineLocation};
use crate::detect;

pub struct App {
    pub projects: Vec<Project>,
    pub machine_id: String,
    pub db: Database,
    pub list_state: ListState,
    pub selected_location: Option<MachineLocation>,
    pub selected_detection: Option<detect::DetectedProject>,
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
        };
        app.update_selected_details();
        Ok(app)
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Up | KeyCode::Char('k') => self.previous(),
            KeyCode::Down | KeyCode::Char('j') => self.next(),
            _ => {}
        }
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
                self.selected_location = self.db.get_location(project.id, &self.machine_id).ok().flatten();

                if let Some(ref loc) = self.selected_location {
                    self.selected_detection = detect::detect(std::path::Path::new(&loc.path)).ok();
                }
            }
        }
    }

    pub fn selected_project(&self) -> Option<&Project> {
        self.list_state.selected().and_then(|i| self.projects.get(i))
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(frame.area());

        self.render_project_list(frame, chunks[0]);
        self.render_details(frame, chunks[1]);
    }

    fn render_project_list(&mut self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = self.projects.iter().map(|p| {
            let has_path = self.db.get_location(p.id, &self.machine_id).ok().flatten().is_some();
            let indicator = if has_path { "✓" } else { "✗" };

            let line = Line::from(vec![
                Span::styled(
                    format!(" {} ", indicator),
                    Style::default().fg(if has_path { Color::Green } else { Color::Red }),
                ),
                Span::raw(&p.name),
            ]);
            ListItem::new(line)
        }).collect();

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
```

**Step 3: Update src/main.rs**

```rust
mod app;
mod db;
mod detect;
mod errors;
mod gh;
mod machine;
mod sync;
mod tui;

use crate::errors::AppError;
use clap::Parser;

#[derive(Parser)]
#[command(name = "claude-manager")]
#[command(about = "Personal project dashboard across machines")]
struct Cli {
    /// Initialize Claude Manager (run once per machine)
    #[arg(long)]
    init: bool,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Always check gh auth first
    if !gh::check_auth()? {
        return Err(AppError::GhNotAuthenticated.into());
    }

    if cli.init {
        run_init()?;
    } else {
        run_tui()?;
    }

    Ok(())
}

fn run_init() -> anyhow::Result<()> {
    println!("Initializing Claude Manager...\n");

    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    sync::init()?;
    println!("Sync repo ready.");

    let db_path = sync::db_path()?;
    let _db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}

fn run_tui() -> anyhow::Result<()> {
    if !sync::is_initialized()? {
        return Err(AppError::NotInitialized.into());
    }

    sync::pull()?;

    let machine_id = machine::get_machine_id()?.ok_or(AppError::NotInitialized)?;
    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;

    let mut app = app::App::new(db, machine_id)?;
    let mut tui = tui::Tui::new()?;
    tui.run(&mut app)?;

    Ok(())
}
```

**Step 4: Test TUI launches**

Run: `cargo run`
Expected: TUI opens showing project list (may be empty), quit with 'q'

**Step 5: Commit**

```bash
git add src/tui.rs src/app.rs src/main.rs
git commit -m "feat: basic TUI framework with project list and details"
```

---

## Task 8: Add Project Action

**Files:**
- Create: `src/ui/input.rs`
- Create: `src/ui/mod.rs`
- Modify: `src/app.rs`

**Step 1: Create src/ui/mod.rs**

```rust
pub mod input;
```

**Step 2: Create src/ui/input.rs**

```rust
use crossterm::event::KeyCode;
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub struct InputDialog {
    pub title: String,
    pub value: String,
    pub visible: bool,
}

impl InputDialog {
    pub fn new(title: &str) -> Self {
        Self {
            title: title.to_string(),
            value: String::new(),
            visible: false,
        }
    }

    pub fn show(&mut self) {
        self.value.clear();
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
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
        let height = 3;
        let x = (area.width.saturating_sub(width)) / 2;
        let y = (area.height.saturating_sub(height)) / 2;
        let dialog_area = Rect::new(x, y, width, height);

        frame.render_widget(Clear, dialog_area);

        let input = Paragraph::new(self.value.as_str())
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" {} ", self.title))
                    .border_style(Style::default().fg(Color::Cyan)),
            );
        frame.render_widget(input, dialog_area);
    }
}
```

**Step 3: Modify src/app.rs to add input handling**

Add to imports at top:
```rust
use crate::ui::input::InputDialog;
use crate::sync;
use std::path::Path;
```

Add fields to App struct:
```rust
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
}

#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    AddName,
    AddUrl,
    SetPath,
}
```

Update App::new:
```rust
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
    };
    app.update_selected_details();
    Ok(app)
}
```

Update handle_key:
```rust
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
        _ => {}
    }
}
```

Add action methods:
```rust
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
        if self.db.set_location(project_id, &self.machine_id, path).is_ok() {
            let _ = sync::push(&format!("Set path for {} on {}", project_name, self.machine_id));
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

pub fn is_input_mode(&self) -> bool {
    self.input_mode != InputMode::Normal
}
```

Update render method to show dialogs:
```rust
pub fn render(&mut self, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(frame.area());

    self.render_project_list(frame, chunks[0]);
    self.render_details(frame, chunks[1]);

    // Render input dialogs on top
    let area = frame.area();
    self.name_input.render(frame, area);
    self.url_input.render(frame, area);
    self.path_input.render(frame, area);
}
```

**Step 4: Update src/main.rs imports**

Add to imports:
```rust
mod ui;
```

**Step 5: Update tui.rs to check input mode before quit**

```rust
if key.kind == KeyEventKind::Press {
    if key.code == KeyCode::Char('q') && !app.is_input_mode() {
        break;
    }
    app.handle_key(key.code);
}
```

**Step 6: Test adding a project**

Run: `cargo run`
- Press 'a', enter name, enter URL
- Should appear in list
Expected: Project added and synced

**Step 7: Commit**

```bash
git add src/ui/ src/app.rs src/main.rs src/tui.rs
git commit -m "feat: add/delete project and set path actions"
```

---

## Task 9: Help Bar

**Files:**
- Modify: `src/app.rs`

**Step 1: Add help bar to render**

Update the render method to use a vertical layout with help bar at bottom:

```rust
pub fn render(&mut self, frame: &mut Frame) {
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area());

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(main_chunks[0]);

    self.render_project_list(frame, content_chunks[0]);
    self.render_details(frame, content_chunks[1]);
    self.render_help_bar(frame, main_chunks[1]);

    // Render input dialogs on top
    let area = frame.area();
    self.name_input.render(frame, area);
    self.url_input.render(frame, area);
    self.path_input.render(frame, area);
}

fn render_help_bar(&self, frame: &mut Frame, area: Rect) {
    let help_text = " [a]dd  [p]ath  [d]elete  [q]uit ";
    let machine_text = format!(" Machine: {} ", self.machine_id);

    let help = Paragraph::new(Line::from(vec![
        Span::styled(help_text, Style::default().fg(Color::DarkGray)),
        Span::raw(" │ "),
        Span::styled(machine_text, Style::default().fg(Color::Cyan)),
    ]));

    frame.render_widget(help, area);
}
```

**Step 2: Test help bar displays**

Run: `cargo run`
Expected: Help bar visible at bottom with machine ID

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: help bar with keybindings and machine ID"
```

---

## Task 10: Process Management

**Files:**
- Create: `src/process.rs`
- Modify: `src/app.rs`

**Step 1: Create src/process.rs**

```rust
use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ProcessManager {
    processes: HashMap<i64, Child>,
    output_buffers: Arc<Mutex<HashMap<i64, Vec<String>>>>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            output_buffers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&mut self, project_id: i64, cwd: &Path, command: &str) -> Result<()> {
        // Parse command into program and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let program = parts[0];
        let args = &parts[1..];

        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        // Setup output capture
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        {
            let mut buffers = self.output_buffers.lock().unwrap();
            buffers.insert(project_id, Vec::new());
        }

        // Spawn threads to capture output
        if let Some(stdout) = stdout {
            let buffers = Arc::clone(&self.output_buffers);
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut buffers) = buffers.lock() {
                        if let Some(buf) = buffers.get_mut(&project_id) {
                            buf.push(line);
                            // Keep last 1000 lines
                            if buf.len() > 1000 {
                                buf.remove(0);
                            }
                        }
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let buffers = Arc::clone(&self.output_buffers);
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut buffers) = buffers.lock() {
                        if let Some(buf) = buffers.get_mut(&project_id) {
                            buf.push(format!("[stderr] {}", line));
                            if buf.len() > 1000 {
                                buf.remove(0);
                            }
                        }
                    }
                }
            });
        }

        self.processes.insert(project_id, child);
        Ok(())
    }

    pub fn stop(&mut self, project_id: i64) -> Result<()> {
        if let Some(mut child) = self.processes.remove(&project_id) {
            // Try graceful shutdown first
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }
            #[cfg(windows)]
            {
                let _ = child.kill();
            }

            // Wait a bit then force kill if needed
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = child.kill();
            let _ = child.wait();
        }

        // Clean up buffer
        if let Ok(mut buffers) = self.output_buffers.lock() {
            buffers.remove(&project_id);
        }

        Ok(())
    }

    pub fn is_running(&mut self, project_id: i64) -> bool {
        if let Some(child) = self.processes.get_mut(&project_id) {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process exited, remove it
                    self.processes.remove(&project_id);
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            false
        }
    }

    pub fn get_output(&self, project_id: i64) -> Vec<String> {
        if let Ok(buffers) = self.output_buffers.lock() {
            buffers.get(&project_id).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn running_projects(&self) -> Vec<i64> {
        self.processes.keys().cloned().collect()
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Update Cargo.toml for libc**

Add to dependencies:
```toml
[target.'cfg(unix)'.dependencies]
libc = "0.2"
```

**Step 3: Update src/app.rs to integrate process manager**

Add to imports:
```rust
use crate::process::ProcessManager;
```

Add field to App:
```rust
pub process_manager: ProcessManager,
```

Initialize in App::new:
```rust
process_manager: ProcessManager::new(),
```

Add run/stop handling to handle_normal_key:
```rust
KeyCode::Char('r') => self.run_selected(),
KeyCode::Char('s') => self.stop_selected(),
```

Add methods:
```rust
fn run_selected(&mut self) {
    if let Some(project) = self.selected_project() {
        let project_id = project.id;

        if self.process_manager.is_running(project_id) {
            return; // Already running
        }

        if let Some(ref loc) = self.selected_location {
            let path = Path::new(&loc.path);

            // Determine command: override > detected > none
            let cmd = loc.run_command.clone()
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
```

**Step 4: Update main.rs imports**

```rust
mod process;
```

**Step 5: Update render_project_list to show running indicator**

```rust
fn render_project_list(&mut self, frame: &mut Frame, area: Rect) {
    let items: Vec<ListItem> = self.projects.iter().map(|p| {
        let has_path = self.db.get_location(p.id, &self.machine_id).ok().flatten().is_some();
        let is_running = self.process_manager.is_running(p.id);

        let status = if is_running {
            Span::styled(" ● ", Style::default().fg(Color::Green))
        } else if has_path {
            Span::styled(" ✓ ", Style::default().fg(Color::Green))
        } else {
            Span::styled(" ✗ ", Style::default().fg(Color::Red))
        };

        let line = Line::from(vec![status, Span::raw(&p.name)]);
        ListItem::new(line)
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(" Projects "))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    frame.render_stateful_widget(list, area, &mut self.list_state);
}
```

**Step 6: Update help bar**

```rust
let help_text = " [a]dd  [p]ath  [r]un  [s]top  [d]elete  [q]uit ";
```

**Step 7: Test process management**

Run: `cargo run`
- Add a project, set path to a JS project with dev script
- Press 'r' to run, 's' to stop
Expected: Project shows running indicator

**Step 8: Commit**

```bash
git add Cargo.toml src/process.rs src/app.rs src/main.rs
git commit -m "feat: process management with run/stop"
```

---

## Task 11: Logs Panel

**Files:**
- Modify: `src/app.rs`

**Step 1: Add logs panel state**

Add field to App:
```rust
show_logs: bool,
```

Initialize in App::new:
```rust
show_logs: true,
```

**Step 2: Update render for logs panel**

```rust
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
```

**Step 3: Add Ctrl+L toggle**

In handle_normal_key:
```rust
KeyCode::Char('l') if key_modifiers.contains(KeyModifiers::CONTROL) => {
    self.show_logs = !self.show_logs;
}
```

Note: Need to pass modifiers to handle_key. Update signature and tui.rs.

**Step 4: Update tui.rs to pass modifiers**

```rust
use crossterm::event::{KeyModifiers, ...};

// In the key handling:
app.handle_key(key.code, key.modifiers);
```

Update App::handle_key signature:
```rust
pub fn handle_key(&mut self, key: KeyCode, modifiers: KeyModifiers) {
    match self.input_mode {
        InputMode::Normal => self.handle_normal_key(key, modifiers),
        // ... input modes don't need modifiers
    }
}

fn handle_normal_key(&mut self, key: KeyCode, modifiers: KeyModifiers) {
    match key {
        KeyCode::Char('l') if modifiers.contains(KeyModifiers::CONTROL) => {
            self.show_logs = !self.show_logs;
        }
        // ... rest unchanged
    }
}
```

**Step 5: Test logs panel**

Run: `cargo run`
- Run a project
- Should see logs panel with output
- Ctrl+L to toggle
Expected: Logs show and update

**Step 6: Commit**

```bash
git add src/app.rs src/tui.rs
git commit -m "feat: logs panel with toggle"
```

---

## Task 12: Port Scanning

**Files:**
- Create: `src/ports.rs`
- Modify: `src/app.rs`

**Step 1: Create src/ports.rs**

```rust
use anyhow::Result;
use std::collections::HashMap;
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

pub fn scan_ports() -> Vec<PortInfo> {
    let ports_to_check: Vec<u16> = (3000..=3010)
        .chain(4000..=4010)
        .chain(5000..=5010)
        .chain(8000..=8010)
        .chain(std::iter::once(8080))
        .chain(std::iter::once(9000))
        .collect();

    let mut results = Vec::new();

    for port in ports_to_check {
        if is_port_open(port) {
            let (pid, name) = get_process_for_port(port).unwrap_or((None, None));
            results.push(PortInfo {
                port,
                pid,
                process_name: name,
            });
        }
    }

    results
}

fn is_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(50),
    ).is_ok()
}

#[cfg(target_os = "linux")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::fs;
    use std::path::Path;

    // Read /proc/net/tcp to find the inode for this port
    let tcp_content = fs::read_to_string("/proc/net/tcp")?;
    let port_hex = format!("{:04X}", port);

    let mut target_inode: Option<u64> = None;

    for line in tcp_content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        // local_address is like "00000000:0CEA" (ip:port in hex)
        let local_addr = parts[1];
        if let Some(local_port) = local_addr.split(':').nth(1) {
            if local_port == port_hex {
                // Found it, get inode (column 9)
                if let Ok(inode) = parts[9].parse::<u64>() {
                    target_inode = Some(inode);
                    break;
                }
            }
        }
    }

    let inode = match target_inode {
        Some(i) => i,
        None => return Ok((None, None)),
    };

    // Find which process owns this inode by checking /proc/*/fd/
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let fd_path = Path::new("/proc").join(&pid_str).join("fd");
        if let Ok(fd_entries) = fs::read_dir(&fd_path) {
            for fd_entry in fd_entries.flatten() {
                if let Ok(link) = fs::read_link(fd_entry.path()) {
                    let link_str = link.to_string_lossy();
                    if link_str.contains(&format!("socket:[{}]", inode)) {
                        // Found the process, get its name
                        let comm_path = Path::new("/proc").join(&pid_str).join("comm");
                        let name = fs::read_to_string(comm_path)
                            .ok()
                            .map(|s| s.trim().to_string());
                        return Ok((Some(pid), name));
                    }
                }
            }
        }
    }

    Ok((None, None))
}

#[cfg(target_os = "macos")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::process::Command;

    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        // Name is parts[8], should end with ":port"
        let name_field = parts[8];
        if name_field.ends_with(&format!(":{}", port)) {
            let pid: u32 = parts[1].parse().ok()?;
            let name = Some(parts[0].to_string());
            return Ok((Some(pid), name));
        }
    }

    Ok((None, None))
}

#[cfg(target_os = "windows")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::process::Command;

    let output = Command::new("netstat")
        .args(["-ano"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // Local Address is like "0.0.0.0:3000"
        let local_addr = parts[1];
        if local_addr.ends_with(&format!(":{}", port)) {
            let pid: u32 = parts[4].parse().ok()?;
            return Ok((Some(pid), None)); // Windows netstat doesn't show process name
        }
    }

    Ok((None, None))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_process_for_port(_port: u16) -> Result<(Option<u32>, Option<String>)> {
    Ok((None, None))
}
```

**Step 2: Update src/app.rs to show ports**

Add imports and field:
```rust
use crate::ports::{self, PortInfo};

// In App struct:
pub port_info: Vec<PortInfo>,
last_port_scan: std::time::Instant,
```

Initialize in App::new:
```rust
port_info: ports::scan_ports(),
last_port_scan: std::time::Instant::now(),
```

Add port refresh logic (call in render or handle_key):
```rust
fn maybe_refresh_ports(&mut self) {
    if self.last_port_scan.elapsed() > std::time::Duration::from_secs(30) {
        self.port_info = ports::scan_ports();
        self.last_port_scan = std::time::Instant::now();
    }
}
```

Update render to show ports bar:
```rust
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

    // Input dialogs on top...
}

fn render_ports_bar(&self, frame: &mut Frame, area: Rect) {
    let running_pids: HashMap<u32, i64> = self.process_manager.running_projects()
        .iter()
        .filter_map(|&proj_id| {
            // We'd need to track PIDs in process manager for proper linking
            // For now, just show port info
            None::<(u32, i64)>
        })
        .collect();

    let port_spans: Vec<Span> = self.port_info.iter().map(|p| {
        let label = match (&p.process_name, p.pid) {
            (Some(name), Some(pid)) => format!("{}({},{})", p.port, name, pid),
            (Some(name), None) => format!("{}({})", p.port, name),
            (None, Some(pid)) => format!("{}(PID:{})", p.port, pid),
            (None, None) => format!("{}", p.port),
        };
        Span::styled(format!(" {} ", label), Style::default().fg(Color::Yellow))
    }).collect();

    let content = if port_spans.is_empty() {
        Line::from(Span::styled(" No ports in use ", Style::default().fg(Color::DarkGray)))
    } else {
        let mut spans = vec![Span::styled("Ports:", Style::default().fg(Color::DarkGray))];
        spans.extend(port_spans);
        Line::from(spans)
    };

    let para = Paragraph::new(content);
    frame.render_widget(para, area);
}
```

**Step 3: Add to main.rs**

```rust
mod ports;
```

**Step 4: Test port scanning**

Run: `cargo run`
- Start a dev server on port 3000 externally
- Should show in ports bar
Expected: Ports displayed with PID/process name

**Step 5: Commit**

```bash
git add src/ports.rs src/app.rs src/main.rs
git commit -m "feat: port scanning with process info"
```

---

## Task 13: Git Status

**Files:**
- Create: `src/git_status.rs`
- Modify: `src/app.rs`

**Step 1: Create src/git_status.rs**

```rust
use anyhow::Result;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct GitStatus {
    pub branch: String,
    pub staged: usize,
    pub modified: usize,
    pub untracked: usize,
    pub ahead: usize,
    pub behind: usize,
}

pub fn get_status(path: &Path) -> Result<GitStatus> {
    let git_dir = path.join(".git");
    if !git_dir.exists() {
        anyhow::bail!("Not a git repository");
    }

    let mut status = GitStatus::default();

    // Get branch name
    let output = Command::new("git")
        .current_dir(path)
        .args(["branch", "--show-current"])
        .output()?;
    status.branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if status.branch.is_empty() {
        status.branch = "HEAD".to_string();
    }

    // Get status counts
    let output = Command::new("git")
        .current_dir(path)
        .args(["status", "--porcelain"])
        .output()?;
    let porcelain = String::from_utf8_lossy(&output.stdout);

    for line in porcelain.lines() {
        if line.len() < 2 {
            continue;
        }
        let index = line.chars().next().unwrap_or(' ');
        let worktree = line.chars().nth(1).unwrap_or(' ');

        if index != ' ' && index != '?' {
            status.staged += 1;
        }
        if worktree == 'M' || worktree == 'D' {
            status.modified += 1;
        }
        if index == '?' {
            status.untracked += 1;
        }
    }

    // Get ahead/behind (may fail if no upstream)
    let output = Command::new("git")
        .current_dir(path)
        .args(["rev-list", "--left-right", "--count", "@{u}...HEAD"])
        .output();

    if let Ok(output) = output {
        if output.status.success() {
            let counts = String::from_utf8_lossy(&output.stdout);
            let parts: Vec<&str> = counts.trim().split_whitespace().collect();
            if parts.len() == 2 {
                status.behind = parts[0].parse().unwrap_or(0);
                status.ahead = parts[1].parse().unwrap_or(0);
            }
        }
    }

    Ok(status)
}
```

**Step 2: Update src/app.rs**

Add imports and field:
```rust
use crate::git_status::{self, GitStatus};

// In App struct:
pub selected_git_status: Option<GitStatus>,
```

Initialize in App::new:
```rust
selected_git_status: None,
```

Update update_selected_details:
```rust
fn update_selected_details(&mut self) {
    self.selected_location = None;
    self.selected_detection = None;
    self.selected_git_status = None;

    if let Some(idx) = self.list_state.selected() {
        if let Some(project) = self.projects.get(idx) {
            self.selected_location = self.db.get_location(project.id, &self.machine_id).ok().flatten();

            if let Some(ref loc) = self.selected_location {
                let path = Path::new(&loc.path);
                self.selected_detection = detect::detect(path).ok();
                self.selected_git_status = git_status::get_status(path).ok();
            }
        }
    }
}
```

Update render_details:
```rust
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
                        Style::default().fg(if git.ahead > 0 { Color::Yellow } else { Color::White }),
                    ),
                    Span::raw("  "),
                    Span::styled("Behind: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        git.behind.to_string(),
                        Style::default().fg(if git.behind > 0 { Color::Red } else { Color::White }),
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
```

**Step 3: Add to main.rs**

```rust
mod git_status;
```

**Step 4: Add Enter to refresh selected project status**

In handle_normal_key:
```rust
KeyCode::Enter => self.update_selected_details(),
```

**Step 5: Test git status**

Run: `cargo run`
- Select a project with a valid path
- Should show branch, staged, modified, ahead/behind
Expected: Git status displays correctly

**Step 6: Commit**

```bash
git add src/git_status.rs src/app.rs src/main.rs
git commit -m "feat: git status display with ahead/behind"
```

---

## Task 14: Project Scanner for Init

**Files:**
- Create: `src/scanner.rs`
- Modify: `src/main.rs`

**Step 1: Create src/scanner.rs**

```rust
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct ScannedProject {
    pub path: PathBuf,
    pub name: String,
    pub remote_url: Option<String>,
}

pub fn scan_directories() -> Vec<ScannedProject> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Vec::new(),
    };

    let search_dirs = [
        home.join("projects"),
        home.join("Projects"),
        home.join("dev"),
        home.join("Dev"),
        home.join("code"),
        home.join("Code"),
        home.join("src"),
        home.join("Documents").join("Projects"),
        home.join("Documents").join("projects"),
    ];

    let mut results = Vec::new();

    for dir in search_dirs {
        if dir.exists() {
            if let Ok(projects) = scan_directory(&dir) {
                results.extend(projects);
            }
        }
    }

    // Deduplicate by path
    results.sort_by(|a, b| a.path.cmp(&b.path));
    results.dedup_by(|a, b| a.path == b.path);

    results
}

fn scan_directory(dir: &Path) -> Result<Vec<ScannedProject>> {
    let mut results = Vec::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Skip hidden directories
        if path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        // Check if it's a git repo
        if path.join(".git").exists() {
            let name = path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            let remote_url = get_git_remote(&path);

            results.push(ScannedProject {
                path,
                name,
                remote_url,
            });
        }
    }

    Ok(results)
}

fn get_git_remote(path: &Path) -> Option<String> {
    let output = Command::new("git")
        .current_dir(path)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}
```

**Step 2: Update src/main.rs to use scanner in init**

```rust
mod scanner;

fn run_init() -> anyhow::Result<()> {
    println!("Initializing Claude Manager...\n");

    let machine_id = machine::get_or_create_machine_id()?;
    println!("Machine ID: {}", machine_id);

    sync::init()?;
    println!("Sync repo ready.");

    let db_path = sync::db_path()?;
    let db = db::Database::open(&db_path)?;
    println!("Database initialized.");

    // Offer to scan for projects
    println!("\nScan for existing git repos in common directories? [Y/n] ");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

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

            println!("\nEnter numbers to import (comma-separated), 'all', or 'none': ");
            input.clear();
            std::io::stdin().read_line(&mut input)?;

            let to_import: Vec<usize> = if input.trim() == "all" {
                (0..found.len()).collect()
            } else if input.trim() == "none" {
                Vec::new()
            } else {
                input.trim()
                    .split(',')
                    .filter_map(|s| s.trim().parse::<usize>().ok())
                    .filter(|&n| n > 0 && n <= found.len())
                    .map(|n| n - 1)
                    .collect()
            };

            for idx in to_import {
                let proj = &found[idx];
                let url = proj.remote_url.as_deref().unwrap_or("");

                match db.add_project(&proj.name, url) {
                    Ok(id) => {
                        db.set_location(id, &machine_id, proj.path.to_str().unwrap_or(""))?;
                        println!("  Imported: {}", proj.name);
                    }
                    Err(e) => {
                        println!("  Skipped {} ({})", proj.name, e);
                    }
                }
            }

            // Sync to GitHub
            sync::push("Import projects from scan")?;
            println!("\nProjects synced to GitHub.");
        }
    }

    println!("\nInitialization complete! Run 'claude-manager' to start.");
    Ok(())
}
```

**Step 3: Test scanner**

Run: `cargo run -- --init` (on a fresh setup or after removing ~/.claude-manager)
Expected: Scans directories, shows found projects, allows selection

**Step 4: Commit**

```bash
git add src/scanner.rs src/main.rs
git commit -m "feat: project scanner for init bulk import"
```

---

## Task 15: Final Polish

**Files:**
- Modify: `src/app.rs`
- Modify: `src/tui.rs`

**Step 1: Add F5 full refresh**

In handle_normal_key:
```rust
KeyCode::F(5) => self.full_refresh(),
```

Add method:
```rust
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
```

**Step 2: Add 'e' to edit run command**

Add new input mode:
```rust
#[derive(Default, PartialEq)]
enum InputMode {
    #[default]
    Normal,
    AddName,
    AddUrl,
    SetPath,
    EditRunCmd,
}
```

Add input dialog in App:
```rust
run_cmd_input: InputDialog::new("Run Command"),
```

Initialize in App::new:
```rust
run_cmd_input: InputDialog::new("Run Command"),
```

Handle in handle_normal_key:
```rust
KeyCode::Char('e') => {
    if self.selected_location.is_some() {
        self.run_cmd_input.show();
        self.input_mode = InputMode::EditRunCmd;
    }
}
```

Handle in handle_key:
```rust
InputMode::EditRunCmd => {
    if let Some(cmd) = self.run_cmd_input.handle_key(key) {
        self.set_run_command(if cmd.is_empty() { None } else { Some(&cmd) });
        self.input_mode = InputMode::Normal;
    }
    if !self.run_cmd_input.visible {
        self.input_mode = InputMode::Normal;
    }
}
```

Add method:
```rust
fn set_run_command(&mut self, cmd: Option<&str>) {
    if let Some(project) = self.selected_project() {
        let project_id = project.id;
        let project_name = project.name.clone();
        if self.db.set_run_command(project_id, &self.machine_id, cmd).is_ok() {
            let _ = sync::push(&format!("Set run command for {} on {}", project_name, self.machine_id));
            self.update_selected_details();
        }
    }
}
```

Render the dialog:
```rust
self.run_cmd_input.render(frame, area);
```

**Step 3: Update help bar**

```rust
let help_text = " [a]dd  [p]ath  [e]dit cmd  [r]un  [s]top  [d]elete  [F5]refresh  [q]uit ";
```

**Step 4: Handle quit with running processes**

Update tui.rs run loop:
```rust
if key.code == KeyCode::Char('q') && !app.is_input_mode() {
    // Stop all running processes before quitting
    for project_id in app.process_manager.running_projects() {
        let _ = app.process_manager.stop(project_id);
    }
    break;
}
```

**Step 5: Test all features**

Run: `cargo run`
- Test all keybindings work
- Test F5 refresh
- Test 'e' to edit run command
- Test quit stops processes
Expected: All features working

**Step 6: Final commit**

```bash
git add src/app.rs src/tui.rs
git commit -m "feat: F5 refresh, edit run command, clean quit"
```

---

## Summary

The implementation is split into 15 tasks:

1. **Project Scaffold** - Cargo.toml, main.rs, errors.rs
2. **Machine ID** - Generate and persist machine identifier
3. **GitHub CLI** - Auth check and repo operations via `gh`
4. **Database** - SQLite schema and CRUD operations
5. **Sync Module** - Git pull/push for sync repo
6. **Run Detection** - Auto-detect package manager and run command
7. **Basic TUI** - Ratatui framework with project list
8. **Add Project** - Input dialogs for add/path/delete
9. **Help Bar** - Keybindings display
10. **Process Management** - Run/stop dev servers
11. **Logs Panel** - Show process output
12. **Port Scanning** - Detect listening ports
13. **Git Status** - Show branch, staged, ahead/behind
14. **Project Scanner** - Bulk import during init
15. **Final Polish** - F5 refresh, edit command, clean quit

Each task is self-contained with tests and commits.
