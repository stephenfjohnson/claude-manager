use anyhow::Result;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

pub struct ProcessManager {
    processes: HashMap<String, Child>,
    output_buffers: Arc<Mutex<HashMap<String, Vec<String>>>>,
    ports: HashMap<String, u16>,
}

impl ProcessManager {
    pub fn new() -> Self {
        Self {
            processes: HashMap::new(),
            output_buffers: Arc::new(Mutex::new(HashMap::new())),
            ports: HashMap::new(),
        }
    }

    #[allow(dead_code)]
    pub fn start(&mut self, project_name: &str, cwd: &Path, command: &str) -> Result<()> {
        self.start_with_port(project_name, cwd, command, None)
    }

    pub fn start_with_port(
        &mut self,
        project_name: &str,
        cwd: &Path,
        command: &str,
        port: Option<u16>,
    ) -> Result<()> {
        // Parse command into program and args
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let mut cmd;

        // On Windows, commands like npm/pnpm/yarn/bun are .cmd files, not .exe.
        // We must run them through cmd.exe to resolve them properly.
        // Also use CREATE_NO_WINDOW and null stdin to avoid interfering with the TUI console.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            cmd = Command::new("cmd.exe");
            cmd.args(["/c", command])
                .current_dir(cwd)
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW);
        }

        #[cfg(not(windows))]
        {
            let program = parts[0];
            let args = &parts[1..];
            cmd = Command::new(program);
            cmd.args(args)
                .current_dir(cwd)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
        }

        // Set PORT env var if provided (for Node.js projects)
        if let Some(p) = port {
            cmd.env("PORT", p.to_string());
        }

        let mut child = cmd.spawn()?;

        // Setup output capture
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let key = project_name.to_string();

        {
            let mut buffers = self.output_buffers.lock().unwrap();
            buffers.insert(key.clone(), Vec::new());
        }

        // Spawn threads to capture output
        if let Some(stdout) = stdout {
            let buffers = Arc::clone(&self.output_buffers);
            let key = key.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut buffers) = buffers.lock() {
                        if let Some(buf) = buffers.get_mut(&key) {
                            buf.push(line);
                            // Keep last 1000 lines
                            if buf.len() > 1000 {
                                buf.remove(0);
                            }
                        }
                    }
                }
            });
        }

        if let Some(stderr) = stderr {
            let buffers = Arc::clone(&self.output_buffers);
            let key = key.clone();
            thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Ok(mut buffers) = buffers.lock() {
                        if let Some(buf) = buffers.get_mut(&key) {
                            buf.push(format!("[stderr] {}", line));
                            if buf.len() > 1000 {
                                buf.remove(0);
                            }
                        }
                    }
                }
            });
        }

        self.processes.insert(key.clone(), child);
        if let Some(p) = port {
            self.ports.insert(key, p);
        }
        Ok(())
    }

    pub fn stop(&mut self, project_name: &str) -> Result<()> {
        if let Some(mut child) = self.processes.remove(project_name) {
            // Try graceful shutdown first
            #[cfg(unix)]
            {
                unsafe {
                    libc::kill(child.id() as i32, libc::SIGTERM);
                }
            }
            #[cfg(windows)]
            {
                let _ = child.kill();
            }

            // Wait a bit then force kill if needed
            std::thread::sleep(std::time::Duration::from_millis(500));
            let _ = child.kill();
            let _ = child.wait();
        }

        // Clean up buffer
        if let Ok(mut buffers) = self.output_buffers.lock() {
            buffers.remove(project_name);
        }

        self.ports.remove(project_name);

        Ok(())
    }

    pub fn is_running(&self, project_name: &str) -> bool {
        self.processes.contains_key(project_name)
    }

    pub fn reap_dead(&mut self) {
        let dead: Vec<String> = self
            .processes
            .iter_mut()
            .filter_map(|(name, child)| match child.try_wait() {
                Ok(Some(_)) => Some(name.clone()),
                _ => None,
            })
            .collect();
        for name in dead {
            self.processes.remove(&name);
            self.ports.remove(&name);
        }
    }

    pub fn get_output(&self, project_name: &str) -> Vec<String> {
        if let Ok(buffers) = self.output_buffers.lock() {
            buffers.get(project_name).cloned().unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    pub fn running_projects(&self) -> Vec<String> {
        self.processes.keys().cloned().collect()
    }

    pub fn get_port(&self, project_name: &str) -> Option<u16> {
        self.ports.get(project_name).copied()
    }
}

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_start_and_capture_output() {
        let mut pm = ProcessManager::new();
        let cwd = env::temp_dir();

        // Use a command that produces output and exits
        #[cfg(windows)]
        let command = "echo hello from dev server";
        #[cfg(not(windows))]
        let command = "echo hello from dev server";

        pm.start("test-project", &cwd, command).expect("Failed to start process");

        // Give it a moment to produce output
        std::thread::sleep(std::time::Duration::from_millis(500));

        let output = pm.get_output("test-project");
        assert!(
            !output.is_empty(),
            "Expected output from process, got nothing"
        );
        assert!(
            output.iter().any(|l| l.contains("hello from dev server")),
            "Expected 'hello from dev server' in output, got: {:?}",
            output
        );
    }

    #[cfg(windows)]
    #[test]
    fn test_cmd_resolves_cmd_files() {
        let mut pm = ProcessManager::new();
        let cwd = env::temp_dir();

        // npm --version should work through cmd.exe /c
        // This verifies .cmd file resolution works
        pm.start("npm-test", &cwd, "npm --version")
            .expect("Failed to start npm via cmd.exe");

        std::thread::sleep(std::time::Duration::from_millis(2000));

        let output = pm.get_output("npm-test");
        assert!(
            !output.is_empty(),
            "npm --version produced no output - cmd.exe /c is not resolving .cmd files"
        );
        // npm version output is like "10.2.0"
        assert!(
            output.iter().any(|l| l.chars().any(|c| c.is_ascii_digit())),
            "Expected version number from npm, got: {:?}",
            output
        );
    }

    #[test]
    fn test_start_long_running_and_stop() {
        let mut pm = ProcessManager::new();
        let cwd = env::temp_dir();

        // Start a long-running process
        #[cfg(windows)]
        let command = "ping -n 60 127.0.0.1";
        #[cfg(not(windows))]
        let command = "sleep 60";

        pm.start("long-running", &cwd, command).expect("Failed to start process");

        std::thread::sleep(std::time::Duration::from_millis(500));

        assert!(pm.is_running("long-running"), "Process should be running");

        pm.stop("long-running").expect("Failed to stop process");

        assert!(!pm.is_running("long-running"), "Process should be stopped");
    }
}
