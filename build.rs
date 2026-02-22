fn main() {
    // Set GIT_VERSION from the latest git tag, falling back to Cargo.toml version
    if let Ok(output) = std::process::Command::new("git")
        .args(["describe", "--tags", "--abbrev=0"])
        .output()
    {
        if output.status.success() {
            let tag = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let version = tag.trim_start_matches('v');
            println!("cargo:rustc-env=GIT_VERSION={}", version);
        }
    }
    // Rebuild if tags change
    println!("cargo:rerun-if-changed=.git/refs/tags");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to set icon: {}", e);
        }
    }
}
