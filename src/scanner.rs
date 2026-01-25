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
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with('.'))
            .unwrap_or(false)
        {
            continue;
        }

        // Check if it's a git repo
        if path.join(".git").exists() {
            let name = path
                .file_name()
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
