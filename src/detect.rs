use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct DetectedProject {
    pub package_manager: Option<PackageManager>,
    pub run_command: Option<String>,
    pub project_type: ProjectType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PackageManager {
    Pnpm,
    Yarn,
    Bun,
    Npm,
    Cargo,
    Go,
    Python,
}

impl PackageManager {
    pub fn as_str(&self) -> &'static str {
        match self {
            PackageManager::Pnpm => "pnpm",
            PackageManager::Yarn => "yarn",
            PackageManager::Bun => "bun",
            PackageManager::Npm => "npm",
            PackageManager::Cargo => "cargo",
            PackageManager::Go => "go",
            PackageManager::Python => "python",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ProjectType {
    JavaScript,
    Rust,
    Go,
    Python,
    Unknown,
}

#[derive(Deserialize)]
struct PackageJson {
    scripts: Option<HashMap<String, String>>,
}

pub fn detect(path: &Path) -> Result<DetectedProject> {
    // Check for JS project
    if path.join("package.json").exists() {
        return detect_js(path);
    }

    // Check for Rust project
    if path.join("Cargo.toml").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Cargo),
            run_command: Some("cargo run".to_string()),
            project_type: ProjectType::Rust,
        });
    }

    // Check for Go project
    if path.join("go.mod").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Go),
            run_command: Some("go run .".to_string()),
            project_type: ProjectType::Go,
        });
    }

    // Check for Python project
    if path.join("manage.py").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Python),
            run_command: Some("python manage.py runserver".to_string()),
            project_type: ProjectType::Python,
        });
    }
    if path.join("main.py").exists() {
        return Ok(DetectedProject {
            package_manager: Some(PackageManager::Python),
            run_command: Some("python main.py".to_string()),
            project_type: ProjectType::Python,
        });
    }

    Ok(DetectedProject {
        package_manager: None,
        run_command: None,
        project_type: ProjectType::Unknown,
    })
}

fn detect_js(path: &Path) -> Result<DetectedProject> {
    // Detect package manager from lockfile
    let pm = if path.join("pnpm-lock.yaml").exists() {
        PackageManager::Pnpm
    } else if path.join("yarn.lock").exists() {
        PackageManager::Yarn
    } else if path.join("bun.lockb").exists() {
        PackageManager::Bun
    } else {
        PackageManager::Npm
    };

    // Read package.json to find best run script
    let pkg_path = path.join("package.json");
    let content = fs::read_to_string(&pkg_path)?;
    let pkg: PackageJson = serde_json::from_str(&content)?;

    let run_cmd = if let Some(scripts) = pkg.scripts {
        // Preference order: dev > start > serve > watch
        let script = ["dev", "start", "serve", "watch"]
            .iter()
            .find(|s| scripts.contains_key(**s))
            .copied();

        script.map(|s| format!("{} run {}", pm.as_str(), s))
    } else {
        None
    };

    Ok(DetectedProject {
        package_manager: Some(pm),
        run_command: run_cmd,
        project_type: ProjectType::JavaScript,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_js_with_pnpm() {
        let temp = std::env::temp_dir().join("claude-manager-detect-test");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        fs::write(temp.join("pnpm-lock.yaml"), "").unwrap();
        fs::write(
            temp.join("package.json"),
            r#"{"scripts": {"dev": "vite"}}"#,
        )
        .unwrap();

        let detected = detect(&temp).unwrap();
        assert_eq!(detected.package_manager, Some(PackageManager::Pnpm));
        assert_eq!(detected.run_command, Some("pnpm run dev".to_string()));
        assert_eq!(detected.project_type, ProjectType::JavaScript);

        let _ = fs::remove_dir_all(&temp);
    }

    #[test]
    fn test_detect_rust() {
        let temp = std::env::temp_dir().join("claude-manager-detect-rust");
        let _ = fs::remove_dir_all(&temp);
        fs::create_dir_all(&temp).unwrap();

        fs::write(temp.join("Cargo.toml"), "[package]\nname = \"test\"").unwrap();

        let detected = detect(&temp).unwrap();
        assert_eq!(detected.package_manager, Some(PackageManager::Cargo));
        assert_eq!(detected.run_command, Some("cargo run".to_string()));
        assert_eq!(detected.project_type, ProjectType::Rust);

        let _ = fs::remove_dir_all(&temp);
    }
}
