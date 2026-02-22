# Claude Manager

A TUI (terminal user interface) for managing development projects. Run dev servers, spawn Claude terminals, monitor git status, and track ports — all from one dashboard.

## Features

- **Process Management** — Start and stop dev servers with automatic dependency installation and port assignment
- **Claude Integration** — Spawn terminal sessions with the `claude` CLI in any project directory
- **Git Status** — Live branch, staged/modified/untracked counts, and ahead/behind tracking
- **Port Monitoring** — Scan 60 common dev ports (3000-3010, 4000-4010, 5000-5010, 8000-8010, etc.) with process names
- **Project Detection** — Auto-detect project types (JavaScript, Rust, Go, Python) and package managers (npm, pnpm, yarn, bun)
- **GitHub Import** — Add projects from your GitHub repos, clone them, or scan local directories
- **Auto-Update** — Background update checks with one-key install
- **Cross-Platform** — Windows, macOS (Apple Silicon), and Linux

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `j` / `k` | Navigate project list |
| `r` | Run project (fetch, install deps, start dev server + Claude terminal) |
| `x` | Stop project |
| `e` | Edit run command |
| `a` | Add project from GitHub |
| `i` | Import project from local path |
| `g` | Git clone a project |
| `s` | Scan directories for git repos |
| `c` | Configure clone/install directory |
| `d` | Delete project |
| `u` | Install available update |
| `F5` | Full refresh |
| `q` | Quit |

## Installation

### Download a Release

Pre-built binaries are available for Windows, macOS (Apple Silicon), and Linux.

1. Go to the [Releases](https://github.com/stephenfjohnson/claude-manager/releases) page
2. Download the archive for your platform:
   - **Windows:** `claude-manager-x86_64-pc-windows-msvc.zip`
   - **macOS:** `claude-manager-aarch64-apple-darwin.tar.gz`
   - **Linux:** `claude-manager-x86_64-unknown-linux-gnu.tar.gz`
3. Extract the binary and place it somewhere on your `PATH`

**Windows:**
```powershell
# Extract the zip, then move the binary to a directory on your PATH
Expand-Archive claude-manager-x86_64-pc-windows-msvc.zip -DestinationPath .
```

**macOS / Linux:**
```bash
tar xzf claude-manager-*.tar.gz
chmod +x claude-manager
sudo mv claude-manager /usr/local/bin/
```

After the first install, Claude Manager will notify you of new versions and can update itself with the `u` key.

### Build from Source

Requires the [Rust toolchain](https://rustup.rs/).

```bash
cargo build --release
```

The binary will be at `target/release/claude-manager` (or `claude-manager.exe` on Windows).

## Prerequisites

The [GitHub CLI](https://cli.github.com/) (`gh`) is required for adding projects from GitHub:

```bash
gh auth login
```

## Usage

```bash
claude-manager
```

On first launch, you can scan for existing git repositories or add projects from GitHub.

## Configuration

Settings are stored in `~/.claude-manager/projects.toml`. You can configure:

- **Install directory** — where new projects are cloned (set with `c`)
- **Run commands** — per-project override for the dev server command (set with `e`)
- **Projects** — added via GitHub import, local path, directory scan, or git clone
