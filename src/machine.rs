use anyhow::Result;
use std::fs;
use std::path::PathBuf;

fn config_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Cannot find home directory"))?;
    Ok(home.join(".claude-manager"))
}

pub fn machine_id_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("machine-id"))
}

pub fn get_or_create_machine_id() -> Result<String> {
    let path = machine_id_path()?;

    if path.exists() {
        return Ok(fs::read_to_string(&path)?.trim().to_string());
    }

    // Generate new ID: hostname-random8
    let host = hostname::get()?.to_string_lossy().to_string();
    let suffix: String = uuid::Uuid::new_v4().to_string()[..8].to_string();
    let id = format!("{}-{}", host, suffix);

    // Ensure parent dir exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, &id)?;
    Ok(id)
}

pub fn get_machine_id() -> Result<Option<String>> {
    let path = machine_id_path()?;
    if path.exists() {
        Ok(Some(fs::read_to_string(&path)?.trim().to_string()))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_id_format() {
        // Test the format logic
        let host = "testhost";
        let suffix = &uuid::Uuid::new_v4().to_string()[..8];
        let id = format!("{}-{}", host, suffix);

        assert!(id.starts_with("testhost-"));
        assert_eq!(id.len(), "testhost-".len() + 8);
    }
}
