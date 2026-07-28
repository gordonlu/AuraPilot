use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum TaskState {
    #[default]
    Backlog,
    InProgress,
    InReview,
    Done,
}

impl TaskState {
    pub const ALL: [Self; 4] = [Self::Backlog, Self::InProgress, Self::InReview, Self::Done];

    pub const fn directory(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::InProgress => "in-progress",
            Self::InReview => "in-review",
            Self::Done => "done",
        }
    }

    pub fn from_task_path(path: &Path) -> Option<Self> {
        let components = path
            .components()
            .filter_map(|component| component.as_os_str().to_str())
            .collect::<Vec<_>>();
        let window = components.windows(4).find(|parts| {
            parts[0] == ".aurapilot" && parts[1] == "tasks" && parts[3].ends_with(".yaml")
        })?;
        Self::ALL
            .into_iter()
            .find(|state| state.directory() == window[2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_state_requires_the_standard_protocol_location() {
        assert_eq!(
            TaskState::from_task_path(Path::new(
                "/repo/.aurapilot/tasks/in-progress/TASK-001.yaml"
            )),
            Some(TaskState::InProgress)
        );
        assert_eq!(
            TaskState::from_task_path(Path::new("other/backlog/TASK-001.yaml")),
            None
        );
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TaskLogEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct TaskDocument {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub task_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assigned: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub desc: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accept: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub log: Vec<TaskLogEntry>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blockers: Vec<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq)]
pub struct ProjectDocument {
    pub name: Option<String>,
    pub owner: Option<String>,
    pub health: Option<String>,
    pub sprint: Option<String>,
    pub notes: Option<String>,
    pub schema_version: Option<u32>,
    pub created: Option<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LocatedTask {
    pub path: PathBuf,
    pub state: TaskState,
    pub document: TaskDocument,
}
