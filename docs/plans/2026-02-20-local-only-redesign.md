# Claude Manager - Local-Only Redesign

Redesign claude-manager to be fully local with no synced database. Each machine maintains its own independent project list.

## Motivation

- Remove cross-machine sync complexity (GitHub sync repo, machine IDs, conflict resolution)
- Simplify startup (no internet required, no `--init` ceremony)
- Make `gh` CLI optional (only needed for "add from GitHub" feature)

## Key Decisions

- **Fully local** — no cross-machine awareness, no shared state
- **TOML config file** — replaces SQLite database, easy to hand-edit
- **No `--init` flag** — first run auto-detects and prompts
- **`gh` CLI optional** — app works without it, GitHub features gracefully disabled
- **Scan on demand** — prompt on first launch, `[s]` keybind to rescan anytime

---

## Data Model

**File: `~/.claude-manager/projects.toml`**

```toml
install_dir = "~/Projects"

[[projects]]
name = "claude-manager"
repo_url = "https://github.com/stephenfjohnson/claude-manager.git"
path = "/home/user/Projects/claude-manager"
run_command = "cargo run"

[[projects]]
name = "my-web-app"
repo_url = "https://github.com/stephenfjohnson/my-web-app.git"
path = "/home/user/Projects/my-web-app"
# run_command omitted = auto-detected at runtime
```

Each project has:
- `name` (required) — project display name
- `repo_url` (optional) — GitHub remote URL
- `path` (required) — local filesystem path
- `run_command` (optional) — override for auto-detected command

---

## File Structure

```
~/.claude-manager/
└── projects.toml    # All config + project list (single file)
```

Compared to current:
```
~/.claude-manager/           (REMOVED)
├── machine-id               ← REMOVED
├── config.toml              ← MERGED into projects.toml
└── sync/                    ← REMOVED entirely
    ├── .git/
    └── projects.db
```

---

## Startup Flow

Every launch follows the same flow (no `--init` vs normal distinction):

1. Check if `~/.claude-manager/projects.toml` exists
2. **First run** (no file):
   - Create `~/.claude-manager/` directory and empty `projects.toml`
   - Prompt: "Scan for local repos? (y/n)"
   - If yes: run scanner → show selector → save selected projects
   - If no: start with empty list
3. Load projects from TOML
4. Check `gh auth status` silently — if unavailable, disable GitHub features (no error)
5. Launch TUI

---

## Modules Removed

| Module | Reason |
|--------|--------|
| `db.rs` | SQLite replaced by TOML file |
| `sync.rs` | No cross-machine sync |
| `machine.rs` | No machine IDs needed |
| `config.rs` | Merged into projects.toml |

**Dependency removed:** `rusqlite` (SQLite)

**Dependencies also removable:** `hostname`, `uuid` (only used for machine ID generation)

---

## Modules Added

### `store.rs` — Project Store

Replaces `db.rs`, `config.rs`, and `sync.rs` with a single module:

```rust
struct ProjectEntry {
    name: String,
    repo_url: Option<String>,
    path: String,
    run_command: Option<String>,
}

struct ProjectStore {
    install_dir: Option<String>,
    projects: Vec<ProjectEntry>,
    path: PathBuf,  // ~/.claude-manager/projects.toml
}

impl ProjectStore {
    fn load() -> Result<Self>;           // Read TOML or create default
    fn save(&self) -> Result<()>;        // Write TOML
    fn add(&mut self, project: ProjectEntry);
    fn remove(&mut self, name: &str);    // Remove from list only, not from disk
    fn list(&self) -> &[ProjectEntry];
}
```

---

## Modules Unchanged

| Module | Purpose |
|--------|---------|
| `detect.rs` | Project type detection (JS, Rust, Go, Python) |
| `git_status.rs` | Git branch/status display |
| `ports.rs` | Port scanning for dev servers |
| `process.rs` | Process manager for dev servers |
| `scanner.rs` | Directory scanning for repos |
| `tui.rs` | Terminal UI event loop |
| `ui/input.rs` | Text input dialog |
| `ui/selector.rs` | List selector dialog |

---

## Modules Modified

### `gh.rs`

- `check_auth()` becomes non-fatal — returns `bool` instead of `Result`
- Other functions unchanged

### `app.rs`

- Replace `db: Database` → `store: ProjectStore`
- Remove `machine_id: String`
- Remove all `sync::push()` calls → `store.save()`
- Add `[s]` keybind for rescan
- Remove `[p]ath` command (path set at import/scan time)
- `[a]dd` gracefully disabled if `gh` unavailable
- `[d]el` removes from TOML only (never touches disk)

### `main.rs`

- Remove `--init` flag and init flow
- Single startup path: load store → check gh (optional) → launch TUI
- First-run detection happens inline

---

## Key Bindings (Updated)

| Key | Action | Change |
|-----|--------|--------|
| `a` | Add from GitHub | Disabled if `gh` unavailable |
| `s` | Scan for repos | **NEW** — triggers scanner, shows selector |
| `r` | Run project | No change |
| `e` | Edit run command | Saves to TOML instead of DB |
| `i` | Import from path | Saves to TOML instead of DB |
| `d` | Delete project | Removes from TOML only |
| `c` | Set install dir | Saves to TOML instead of separate config |
| `F5` | Full refresh | Reloads TOML + refreshes git status (no sync pull) |
| `q` | Quit | No change |

Removed: `[p]ath` (path set at import time)

---

## TUI Layout Changes

- Remove "Other Machines" section from details panel (no cross-machine data)
- Remove machine ID from status bar
- Show "(gh unavailable)" indicator if GitHub features disabled
