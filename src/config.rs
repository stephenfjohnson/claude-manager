use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MachineConfig {
    /// Default directory for cloning new projects
    pub install_dir: Option<String>,
}

fn config_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".claude-manager").join("config.toml"))
}

impl MachineConfig {
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: MachineConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(MachineConfig::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;

        // Ensure parent dir exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        Ok(())
    }

    pub fn set_install_dir(&mut self, dir: Option<String>) -> Result<()> {
        self.install_dir = dir;
        self.save()
    }

    /// Get the install directory, expanding ~ to home
    pub fn get_install_dir(&self) -> Option<PathBuf> {
        self.install_dir.as_ref().map(|dir| {
            if dir.starts_with("~/") {
                if let Some(home) = dirs::home_dir() {
                    return home.join(&dir[2..]);
                }
            }
            PathBuf::from(dir)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MachineConfig::default();
        assert!(config.install_dir.is_none());
    }

    #[test]
    fn test_expand_tilde() {
        let mut config = MachineConfig::default();
        config.install_dir = Some("~/Projects".to_string());

        let expanded = config.get_install_dir().unwrap();
        assert!(!expanded.to_string_lossy().contains('~'));
    }
}
