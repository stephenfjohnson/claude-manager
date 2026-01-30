# Claude Manager

A personal project dashboard for tracking and managing git repositories across multiple machines.

## Prerequisites

### Windows

1. **Rust toolchain** - Install via [rustup](https://rustup.rs/):
   ```powershell
   winget install Rustlang.Rustup
   ```
   Or download and run the installer from https://rustup.rs/

2. **GitHub CLI** - Required for syncing project data:
   ```powershell
   winget install GitHub.cli
   ```

3. **Authenticate with GitHub**:
   ```powershell
   gh auth login
   ```

### macOS

1. **Rust toolchain** - Install via [rustup](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```
   Or via Homebrew:
   ```bash
   brew install rust
   ```

2. **GitHub CLI**:
   ```bash
   brew install gh
   ```

3. **Authenticate with GitHub**:
   ```bash
   gh auth login
   ```

### Linux

1. **Rust toolchain** - Install via [rustup](https://rustup.rs/):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **GitHub CLI** - Install based on your distribution:

   **Debian/Ubuntu:**
   ```bash
   sudo apt install gh
   ```

   **Fedora:**
   ```bash
   sudo dnf install gh
   ```

   **Arch Linux:**
   ```bash
   sudo pacman -S github-cli
   ```

3. **Authenticate with GitHub**:
   ```bash
   gh auth login
   ```

## Building

```bash
cargo build --release
```

The binary will be at:
- **Windows:** `target\release\claude-manager.exe`
- **macOS/Linux:** `target/release/claude-manager`

## Running

### First-time setup

Initialize Claude Manager on your machine:

```bash
cargo run -- --init
```

Or if using the built binary:

**Windows:**
```powershell
.\target\release\claude-manager.exe --init
```

**macOS/Linux:**
```bash
./target/release/claude-manager --init
```

This will:
- Generate a unique machine ID
- Set up a sync repository on GitHub
- Optionally scan for existing git repositories to import

### Normal usage

After initialization, run without arguments to start the TUI:

```bash
cargo run
```

Or using the built binary:

**Windows:**
```powershell
.\target\release\claude-manager.exe
```

**macOS/Linux:**
```bash
./target/release/claude-manager
```

## Features

- Track projects across multiple machines
- Scan directories for existing git repositories
- Sync project data via GitHub
- TUI interface for managing projects
