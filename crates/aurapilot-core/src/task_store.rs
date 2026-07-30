use crate::config::CoreConfig;
use crate::diagnostic::Severity;
use crate::model::{LocatedTask, TaskDocument, TaskState};
use crate::parser::parse_task_file;
use crate::task_id::{CreateTaskError, create_backlog_task_document};
use crate::transaction::{FileTransaction, TransactionError};
use crate::validation::{SchemaValidator, SeverityProfile, StatePolicyValidator};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct CreateTaskInput {
    pub title: String,
    pub priority: String,
    pub task_type: String,
    pub desc: Option<String>,
    #[serde(default)]
    pub accept: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct UpdateTaskInput {
    pub title: String,
    pub priority: String,
    pub task_type: String,
    pub desc: Option<String>,
    #[serde(default)]
    pub accept: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct TransitionTaskInput {
    pub target: TaskState,
    pub assigned: Option<String>,
    pub branch: Option<String>,
    pub pr: Option<u64>,
    pub waiting: Option<String>,
    pub commit: Option<String>,
}

#[derive(Debug, Error)]
pub enum TaskStoreError {
    #[error("task id has an invalid format: {0}")]
    InvalidId(String),
    #[error("task not found: {0}")]
    NotFound(String),
    #[error("task exists in multiple state directories: {0}")]
    Duplicate(String),
    #[error("task data is invalid: {0}")]
    Invalid(String),
    #[error(transparent)]
    Create(#[from] CreateTaskError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Serialize(#[from] serde_yaml::Error),
}

pub fn create_task(
    repo: &Path,
    config: &CoreConfig,
    input: CreateTaskInput,
) -> Result<LocatedTask, TaskStoreError> {
    let document = TaskDocument {
        id: Some(format!(
            "TASK-{:0width$}",
            0,
            width = config.task_id_min_width
        )),
        title: Some(input.title.trim().to_owned()),
        priority: Some(input.priority),
        task_type: Some(input.task_type),
        created: Some(Utc::now().date_naive().to_string()),
        desc: normalize_optional(input.desc),
        accept: normalize_lines(input.accept),
        ..TaskDocument::default()
    };
    validate_document(&document, TaskState::Backlog, Path::new("pending.yaml"))?;
    let (_, path) = create_backlog_task_document(repo, config, document)?;
    parse_task_file(&path).map_err(|diagnostic| TaskStoreError::Invalid(diagnostic.message))
}

pub fn update_task(
    repo: &Path,
    id: &str,
    input: UpdateTaskInput,
) -> Result<LocatedTask, TaskStoreError> {
    let mut task = locate_task(repo, id)?;
    task.document.title = Some(input.title.trim().to_owned());
    task.document.priority = Some(input.priority);
    task.document.task_type = Some(input.task_type);
    task.document.desc = normalize_optional(input.desc);
    task.document.accept = normalize_lines(input.accept);
    validate_document(&task.document, task.state, &task.path)?;
    let content = serde_yaml::to_string(&task.document)?;
    let relative = task_relative(task.state, id);
    FileTransaction::new(repo).write(&relative, content.as_bytes())?;
    locate_task(repo, id)
}

pub fn transition_task(
    repo: &Path,
    id: &str,
    input: TransitionTaskInput,
) -> Result<LocatedTask, TaskStoreError> {
    let mut task = locate_task(repo, id)?;
    if task.state == input.target {
        return Ok(task);
    }
    match input.target {
        TaskState::Backlog => {
            task.document.assigned = None;
            task.document.branch = None;
            task.document.started = None;
            task.document.pr = None;
            task.document.waiting = None;
            task.document.completed = None;
            task.document.commit = None;
        }
        TaskState::InProgress => {
            task.document.assigned = input.assigned.or(task.document.assigned);
            task.document.branch = input.branch.or(task.document.branch);
            task.document.started.get_or_insert_with(now);
            task.document.pr = None;
            task.document.waiting = None;
            task.document.completed = None;
            task.document.commit = None;
        }
        TaskState::InReview => {
            task.document.assigned = input.assigned.or(task.document.assigned);
            task.document.branch = input.branch.or(task.document.branch);
            task.document.pr = input.pr.or(task.document.pr);
            task.document.waiting = input.waiting.or(task.document.waiting);
            task.document.completed = None;
            task.document.commit = None;
        }
        TaskState::Done => {
            task.document.commit = input.commit.or(task.document.commit);
            task.document.completed.get_or_insert_with(now);
        }
    }
    let destination = repo
        .join(".aurapilot")
        .join(task_relative(input.target, id));
    validate_document(&task.document, input.target, &destination)?;
    let content = serde_yaml::to_string(&task.document)?;
    FileTransaction::new(repo).move_with_content(
        &task_relative(task.state, id),
        &task_relative(input.target, id),
        content.as_bytes(),
    )?;
    locate_task(repo, id)
}

pub fn delete_task(repo: &Path, id: &str) -> Result<PathBuf, TaskStoreError> {
    let task = locate_task(repo, id)?;
    Ok(FileTransaction::new(repo).delete(&task_relative(task.state, id))?)
}

pub fn locate_task(repo: &Path, id: &str) -> Result<LocatedTask, TaskStoreError> {
    validate_id(id)?;
    let matches = TaskState::ALL
        .into_iter()
        .filter_map(|state| {
            let path = repo.join(".aurapilot").join(task_relative(state, id));
            path.is_file().then_some(path)
        })
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [] => Err(TaskStoreError::NotFound(id.to_owned())),
        [path] => {
            parse_task_file(path).map_err(|diagnostic| TaskStoreError::Invalid(diagnostic.message))
        }
        _ => Err(TaskStoreError::Duplicate(id.to_owned())),
    }
}

fn validate_document(
    document: &TaskDocument,
    state: TaskState,
    path: &Path,
) -> Result<(), TaskStoreError> {
    let located = LocatedTask {
        path: path.to_path_buf(),
        state,
        document: document.clone(),
    };
    let mut diagnostics = SchemaValidator::new(SeverityProfile::strict()).validate_task(document);
    diagnostics.extend(StatePolicyValidator::validate(
        &located,
        SeverityProfile::strict(),
    ));
    let messages = diagnostics
        .into_iter()
        .filter(|item| item.severity >= Severity::Error)
        .map(|item| item.message)
        .collect::<Vec<_>>();
    if messages.is_empty() {
        Ok(())
    } else {
        Err(TaskStoreError::Invalid(messages.join("; ")))
    }
}

fn validate_id(id: &str) -> Result<(), TaskStoreError> {
    let valid = id
        .strip_prefix("TASK-")
        .is_some_and(|number| number.len() >= 3 && number.chars().all(|c| c.is_ascii_digit()));
    if valid {
        Ok(())
    } else {
        Err(TaskStoreError::InvalidId(id.to_owned()))
    }
}

fn task_relative(state: TaskState, id: &str) -> PathBuf {
    PathBuf::from("tasks")
        .join(state.directory())
        .join(format!("{id}.yaml"))
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_owned();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

fn normalize_lines(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .collect()
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn repo() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        for state in TaskState::ALL {
            fs::create_dir_all(dir.path().join(".aurapilot/tasks").join(state.directory()))
                .unwrap();
        }
        dir
    }

    fn create_input() -> CreateTaskInput {
        CreateTaskInput {
            title: "Build board".into(),
            priority: "P1".into(),
            task_type: "feature".into(),
            desc: Some("Phase 3 UI".into()),
            accept: vec!["Renders four columns".into()],
        }
    }

    #[test]
    fn create_update_and_delete_preserve_protocol_data() {
        let repo = repo();
        let created = create_task(repo.path(), &CoreConfig::default(), create_input()).unwrap();
        assert_eq!(created.document.id.as_deref(), Some("TASK-001"));

        let updated = update_task(
            repo.path(),
            "TASK-001",
            UpdateTaskInput {
                title: "Production board".into(),
                priority: "P0".into(),
                task_type: "feature".into(),
                desc: None,
                accept: vec!["Keyboard accessible".into()],
            },
        )
        .unwrap();
        assert_eq!(updated.document.title.as_deref(), Some("Production board"));
        assert_eq!(updated.document.priority.as_deref(), Some("P0"));

        let deleted = delete_task(repo.path(), "TASK-001").unwrap();
        assert!(!deleted.exists());
        assert!(matches!(
            locate_task(repo.path(), "TASK-001"),
            Err(TaskStoreError::NotFound(_))
        ));
    }

    #[test]
    fn transitions_require_state_fields_and_reopen_clears_them() {
        let repo = repo();
        create_task(repo.path(), &CoreConfig::default(), create_input()).unwrap();
        let error = transition_task(
            repo.path(),
            "TASK-001",
            TransitionTaskInput {
                target: TaskState::InProgress,
                ..TransitionTaskInput::default()
            },
        )
        .unwrap_err();
        assert!(matches!(error, TaskStoreError::Invalid(_)));

        let active = transition_task(
            repo.path(),
            "TASK-001",
            TransitionTaskInput {
                target: TaskState::InProgress,
                assigned: Some("codex".into()),
                branch: Some("task/phase-3".into()),
                ..TransitionTaskInput::default()
            },
        )
        .unwrap();
        assert_eq!(active.state, TaskState::InProgress);

        let reopened = transition_task(
            repo.path(),
            "TASK-001",
            TransitionTaskInput {
                target: TaskState::Backlog,
                ..TransitionTaskInput::default()
            },
        )
        .unwrap();
        assert_eq!(reopened.state, TaskState::Backlog);
        assert!(reopened.document.assigned.is_none());
        assert!(reopened.document.started.is_none());
    }

    #[test]
    fn completing_a_task_does_not_require_a_commit() {
        let repo = repo();
        create_task(repo.path(), &CoreConfig::default(), create_input()).unwrap();
        transition_task(
            repo.path(),
            "TASK-001",
            TransitionTaskInput {
                target: TaskState::InProgress,
                assigned: Some("codex".into()),
                branch: Some("task/phase-3".into()),
                ..TransitionTaskInput::default()
            },
        )
        .unwrap();

        let done = transition_task(
            repo.path(),
            "TASK-001",
            TransitionTaskInput {
                target: TaskState::Done,
                commit: None,
                ..TransitionTaskInput::default()
            },
        )
        .unwrap();

        assert_eq!(done.state, TaskState::Done);
        assert!(done.document.commit.is_none());
        assert!(done.document.completed.is_some());
    }

    #[test]
    fn rejects_duplicate_and_traversal_shaped_ids() {
        let repo = repo();
        create_task(repo.path(), &CoreConfig::default(), create_input()).unwrap();
        fs::copy(
            repo.path().join(".aurapilot/tasks/backlog/TASK-001.yaml"),
            repo.path().join(".aurapilot/tasks/done/TASK-001.yaml"),
        )
        .unwrap();
        assert!(matches!(
            locate_task(repo.path(), "TASK-001"),
            Err(TaskStoreError::Duplicate(_))
        ));
        assert!(matches!(
            delete_task(repo.path(), "../outside"),
            Err(TaskStoreError::InvalidId(_))
        ));
    }
}
