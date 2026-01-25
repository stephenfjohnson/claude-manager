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
