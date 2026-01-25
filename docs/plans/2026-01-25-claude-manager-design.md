# Claude Manager - Design Document

A terminal-based project dashboard for managing development projects across multiple machines.

## Summary

Claude Manager tracks your projects and where they live on each machine. It syncs this metadata via a private GitHub repo, shows git status, and lets you run/stop dev servers from a single dashboard.

## Key Decisions

- **Requires internet** - App won't start without syncing from GitHub
- **Uses `gh` CLI** - All GitHub auth delegated to `gh`
- **Simple sync** - Pull on startup, push on change
- **Small scale** - Under 20 projects, 2-3 machines (no search/pagination)

---

## Data Model

**Database: `projects.db` (SQLite)**

```sql
CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT UNIQUE NOT NULL,
    repo_url TEXT NOT NULL
);

CREATE TABLE machine_locations (
    id INTEGER PRIMARY KEY,
    project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
    machine_id TEXT NOT NULL,
    path TEXT NOT NULL,
    run_command TEXT,  -- override when auto-detect is wrong
    UNIQUE(project_id, machine_id)
);
```

**Machine ID:** `{hostname}-{random-8-chars}` stored in `~/.claude-manager/machine-id`

---

## File Structure

```
~/.claude-manager/
├── machine-id              # "hostname-a1b2c3d4" (generated once)
├── sync/                   # Git repo cloned from GitHub
│   ├── .git/
│   └── projects.db         # SQLite database (synced)
└── logs/                   # Optional: historical logs (not synced)
```

---

## Setup Flow (`--init`)

1. Check `gh auth status` - exit if not logged in
2. Generate machine ID if not exists
3. Check if `claude-manager-sync` repo exists on GitHub:
   - If no: create private repo, clone, create empty DB, commit, push
   - If yes: clone existing repo
4. Offer to scan common directories (`~/projects`, `~/dev`, `~/code`, `~/src`, `~/Documents/Projects`) for git repos
5. Show checklist of found repos, import selected ones

---

## Startup Flow (normal run)

1. Check `gh auth status` - exit if not logged in
2. Check `~/.claude-manager/sync/` exists - exit if not
3. `git pull origin main` - exit on failure (no offline mode)
4. Read machine ID
5. Load projects.db
6. Start TUI

---

## Sync Behavior

- **On startup:** `git pull`
- **On data change:** `git add projects.db && git commit && git push`
- **On push conflict:** `git pull --rebase` then retry push

---

## TUI Layout

```
┌─ Claude Manager ──────────────────────────────────────────────────┐
│ Projects                          │ Details                       │
│ ──────────────────────────────────│───────────────────────────────│
│ ● my-saas-app      [pnpm]  ✓      │ Name: my-saas-app             │
│   portfolio        [npm]   ✓      │ Path: /Users/you/dev/saas     │
│   cli-tool         [bun]   ✗      │ Repo: github.com/you/saas     │
│                                   │                               │
│                                   │ Branch: main                  │
│ Legend:                           │ Staged: 0  Modified: 3        │
│ ● = running                       │ Ahead: 2   Behind: 0          │
│ ✓ = path set on this machine      │                               │
│ ✗ = no path (needs setup)         │ ─── Other Machines ───────    │
│                                   │ desktop-x2y: C:\Projects\saas │
│                                   │ linux-z3w: /home/you/proj     │
├───────────────────────────────────┴───────────────────────────────┤
│ Logs (my-saas-app)                                       [ctrl+l] │
│ > Ready on http://localhost:3000                                  │
│ > GET / 200 in 12ms                                               │
├───────────────────────────────────────────────────────────────────┤
│ Ports: 3000 → my-saas-app │ 5173 (vite, PID 13102)               │
├───────────────────────────────────────────────────────────────────┤
│ [a]dd [p]ath [r]un [s]top [d]elete [k]ill port  │ Machine: mbp-a1│
└───────────────────────────────────────────────────────────────────┘
```

---

## Key Bindings

| Key | Action | Behavior |
|-----|--------|----------|
| `a` | Add project | Menu: "From current directory" / "From GitHub URL" |
| `p` | Set path | File picker or paste path |
| `r` | Run | Auto-detect command, spawn process |
| `s` | Stop | SIGTERM, force kill after 3s |
| `d` | Delete | Confirm, remove project and all locations |
| `k` | Kill port | Select port, kill process |
| `e` | Edit run cmd | Override auto-detected command |
| `↑/↓` | Navigate | Move through project list |
| `Enter` | Refresh | Re-fetch git status for selected |
| `F5` | Full refresh | Git pull, reload DB, rescan ports |
| `Ctrl+L` | Toggle logs | Show/hide logs panel |
| `q` | Quit | Stop all processes, exit |

---

## Run Command Detection

Order of precedence:
1. Override from `machine_locations.run_command` if set
2. JS (`package.json`): prefer `dev` > `start` > `serve` > `watch`
3. Rust (`Cargo.toml`): `cargo run`
4. Go (`go.mod`): `go run .`
5. Python: `manage.py` → `python manage.py runserver`, or `main.py` → `python main.py`
6. Nothing detected → prompt to set with `e`

---

## Port Scanning

- Scan on startup and every 30 seconds
- Ports: 3000-3010, 4000-4010, 5000-5010, 8000-8010, 8080, 9000
- Platform-specific:
  - Linux: `/proc/net/tcp` + `/proc/{pid}/fd`
  - macOS: `lsof -iTCP -sTCP:LISTEN -n -P`
  - Windows: `netstat -ano`
- Link ports to our projects via in-memory PID map

---

## Git Status

- Refresh when selecting a project, then every 60s while selected
- Local status: branch, staged count, modified count (via git2 or git CLI)
- Remote status: ahead/behind via `git fetch` then compare

---

## Process Management

**Run:**
1. Get path from machine_locations
2. Detect or use override command
3. Spawn with cwd = project path
4. Store PID in memory
5. Pipe stdout/stderr to logs panel

**Stop:**
1. SIGTERM (Unix) / TerminateProcess (Windows)
2. Wait 3 seconds
3. Force kill if needed
4. Remove from PID map

---

## Technology Stack

| Component | Choice | Reason |
|-----------|--------|--------|
| Language | Rust | Single binary, cross-platform |
| TUI | Ratatui | Mature, flexible layouts |
| Database | rusqlite | SQLite bindings |
| Git ops | gh + git CLI | Auth handled by gh |
| Async | Tokio | Process spawning, port scanning |

---

## Rust Project Structure

```
claude-manager/
├── Cargo.toml
├── src/
│   ├── main.rs             # Entry, arg parsing
│   ├── app.rs              # App state, event loop
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── layout.rs       # Panel arrangement
│   │   ├── projects.rs     # Project list widget
│   │   ├── details.rs      # Details panel widget
│   │   ├── logs.rs         # Logs panel widget
│   │   └── ports.rs        # Ports bar widget
│   ├── db.rs               # SQLite operations
│   ├── sync.rs             # Git/gh operations
│   ├── machine.rs          # Machine ID generation
│   ├── process.rs          # Spawn, stop, output capture
│   ├── ports.rs            # Port scanning
│   ├── detect.rs           # Run command detection
│   └── git_status.rs       # Local git status
```

---

## Dependencies

**Required on system:**
- `gh` CLI (authenticated via `gh auth login`)
- `git`

**Rust crates (likely):**
- `ratatui` - TUI framework
- `crossterm` - Terminal backend
- `rusqlite` - SQLite
- `tokio` - Async runtime
- `serde` / `serde_json` - For package.json parsing
- `hostname` - Get machine hostname
- `uuid` - Generate machine ID suffix
