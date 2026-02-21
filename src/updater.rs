use std::sync::mpsc::{self, Receiver};
use std::thread;

#[derive(Clone, Debug)]
pub struct UpdateInfo {
    pub version: String,
    pub download_url: String,
}

pub struct UpdateChecker {
    result_rx: Receiver<Option<UpdateInfo>>,
}

impl UpdateChecker {
    pub fn check_in_background(owner: &str, repo: &str) -> Self {
        let (tx, rx) = mpsc::channel();
        let url = format!(
            "https://api.github.com/repos/{}/{}/releases/latest",
            owner, repo
        );
        let current_version = env!("CARGO_PKG_VERSION").to_string();

        thread::spawn(move || {
            let result = check_for_update(&url, &current_version);
            let _ = tx.send(result);
        });

        Self { result_rx: rx }
    }

    pub fn poll(&self) -> Option<Option<UpdateInfo>> {
        self.result_rx.try_recv().ok()
    }
}

fn check_for_update(api_url: &str, current_version: &str) -> Option<UpdateInfo> {
    use std::process::Command;

    #[cfg(windows)]
    let output = Command::new("curl.exe")
        .args(["-s", "-L", "--max-time", "10", api_url])
        .output()
        .ok()?;

    #[cfg(not(windows))]
    let output = Command::new("curl")
        .args(["-s", "-L", "--max-time", "10", api_url])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let body = String::from_utf8_lossy(&output.stdout);

    let tag = extract_json_string(&body, "tag_name")?;
    let version = tag.trim_start_matches('v');

    if version_is_newer(version, current_version) {
        let asset_name = platform_asset_name();
        let download_url = find_asset_url(&body, &asset_name)?;

        Some(UpdateInfo {
            version: version.to_string(),
            download_url,
        })
    } else {
        None
    }
}

fn extract_json_string(json: &str, key: &str) -> Option<String> {
    let pattern = format!("\"{}\"", key);
    let start = json.find(&pattern)? + pattern.len();
    let rest = &json[start..];
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(':')?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn find_asset_url(json: &str, asset_name: &str) -> Option<String> {
    let mut search_from = 0;
    while let Some(pos) = json[search_from..].find("browser_download_url") {
        let abs_pos = search_from + pos;
        let rest = &json[abs_pos..];
        if let Some(url) = extract_json_string(rest, "browser_download_url") {
            if url.contains(asset_name) {
                return Some(url);
            }
        }
        search_from = abs_pos + 1;
    }
    None
}

fn platform_asset_name() -> String {
    #[cfg(target_os = "windows")]
    return "x86_64-pc-windows-msvc".to_string();

    #[cfg(target_os = "linux")]
    return "x86_64-unknown-linux-gnu".to_string();

    #[cfg(target_os = "macos")]
    return "aarch64-apple-darwin".to_string();

    #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
    return "unknown".to_string();
}

fn version_is_newer(remote: &str, local: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> {
        v.split('.').filter_map(|s| s.parse().ok()).collect()
    };
    let r = parse(remote);
    let l = parse(local);
    r > l
}

pub fn apply_update(info: &UpdateInfo) -> anyhow::Result<()> {
    use std::process::Command;

    let exe_path = std::env::current_exe()?;
    let exe_dir = exe_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("No parent dir"))?;

    #[cfg(windows)]
    let archive_name = "claude-manager-update.zip";
    #[cfg(not(windows))]
    let archive_name = "claude-manager-update.tar.gz";

    let archive_path = exe_dir.join(archive_name);

    // Download
    #[cfg(windows)]
    {
        let status = Command::new("curl.exe")
            .args([
                "-s",
                "-L",
                "--max-time",
                "120",
                "-o",
                &archive_path.to_string_lossy().to_string(),
                &info.download_url,
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("Download failed");
        }

        let temp_dir = exe_dir.join("_update_temp");
        let _ = std::fs::create_dir_all(&temp_dir);
        let status = Command::new("tar")
            .args([
                "-xf",
                &archive_path.to_string_lossy().to_string(),
                "-C",
                &temp_dir.to_string_lossy().to_string(),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("Extract failed");
        }

        let old_path = exe_path.with_extension("exe.old");
        let _ = std::fs::remove_file(&old_path);
        std::fs::rename(&exe_path, &old_path)?;
        std::fs::rename(temp_dir.join("claude-manager.exe"), &exe_path)?;

        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_file(&archive_path);
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("curl")
            .args([
                "-s",
                "-L",
                "--max-time",
                "120",
                "-o",
                &archive_path.to_string_lossy().to_string(),
                &info.download_url,
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("Download failed");
        }

        let temp_dir = exe_dir.join("_update_temp");
        let _ = std::fs::create_dir_all(&temp_dir);
        let status = Command::new("tar")
            .args([
                "-xzf",
                &archive_path.to_string_lossy().to_string(),
                "-C",
                &temp_dir.to_string_lossy().to_string(),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("Extract failed");
        }

        std::fs::rename(temp_dir.join("claude-manager"), &exe_path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&exe_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&exe_path, perms)?;
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
        let _ = std::fs::remove_file(&archive_path);
    }

    Ok(())
}

pub fn cleanup_old_exe() {
    #[cfg(windows)]
    {
        if let Ok(exe_path) = std::env::current_exe() {
            let old_path = exe_path.with_extension("exe.old");
            let _ = std::fs::remove_file(old_path);
        }
    }
}
