use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::detect::{self, DetectedProject};
use crate::git_status::{self, GitStatus};

pub struct GitStatusResult {
    pub path: String,
    pub git_status: Option<GitStatus>,
    pub detection: Option<DetectedProject>,
}

pub struct GitWorker {
    request_tx: Sender<String>,
    result_rx: Receiver<GitStatusResult>,
    cache: HashMap<String, CachedEntry>,
}

struct CachedEntry {
    git_status: Option<GitStatus>,
    detection: Option<DetectedProject>,
    fetched_at: Instant,
}

impl GitWorker {
    pub fn new() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<String>();
        let (result_tx, result_rx) = mpsc::channel::<GitStatusResult>();

        thread::spawn(move || {
            while let Ok(path_str) = request_rx.recv() {
                let path = PathBuf::from(&path_str);
                let git_status = if path.join(".git").exists() {
                    git_status::get_status(&path).ok()
                } else {
                    None
                };
                let detection = detect::detect(&path).ok();

                let _ = result_tx.send(GitStatusResult {
                    path: path_str,
                    git_status,
                    detection,
                });
            }
        });

        Self {
            request_tx,
            result_rx,
            cache: HashMap::new(),
        }
    }

    /// Request a background status fetch for a path
    pub fn request(&self, path: &str) {
        let _ = self.request_tx.send(path.to_string());
    }

    /// Poll for completed results, updating cache. Call this in the event loop.
    pub fn poll(&mut self) -> bool {
        let mut updated = false;
        while let Ok(result) = self.result_rx.try_recv() {
            self.cache.insert(
                result.path,
                CachedEntry {
                    git_status: result.git_status,
                    detection: result.detection,
                    fetched_at: Instant::now(),
                },
            );
            updated = true;
        }
        updated
    }

    /// Get cached git status for a path
    pub fn get_git_status(&self, path: &str) -> Option<&GitStatus> {
        self.cache.get(path).and_then(|e| e.git_status.as_ref())
    }

    /// Get cached detection for a path
    pub fn get_detection(&self, path: &str) -> Option<&DetectedProject> {
        self.cache.get(path).and_then(|e| e.detection.as_ref())
    }

    /// Check if cache entry is stale (older than 30 seconds)
    pub fn is_stale(&self, path: &str) -> bool {
        match self.cache.get(path) {
            Some(entry) => entry.fetched_at.elapsed().as_secs() > 30,
            None => true,
        }
    }

    /// Invalidate all cache entries (for F5 refresh)
    pub fn invalidate_all(&mut self) {
        self.cache.clear();
    }
}
