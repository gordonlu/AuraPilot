use crate::config::CoreConfig;
use crate::project_registry::RegisteredProject;
use notify::event::{ModifyKind, RenameMode};
use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, mpsc};
use std::thread::{self, JoinHandle};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProjectChangeKind {
    Created,
    Modified,
    Removed,
    Renamed,
    RescanRequired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ProjectChange {
    pub project_id: Uuid,
    pub kind: ProjectChangeKind,
    pub paths: Vec<PathBuf>,
}

#[derive(Debug, Error)]
pub enum WatchError {
    #[error("project is already watched: {0}")]
    Duplicate(Uuid),
    #[error("project is not watched: {0}")]
    NotFound(Uuid),
    #[error(transparent)]
    Notify(#[from] notify::Error),
}

enum WorkerMessage {
    Event(notify::Result<Event>),
    Shutdown,
}

pub struct ProjectWatchService {
    watcher: RecommendedWatcher,
    registrations: Arc<RwLock<BTreeMap<Uuid, PathBuf>>>,
    worker_tx: mpsc::Sender<WorkerMessage>,
    worker: Option<JoinHandle<()>>,
}

impl ProjectWatchService {
    pub fn new<F>(config: &CoreConfig, on_change: F) -> Result<Self, WatchError>
    where
        F: Fn(ProjectChange) + Send + Sync + 'static,
    {
        let registrations = Arc::new(RwLock::new(BTreeMap::<Uuid, PathBuf>::new()));
        let worker_registrations = Arc::clone(&registrations);
        let (worker_tx, worker_rx) = mpsc::channel::<WorkerMessage>();
        let notify_tx = worker_tx.clone();
        let watcher = RecommendedWatcher::new(
            move |event| {
                let _ = notify_tx.send(WorkerMessage::Event(event));
            },
            Config::default(),
        )?;
        let debounce = config.watcher_debounce;
        let callback = Arc::new(on_change);
        let worker = thread::spawn(move || {
            while let Ok(message) = worker_rx.recv() {
                let WorkerMessage::Event(first) = message else {
                    break;
                };
                let mut batch = vec![first];
                let mut shutdown = false;
                loop {
                    match worker_rx.recv_timeout(debounce) {
                        Ok(WorkerMessage::Event(event)) => batch.push(event),
                        Ok(WorkerMessage::Shutdown) => {
                            shutdown = true;
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break,
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            shutdown = true;
                            break;
                        }
                    }
                }
                emit_batch(&worker_registrations, callback.as_ref(), batch);
                if shutdown {
                    break;
                }
            }
        });
        Ok(Self {
            watcher,
            registrations,
            worker_tx,
            worker: Some(worker),
        })
    }

    pub fn watch_project(&mut self, project: &RegisteredProject) -> Result<(), WatchError> {
        let aura = project.path.join(".aurapilot");
        let mut registrations = self.registrations.write().expect("watch registry poisoned");
        if registrations.contains_key(&project.id) {
            return Err(WatchError::Duplicate(project.id));
        }
        self.watcher.watch(&aura, RecursiveMode::Recursive)?;
        registrations.insert(project.id, aura);
        Ok(())
    }

    pub fn unwatch_project(&mut self, id: Uuid) -> Result<(), WatchError> {
        let aura = self
            .registrations
            .write()
            .expect("watch registry poisoned")
            .remove(&id)
            .ok_or(WatchError::NotFound(id))?;
        match self.watcher.unwatch(&aura) {
            Ok(()) => Ok(()),
            Err(_error) if !aura.exists() => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

impl Drop for ProjectWatchService {
    fn drop(&mut self) {
        let _ = self.worker_tx.send(WorkerMessage::Shutdown);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn emit_batch<F>(
    registrations: &RwLock<BTreeMap<Uuid, PathBuf>>,
    callback: &F,
    batch: Vec<notify::Result<Event>>,
) where
    F: Fn(ProjectChange),
{
    let registrations = registrations.read().expect("watch registry poisoned");
    let mut changes = BTreeMap::<Uuid, (ProjectChangeKind, BTreeSet<PathBuf>)>::new();
    for result in batch {
        match result {
            Ok(event) => {
                let Some(kind) = normalize_kind(&event.kind) else {
                    continue;
                };
                for (id, root) in registrations.iter() {
                    let relevant = event.paths.iter().filter(|path| path.starts_with(root));
                    for path in relevant {
                        let entry = changes
                            .entry(*id)
                            .or_insert_with(|| (kind, BTreeSet::new()));
                        entry.0 = merge_kind(entry.0, kind);
                        entry.1.insert(path.clone());
                    }
                }
            }
            Err(_) => {
                for id in registrations.keys() {
                    changes
                        .entry(*id)
                        .or_insert_with(|| (ProjectChangeKind::RescanRequired, BTreeSet::new()));
                }
            }
        }
    }
    drop(registrations);
    for (project_id, (kind, paths)) in changes {
        callback(ProjectChange {
            project_id,
            kind,
            paths: paths.into_iter().collect(),
        });
    }
}

fn normalize_kind(kind: &EventKind) -> Option<ProjectChangeKind> {
    match kind {
        EventKind::Create(_) => Some(ProjectChangeKind::Created),
        EventKind::Modify(ModifyKind::Name(
            RenameMode::Any | RenameMode::Both | RenameMode::From | RenameMode::To,
        )) => Some(ProjectChangeKind::Renamed),
        EventKind::Modify(_) => Some(ProjectChangeKind::Modified),
        EventKind::Remove(_) => Some(ProjectChangeKind::Removed),
        EventKind::Other | EventKind::Any => Some(ProjectChangeKind::RescanRequired),
        EventKind::Access(_) => None,
    }
}

fn merge_kind(current: ProjectChangeKind, next: ProjectChangeKind) -> ProjectChangeKind {
    use ProjectChangeKind::*;
    match (current, next) {
        (RescanRequired, _) | (_, RescanRequired) => RescanRequired,
        (Renamed, _) | (_, Renamed) => Renamed,
        (Created, Removed) | (Removed, Created) => Renamed,
        (_, value) => value,
    }
}

pub fn is_protocol_path(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == ".aurapilot")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn external_file_change_is_delivered_within_the_configured_limit() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo with spaces");
        fs::create_dir_all(repo.join(".aurapilot/tasks/backlog")).unwrap();
        let project = RegisteredProject {
            id: Uuid::new_v4(),
            path: repo.clone(),
            registered_at: Utc::now().to_rfc3339(),
            last_profile_id: None,
        };
        let config = CoreConfig::default();
        let (tx, rx) = mpsc::channel();
        let mut service = ProjectWatchService::new(&config, move |change| {
            tx.send(change).unwrap();
        })
        .unwrap();
        service.watch_project(&project).unwrap();

        let path = repo.join(".aurapilot/tasks/backlog/TASK-001.yaml");
        fs::write(&path, "id: TASK-001\n").unwrap();
        let change = rx.recv_timeout(config.watcher_delivery_timeout).unwrap();
        assert_eq!(change.project_id, project.id);
        assert!(change.paths.iter().any(|changed| changed == &path));
    }

    #[test]
    fn remove_create_batches_are_normalized_as_rename() {
        assert_eq!(
            merge_kind(ProjectChangeKind::Removed, ProjectChangeKind::Created),
            ProjectChangeKind::Renamed
        );
    }

    #[test]
    fn watcher_errors_request_a_full_rescan() {
        let id = Uuid::new_v4();
        let registrations = RwLock::new(BTreeMap::from([(id, PathBuf::from("/repo/.aurapilot"))]));
        let (tx, rx) = mpsc::channel();
        emit_batch(
            &registrations,
            &move |change| tx.send(change).unwrap(),
            vec![Err(notify::Error::generic("overflow"))],
        );
        let change = rx.recv().unwrap();
        assert_eq!(change.project_id, id);
        assert_eq!(change.kind, ProjectChangeKind::RescanRequired);
    }
}
