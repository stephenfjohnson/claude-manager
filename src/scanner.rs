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
        home.join("Documents"),
        home.join("Documents").join("Projects"),
        home.join("Documents").join("projects"),
        home.join("Documents").join("dev"),
        home.join("Documents").join("Dev"),
        home.join("Documents").join("code"),
        home.join("Documents").join("Code"),
    ];

    let mut results = Vec::new();

    for dir in search_dirs {
        if dir.exists() {
            scan_directory_recursive(&dir, 3, &mut results);
        }
    }

    // Deduplicate by path
    results.sort_by(|a, b| a.path.cmp(&b.path));
    results.dedup_by(|a, b| a.path == b.path);

    results
}

fn scan_directory_recursive(dir: &Path, depth: usize, results: &mut Vec<ScannedProject>) {
    if depth == 0 {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        // Skip hidden directories
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        // Skip common non-project directories
        let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if matches!(
            dir_name,
            "node_modules" | "target" | "build" | "dist" | "vendor" | "__pycache__"
        ) {
            continue;
        }

        // Check if it's a git repo
        if path.join(".git").exists() {
            let name = dir_name.to_string();
            let remote_url = get_git_remote(&path);

            results.push(ScannedProject {
                path: path.clone(),
                name,
                remote_url,
            });
            // Don't recurse into git repos (submodules would be separate)
        } else {
            // Not a git repo, recurse deeper
            scan_directory_recursive(&path, depth - 1, results);
        }
    }
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
