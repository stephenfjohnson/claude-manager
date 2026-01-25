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
        .args(["repo", "create", repo_name, "--private"])
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

/// List user's repositories
/// Returns Vec of (name, clone_url)
pub fn list_repos() -> Result<Vec<(String, String)>> {
    let output = Command::new("gh")
        .args(["repo", "list", "--json", "name,url", "--limit", "100"])
        .output()?;

    if !output.status.success() {
        bail!("Failed to list repos");
    }

    let json_str = String::from_utf8_lossy(&output.stdout);

    // Parse JSON manually to avoid adding serde dependency to gh module
    // Format: [{"name":"repo","url":"https://..."},...]
    let mut repos = Vec::new();

    // Simple JSON parsing for this specific format
    for item in json_str.split("},") {
        let name = extract_json_string(item, "name");
        let url = extract_json_string(item, "url");

        if let (Some(name), Some(url)) = (name, url) {
            // Convert to clone URL format
            let clone_url = format!("{}.git", url);
            repos.push((name, clone_url));
        }
    }

    Ok(repos)
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
