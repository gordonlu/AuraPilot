use crate::model::{LocatedTask, TaskState};
use crate::path_security::{PathSecurityError, resolve_within_repository};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const PROTOCOL_FILE: &str = ".aurapilot/AGENTS.md";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PointerPrompt {
    pub task_id: String,
    pub protocol_file: String,
    pub task_file: String,
    pub repository: PathBuf,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum PointerPromptError {
    #[error("only backlog tasks can be pushed")]
    NotBacklog,
    #[error("task is missing its id")]
    MissingTaskId,
    #[error("protocol file is unavailable: {0}")]
    MissingProtocol(PathBuf),
    #[error("task file is unavailable: {0}")]
    MissingTask(PathBuf),
    #[error("task file is not inside the repository")]
    InvalidTaskPath,
    #[error(transparent)]
    Path(#[from] PathSecurityError),
}

pub fn build_pointer_prompt(
    repository: &Path,
    task: &LocatedTask,
) -> Result<PointerPrompt, PointerPromptError> {
    if task.state != TaskState::Backlog {
        return Err(PointerPromptError::NotBacklog);
    }
    let task_id = task
        .document
        .id
        .clone()
        .ok_or(PointerPromptError::MissingTaskId)?;
    let repository = fs::canonicalize(repository).map_err(PathSecurityError::Io)?;
    let protocol = resolve_within_repository(&repository, Path::new(PROTOCOL_FILE))?;
    if !protocol.is_file() {
        return Err(PointerPromptError::MissingProtocol(protocol));
    }
    let task_path = resolve_within_repository(&repository, &task.path)?;
    if !task_path.is_file() {
        return Err(PointerPromptError::MissingTask(task_path));
    }
    let relative = task_path
        .strip_prefix(&repository)
        .map_err(|_| PointerPromptError::InvalidTaskPath)?;
    let task_file = portable_path(relative);
    let text = format!(
        "执行 AuraPilot 任务 {task_id}。\n\n开始前必须读取：\n1. {PROTOCOL_FILE}\n2. {task_file}\n\n任务文件和协议文件是唯一事实来源。\n请按协议领取任务、执行、验证并更新进度。"
    );
    Ok(PointerPrompt {
        task_id,
        protocol_file: PROTOCOL_FILE.into(),
        task_file,
        repository,
        text,
    })
}

fn portable_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskDocument;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    #[test]
    fn prompt_is_a_short_pointer_and_never_copies_task_content() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("项目 with spaces");
        fs::create_dir_all(repo.join(".aurapilot/tasks/backlog")).unwrap();
        fs::write(repo.join(PROTOCOL_FILE), "secret protocol content").unwrap();
        let path = repo.join(".aurapilot/tasks/backlog/TASK-025.yaml");
        fs::write(&path, "desc: secret task content").unwrap();
        let task = LocatedTask {
            path,
            state: TaskState::Backlog,
            document: TaskDocument {
                id: Some("TASK-025".into()),
                title: Some("untrusted <script>".into()),
                extensions: BTreeMap::new(),
                ..TaskDocument::default()
            },
        };
        let prompt = build_pointer_prompt(&repo, &task).unwrap();
        assert!(prompt.text.contains("TASK-025"));
        assert!(prompt.text.contains(PROTOCOL_FILE));
        assert!(
            prompt
                .text
                .contains(".aurapilot/tasks/backlog/TASK-025.yaml")
        );
        assert!(!prompt.text.contains("secret"));
        assert!(!prompt.text.contains("<script>"));
    }

    #[test]
    fn non_backlog_tasks_cannot_be_pushed() {
        let task = LocatedTask {
            path: PathBuf::from("task.yaml"),
            state: TaskState::InProgress,
            document: TaskDocument::default(),
        };
        assert!(matches!(
            build_pointer_prompt(Path::new("/repo"), &task),
            Err(PointerPromptError::NotBacklog)
        ));
    }
}
