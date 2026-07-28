use crate::path_security::{PathSecurityError, resolve_aurapilot_path};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransactionError {
    #[error("destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error(transparent)]
    Path(#[from] PathSecurityError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct FileTransaction<'a> {
    repo: &'a Path,
}

impl<'a> FileTransaction<'a> {
    pub const fn new(repo: &'a Path) -> Self {
        Self { repo }
    }

    pub fn write(&self, relative: &Path, content: &[u8]) -> Result<PathBuf, TransactionError> {
        let destination = resolve_aurapilot_path(self.repo, relative)?;
        let parent = destination.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent")
        })?;
        fs::create_dir_all(parent)?;
        let temporary = temporary_path(&destination);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(content)?;
            file.flush()?;
            file.sync_all()?;
            fs::rename(&temporary, &destination)?;
            sync_directory(parent)?;
            Ok(destination.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn write_new(&self, relative: &Path, content: &[u8]) -> Result<PathBuf, TransactionError> {
        let destination = resolve_aurapilot_path(self.repo, relative)?;
        if destination.exists() {
            return Err(TransactionError::DestinationExists(destination));
        }
        let parent = destination.parent().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent")
        })?;
        fs::create_dir_all(parent)?;
        let temporary = temporary_path(&destination);
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temporary)?;
            file.write_all(content)?;
            file.flush()?;
            file.sync_all()?;
            fs::hard_link(&temporary, &destination).map_err(|error| {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    TransactionError::DestinationExists(destination.clone())
                } else {
                    TransactionError::Io(error)
                }
            })?;
            fs::remove_file(&temporary)?;
            sync_directory(parent)?;
            Ok(destination.clone())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    pub fn move_with_content(
        &self,
        from: &Path,
        to: &Path,
        content: &[u8],
    ) -> Result<PathBuf, TransactionError> {
        let source = resolve_aurapilot_path(self.repo, from)?;
        let destination = resolve_aurapilot_path(self.repo, to)?;
        if destination.exists() {
            return Err(TransactionError::DestinationExists(destination));
        }
        let source_original = fs::read(&source)?;
        let backup = temporary_path(&source.with_extension("rollback"));
        fs::rename(&source, &backup)?;
        match self.write_new(to, content) {
            Ok(path) => {
                if let Err(error) = fs::remove_file(&backup) {
                    let _ = fs::remove_file(&path);
                    let _ = fs::write(&source, source_original);
                    return Err(error.into());
                }
                Ok(path)
            }
            Err(error) => {
                let _ = fs::rename(&backup, &source);
                Err(error)
            }
        }
    }

    pub fn delete(&self, relative: &Path) -> Result<PathBuf, TransactionError> {
        let source = resolve_aurapilot_path(self.repo, relative)?;
        let parent = source
            .parent()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "source has no parent"))?;
        let backup = temporary_path(&source.with_extension("delete"));
        fs::rename(&source, &backup)?;
        if let Err(error) = sync_directory(parent).and_then(|()| fs::remove_file(&backup)) {
            let _ = fs::rename(&backup, &source);
            return Err(error.into());
        }
        sync_directory(parent)?;
        Ok(source)
    }
}

fn temporary_path(destination: &Path) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    destination.with_extension(format!("aurapilot-tmp-{}-{nonce}", std::process::id()))
}

fn sync_directory(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        fs::File::open(path)?.sync_all()
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_and_moves_only_inside_aurapilot() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/in-progress")).unwrap();
        let tx = FileTransaction::new(dir.path());
        tx.write(Path::new("tasks/backlog/TASK-001.yaml"), b"id: TASK-001\n")
            .unwrap();
        tx.move_with_content(
            Path::new("tasks/backlog/TASK-001.yaml"),
            Path::new("tasks/in-progress/TASK-001.yaml"),
            b"id: TASK-001\nassigned: Agent\n",
        )
        .unwrap();
        assert!(
            !dir.path()
                .join(".aurapilot/tasks/backlog/TASK-001.yaml")
                .exists()
        );
        assert!(
            dir.path()
                .join(".aurapilot/tasks/in-progress/TASK-001.yaml")
                .exists()
        );
    }

    #[test]
    fn refuses_to_overwrite_duplicate_destination() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/done")).unwrap();
        fs::write(
            dir.path().join(".aurapilot/tasks/backlog/TASK-001.yaml"),
            "old",
        )
        .unwrap();
        fs::write(
            dir.path().join(".aurapilot/tasks/done/TASK-001.yaml"),
            "existing",
        )
        .unwrap();
        let result = FileTransaction::new(dir.path()).move_with_content(
            Path::new("tasks/backlog/TASK-001.yaml"),
            Path::new("tasks/done/TASK-001.yaml"),
            b"new",
        );
        assert!(matches!(
            result,
            Err(TransactionError::DestinationExists(_))
        ));
        assert!(
            dir.path()
                .join(".aurapilot/tasks/backlog/TASK-001.yaml")
                .exists()
        );
    }

    #[test]
    fn write_new_never_overwrites_an_existing_task() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        let path = dir.path().join(".aurapilot/tasks/backlog/TASK-001.yaml");
        fs::write(&path, "original").unwrap();
        let result = FileTransaction::new(dir.path())
            .write_new(Path::new("tasks/backlog/TASK-001.yaml"), b"replacement");
        assert!(matches!(
            result,
            Err(TransactionError::DestinationExists(_))
        ));
        assert_eq!(fs::read_to_string(path).unwrap(), "original");
    }

    #[test]
    fn delete_is_scoped_to_the_protocol_directory() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        let task = dir.path().join(".aurapilot/tasks/backlog/TASK-001.yaml");
        fs::write(&task, "id: TASK-001").unwrap();
        let tx = FileTransaction::new(dir.path());

        tx.delete(Path::new("tasks/backlog/TASK-001.yaml")).unwrap();
        assert!(!task.exists());
        assert!(matches!(
            tx.delete(Path::new("../outside.yaml")),
            Err(TransactionError::Path(_))
        ));
    }
}
