use crate::config::CoreConfig;
use crate::lock::{LockError, ProjectCreateLock};
use crate::model::{TaskDocument, TaskState};
use crate::transaction::{FileTransaction, TransactionError};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CreateTaskError {
    #[error(transparent)]
    Lock(#[from] LockError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serialize(#[from] serde_yaml::Error),
}

pub fn create_backlog_task_document(
    repo: &Path,
    config: &CoreConfig,
    mut document: TaskDocument,
) -> Result<(String, PathBuf), CreateTaskError> {
    let _lock = ProjectCreateLock::acquire(repo, &config.create_lock)?;
    loop {
        let id = next_task_id(repo, config)?;
        document.id = Some(id.clone());
        let content = serde_yaml::to_string(&document)?.into_bytes();
        let relative = PathBuf::from(format!("tasks/backlog/{id}.yaml"));
        match FileTransaction::new(repo).write_new(&relative, &content) {
            Ok(path) => return Ok((id, path)),
            Err(TransactionError::DestinationExists(_)) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

pub fn scan_task_locations(repo: &Path) -> io::Result<BTreeMap<u64, Vec<PathBuf>>> {
    let mut locations = BTreeMap::<u64, Vec<PathBuf>>::new();
    let tasks_root = repo.join(".aurapilot/tasks");
    for state in TaskState::ALL {
        let directory = tasks_root.join(state.directory());
        let entries = match fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        for entry in entries {
            let path = entry?.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let Some(number) = name
                .strip_prefix("TASK-")
                .and_then(|value| value.strip_suffix(".yaml"))
                .and_then(|value| value.parse::<u64>().ok())
            else {
                continue;
            };
            locations.entry(number).or_default().push(path);
        }
    }
    Ok(locations)
}

pub fn scan_task_numbers(repo: &Path) -> io::Result<BTreeSet<u64>> {
    Ok(scan_task_locations(repo)?.into_keys().collect())
}

pub fn duplicate_task_ids(
    repo: &Path,
    config: &CoreConfig,
) -> io::Result<Vec<(String, Vec<PathBuf>)>> {
    Ok(scan_task_locations(repo)?
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .map(|(number, paths)| {
            (
                format!("TASK-{number:0width$}", width = config.task_id_min_width),
                paths,
            )
        })
        .collect())
}

pub fn next_task_id(repo: &Path, config: &CoreConfig) -> io::Result<String> {
    let next = scan_task_numbers(repo)?.last().copied().unwrap_or(0) + 1;
    Ok(format!(
        "TASK-{next:0width$}",
        width = config.task_id_min_width
    ))
}

pub fn create_backlog_task<F>(
    repo: &Path,
    config: &CoreConfig,
    render: F,
) -> Result<(String, PathBuf), CreateTaskError>
where
    F: Fn(&str) -> Vec<u8>,
{
    let _lock = ProjectCreateLock::acquire(repo, &config.create_lock)?;
    loop {
        let id = next_task_id(repo, config)?;
        let relative = PathBuf::from(format!("tasks/backlog/{id}.yaml"));
        match FileTransaction::new(repo).write_new(&relative, &render(&id)) {
            Ok(path) => return Ok((id, path)),
            Err(TransactionError::DestinationExists(_)) => continue,
            Err(error) => return Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn scans_all_states_and_allocates_next_id() {
        let dir = tempdir().unwrap();
        for state in TaskState::ALL {
            fs::create_dir_all(dir.path().join(".aurapilot/tasks").join(state.directory()))
                .unwrap();
        }
        fs::write(
            dir.path().join(".aurapilot/tasks/backlog/TASK-002.yaml"),
            "",
        )
        .unwrap();
        fs::write(dir.path().join(".aurapilot/tasks/done/TASK-010.yaml"), "").unwrap();
        assert_eq!(
            next_task_id(dir.path(), &CoreConfig::default()).unwrap(),
            "TASK-011"
        );
    }

    #[test]
    fn reports_duplicate_ids_across_state_directories() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/done")).unwrap();
        fs::write(
            dir.path().join(".aurapilot/tasks/backlog/TASK-001.yaml"),
            "",
        )
        .unwrap();
        fs::write(dir.path().join(".aurapilot/tasks/done/TASK-001.yaml"), "").unwrap();
        let duplicates = duplicate_task_ids(dir.path(), &CoreConfig::default()).unwrap();
        assert_eq!(duplicates.len(), 1);
        assert_eq!(duplicates[0].0, "TASK-001");
        assert_eq!(duplicates[0].1.len(), 2);
    }

    #[test]
    fn creation_holds_project_lock_and_writes_the_allocated_id() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".aurapilot/tasks/backlog")).unwrap();
        let (id, path) = create_backlog_task(dir.path(), &CoreConfig::default(), |id| {
            format!("id: {id}\ntitle: test\n").into_bytes()
        })
        .unwrap();
        assert_eq!(id, "TASK-001");
        assert!(path.ends_with("TASK-001.yaml"));
        assert!(fs::read_to_string(path).unwrap().contains("id: TASK-001"));
    }
}
