use anyhow::Result;
use rusqlite::{params, Connection};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub repo_url: String,
}

#[derive(Debug, Clone)]
pub struct MachineLocation {
    pub id: i64,
    pub project_id: i64,
    pub machine_id: String,
    pub path: String,
    pub run_command: Option<String>,
}

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS projects (
                id INTEGER PRIMARY KEY,
                name TEXT UNIQUE NOT NULL,
                repo_url TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS machine_locations (
                id INTEGER PRIMARY KEY,
                project_id INTEGER REFERENCES projects(id) ON DELETE CASCADE,
                machine_id TEXT NOT NULL,
                path TEXT NOT NULL,
                run_command TEXT,
                UNIQUE(project_id, machine_id)
            );",
        )?;
        Ok(())
    }

    pub fn add_project(&self, name: &str, repo_url: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO projects (name, repo_url) VALUES (?1, ?2)",
            params![name, repo_url],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn delete_project(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM projects WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, name, repo_url FROM projects ORDER BY name")?;
        let projects = stmt.query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                repo_url: row.get(2)?,
            })
        })?;
        projects.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn set_location(&self, project_id: i64, machine_id: &str, path: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO machine_locations (project_id, machine_id, path)
             VALUES (?1, ?2, ?3)",
            params![project_id, machine_id, path],
        )?;
        Ok(())
    }

    pub fn set_run_command(
        &self,
        project_id: i64,
        machine_id: &str,
        cmd: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE machine_locations SET run_command = ?1
             WHERE project_id = ?2 AND machine_id = ?3",
            params![cmd, project_id, machine_id],
        )?;
        Ok(())
    }

    pub fn get_location(&self, project_id: i64, machine_id: &str) -> Result<Option<MachineLocation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, machine_id, path, run_command
             FROM machine_locations WHERE project_id = ?1 AND machine_id = ?2",
        )?;

        let mut rows = stmt.query(params![project_id, machine_id])?;

        if let Some(row) = rows.next()? {
            Ok(Some(MachineLocation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                machine_id: row.get(2)?,
                path: row.get(3)?,
                run_command: row.get(4)?,
            }))
        } else {
            Ok(None)
        }
    }

    pub fn get_all_locations(&self, project_id: i64) -> Result<Vec<MachineLocation>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, project_id, machine_id, path, run_command
             FROM machine_locations WHERE project_id = ?1",
        )?;

        let locs = stmt.query_map(params![project_id], |row| {
            Ok(MachineLocation {
                id: row.get(0)?,
                project_id: row.get(1)?,
                machine_id: row.get(2)?,
                path: row.get(3)?,
                run_command: row.get(4)?,
            })
        })?;

        locs.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_database_operations() {
        let temp_dir = std::env::temp_dir().join("claude-manager-db-test");
        fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test.db");

        // Clean up from previous runs
        let _ = fs::remove_file(&db_path);

        let db = Database::open(&db_path).unwrap();

        // Add a project
        let id = db.add_project("test-project", "github.com/user/test").unwrap();
        assert!(id > 0);

        // List projects
        let projects = db.list_projects().unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].name, "test-project");

        // Set location
        db.set_location(id, "machine-123", "/home/user/test").unwrap();

        // Get location
        let loc = db.get_location(id, "machine-123").unwrap().unwrap();
        assert_eq!(loc.path, "/home/user/test");
        assert!(loc.run_command.is_none());

        // Set run command
        db.set_run_command(id, "machine-123", Some("npm run dev")).unwrap();
        let loc = db.get_location(id, "machine-123").unwrap().unwrap();
        assert_eq!(loc.run_command, Some("npm run dev".to_string()));

        // Delete project (should cascade delete location)
        db.delete_project(id).unwrap();
        let projects = db.list_projects().unwrap();
        assert!(projects.is_empty());

        // Cleanup
        let _ = fs::remove_file(&db_path);
    }
}
