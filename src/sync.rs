use anyhow::{bail, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

        // Create initial commit with a readme (git needs at least one commit)
        let readme_path = sync_path.join("README.md");
        fs::write(&readme_path, "# Claude Manager Sync\n\nThis repo stores project data for claude-manager.\n")?;

        git_command(&sync_path, &["add", "README.md"])?;
        git_command(&sync_path, &["commit", "-m", "Initial commit"])?;
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
