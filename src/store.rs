use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo_url: Option<String>,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectStore {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub install_dir: Option<String>,
    #[serde(default)]
    pub projects: Vec<ProjectEntry>,

    #[serde(skip)]
    file_path: PathBuf,

    #[serde(skip)]
    first_run: bool,
}

impl ProjectStore {
    /// Returns the path to `~/.claude-manager/projects.toml`.
    fn store_path() -> Result<PathBuf> {
        let home =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
        Ok(home.join(".claude-manager").join("projects.toml"))
    }

    /// Load the project store from disk, or create a default if the file doesn't exist.
    /// Projects are sorted by name (case-insensitive).
    pub fn load() -> Result<Self> {
        let path = Self::store_path()?;
        let mut store = if path.exists() {
            let content = fs::read_to_string(&path)?;
            let mut store: ProjectStore = toml::from_str(&content)?;
            store.first_run = false;
            store
        } else {
            let mut store = ProjectStore::default();
            store.first_run = true;
            store
        };
        store.file_path = path;
        store.sort_projects();
        Ok(store)
    }

    /// Write the store to its TOML file, creating parent directories if needed.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&self.file_path, content)?;
        Ok(())
    }

    /// Returns true if the file didn't exist when loaded (first run).
    pub fn is_first_run(&self) -> bool {
        self.first_run
    }

    /// Add a project entry. Skips duplicates by name. Re-sorts after adding.
    pub fn add(&mut self, entry: ProjectEntry) {
        if self
            .projects
            .iter()
            .any(|p| p.name.eq_ignore_ascii_case(&entry.name))
        {
            return;
        }
        self.projects.push(entry);
        self.sort_projects();
    }

    /// Remove a project by name.
    pub fn remove(&mut self, name: &str) {
        self.projects
            .retain(|p| !p.name.eq_ignore_ascii_case(name));
    }

    /// Find a project by name.
    pub fn get(&self, name: &str) -> Option<&ProjectEntry> {
        self.projects
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Find a project by name, returning a mutable reference.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ProjectEntry> {
        self.projects
            .iter_mut()
            .find(|p| p.name.eq_ignore_ascii_case(name))
    }

    /// Get the install directory as an expanded, absolute path.
    /// Returns None if the stored path is not absolute (invalid).
    pub fn get_install_dir(&self) -> Option<PathBuf> {
        self.install_dir.as_ref().and_then(|dir| {
            let path = if dir.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    home.join(&dir[2..])
                } else {
                    return None;
                }
            } else {
                PathBuf::from(dir)
            };
            // Only return absolute paths to prevent cloning relative to cwd
            if path.is_absolute() {
                Some(path)
            } else {
                None
            }
        })
    }

    /// Sort projects by name, case-insensitive.
    fn sort_projects(&mut self) {
        self.projects
            .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_remove() {
        let mut store = ProjectStore::default();

        // Add a project
        store.add(ProjectEntry {
            name: "my-project".to_string(),
            repo_url: Some("https://github.com/user/my-project".to_string()),
            path: "/home/user/my-project".to_string(),
            run_command: None,
        });
        assert_eq!(store.projects.len(), 1);

        // Add duplicate name — should be skipped
        store.add(ProjectEntry {
            name: "my-project".to_string(),
            repo_url: None,
            path: "/other/path".to_string(),
            run_command: None,
        });
        assert_eq!(store.projects.len(), 1);

        // Remove
        store.remove("my-project");
        assert!(store.projects.is_empty());
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut store = ProjectStore::default();
        store.install_dir = Some("~/Projects".to_string());
        store.add(ProjectEntry {
            name: "alpha".to_string(),
            repo_url: Some("https://github.com/user/alpha".to_string()),
            path: "/home/user/alpha".to_string(),
            run_command: Some("npm start".to_string()),
        });

        // Serialize to TOML string
        let toml_str = toml::to_string_pretty(&store).expect("serialize");

        // Deserialize back
        let restored: ProjectStore = toml::from_str(&toml_str).expect("deserialize");

        assert_eq!(restored.install_dir, Some("~/Projects".to_string()));
        assert_eq!(restored.projects.len(), 1);
        assert_eq!(restored.projects[0].name, "alpha");
        assert_eq!(
            restored.projects[0].repo_url,
            Some("https://github.com/user/alpha".to_string())
        );
        assert_eq!(restored.projects[0].path, "/home/user/alpha");
        assert_eq!(
            restored.projects[0].run_command,
            Some("npm start".to_string())
        );
    }
}
