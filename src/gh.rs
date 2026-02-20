use anyhow::{bail, Result};
use std::process::Command;

/// Check if gh CLI is authenticated. Returns false if gh is not installed or not authed.
pub fn check_auth() -> bool {
    Command::new("gh")
        .args(["auth", "status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
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
