use anyhow::Result;
use std::net::TcpStream;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PortInfo {
    pub port: u16,
    pub pid: Option<u32>,
    pub process_name: Option<String>,
}

pub fn scan_ports() -> Vec<PortInfo> {
    let ports_to_check: Vec<u16> = (3000..=3010)
        .chain(4000..=4010)
        .chain(5000..=5010)
        .chain(8000..=8010)
        .chain(std::iter::once(8080))
        .chain(std::iter::once(9000))
        .collect();

    let mut results = Vec::new();

    for port in ports_to_check {
        if is_port_open(port) {
            let (pid, name) = get_process_for_port(port).unwrap_or((None, None));
            results.push(PortInfo {
                port,
                pid,
                process_name: name,
            });
        }
    }

    results
}

fn is_port_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &format!("127.0.0.1:{}", port).parse().unwrap(),
        Duration::from_millis(50),
    )
    .is_ok()
}

#[cfg(target_os = "linux")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::fs;
    use std::path::Path;

    // Read /proc/net/tcp to find the inode for this port
    let tcp_content = fs::read_to_string("/proc/net/tcp")?;
    let port_hex = format!("{:04X}", port);

    let mut target_inode: Option<u64> = None;

    for line in tcp_content.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 10 {
            continue;
        }

        // local_address is like "00000000:0CEA" (ip:port in hex)
        let local_addr = parts[1];
        if let Some(local_port) = local_addr.split(':').nth(1) {
            if local_port == port_hex {
                // Found it, get inode (column 9)
                if let Ok(inode) = parts[9].parse::<u64>() {
                    target_inode = Some(inode);
                    break;
                }
            }
        }
    }

    let inode = match target_inode {
        Some(i) => i,
        None => return Ok((None, None)),
    };

    // Find which process owns this inode by checking /proc/*/fd/
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let pid_str = entry.file_name().to_string_lossy().to_string();
        let pid: u32 = match pid_str.parse() {
            Ok(p) => p,
            Err(_) => continue,
        };

        let fd_path = Path::new("/proc").join(&pid_str).join("fd");
        if let Ok(fd_entries) = fs::read_dir(&fd_path) {
            for fd_entry in fd_entries.flatten() {
                if let Ok(link) = fs::read_link(fd_entry.path()) {
                    let link_str = link.to_string_lossy();
                    if link_str.contains(&format!("socket:[{}]", inode)) {
                        // Found the process, get its name
                        let comm_path = Path::new("/proc").join(&pid_str).join("comm");
                        let name = fs::read_to_string(comm_path)
                            .ok()
                            .map(|s| s.trim().to_string());
                        return Ok((Some(pid), name));
                    }
                }
            }
        }
    }

    Ok((None, None))
}

#[cfg(target_os = "macos")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::process::Command;

    let output = Command::new("lsof")
        .args(["-iTCP", "-sTCP:LISTEN", "-n", "-P"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 9 {
            continue;
        }

        // Name is parts[8], should end with ":port"
        let name_field = parts[8];
        if name_field.ends_with(&format!(":{}", port)) {
            let pid: u32 = parts[1].parse().unwrap_or(0);
            let name = Some(parts[0].to_string());
            return Ok((Some(pid), name));
        }
    }

    Ok((None, None))
}

#[cfg(target_os = "windows")]
fn get_process_for_port(port: u16) -> Result<(Option<u32>, Option<String>)> {
    use std::process::Command;

    let output = Command::new("netstat").args(["-ano"]).output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);

    for line in stdout.lines() {
        if !line.contains("LISTENING") {
            continue;
        }

        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 5 {
            continue;
        }

        // Local Address is like "0.0.0.0:3000"
        let local_addr = parts[1];
        if local_addr.ends_with(&format!(":{}", port)) {
            let pid: u32 = parts[4].parse().unwrap_or(0);
            return Ok((Some(pid), None)); // Windows netstat doesn't show process name
        }
    }

    Ok((None, None))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn get_process_for_port(_port: u16) -> Result<(Option<u32>, Option<String>)> {
    Ok((None, None))
}
