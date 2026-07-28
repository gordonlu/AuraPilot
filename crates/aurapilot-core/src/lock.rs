use crate::config::LockConfig;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LockError {
    #[error("timed out waiting for project task creation lock after {0:?}")]
    Timeout(Duration),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Serialize, Deserialize)]
struct LockMetadata {
    pid: u32,
    created_unix_ms: u128,
}

pub struct ProjectCreateLock {
    path: PathBuf,
    _file: File,
}

impl ProjectCreateLock {
    pub fn acquire(repo: &Path, config: &LockConfig) -> Result<Self, LockError> {
        let path = repo.join(".aurapilot/.create-task.lock");
        let started = Instant::now();
        loop {
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => {
                    let metadata = LockMetadata {
                        pid: std::process::id(),
                        created_unix_ms: unix_millis(),
                    };
                    serde_json::to_writer(&mut file, &metadata).map_err(io::Error::other)?;
                    file.flush()?;
                    file.sync_all()?;
                    return Ok(Self { path, _file: file });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if is_stale(&path, config.stale_after)? {
                        match fs::remove_file(&path) {
                            Ok(()) => continue,
                            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                            Err(error) => return Err(error.into()),
                        }
                    }
                    if started.elapsed() >= config.wait_timeout {
                        return Err(LockError::Timeout(config.wait_timeout));
                    }
                    thread::sleep(config.retry_interval);
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
}

impl Drop for ProjectCreateLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn is_stale(path: &Path, stale_after: Duration) -> io::Result<bool> {
    let modified = fs::metadata(path)?.modified()?;
    Ok(SystemTime::now()
        .duration_since(modified)
        .unwrap_or_default()
        >= stale_after)
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn lock_is_exclusive_and_removed_on_drop() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".aurapilot")).unwrap();
        let config = LockConfig {
            wait_timeout: Duration::from_millis(20),
            retry_interval: Duration::from_millis(2),
            stale_after: Duration::from_secs(30),
        };
        let lock = ProjectCreateLock::acquire(dir.path(), &config).unwrap();
        assert!(matches!(
            ProjectCreateLock::acquire(dir.path(), &config),
            Err(LockError::Timeout(_))
        ));
        drop(lock);
        assert!(ProjectCreateLock::acquire(dir.path(), &config).is_ok());
    }
}
