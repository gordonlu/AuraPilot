use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::model::{LocatedTask, ProjectDocument, TaskDocument, TaskState};
use std::fs;
use std::path::Path;

pub const REQUIRED_TASK_COLLECTION_FIELDS: [&str; 3] = ["accept", "log", "blockers"];

pub fn missing_task_collection_fields(
    source: &str,
) -> Result<Vec<&'static str>, serde_yaml::Error> {
    let value = serde_yaml::from_str::<serde_yaml::Value>(source)?;
    let Some(mapping) = value.as_mapping() else {
        return Ok(REQUIRED_TASK_COLLECTION_FIELDS.to_vec());
    };
    Ok(REQUIRED_TASK_COLLECTION_FIELDS
        .into_iter()
        .filter(|field| !mapping.contains_key(serde_yaml::Value::String((*field).into())))
        .collect())
}

pub fn parse_task_str(source: &str, path: &Path) -> Result<LocatedTask, Diagnostic> {
    let state = TaskState::from_task_path(path).ok_or_else(|| {
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::InvalidLocation,
            "task file is not inside a supported state directory",
        )
        .path(path)
    })?;
    let document = serde_yaml::from_str::<TaskDocument>(source).map_err(|error| {
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::ParseFailed,
            error.to_string(),
        )
        .path(path)
    })?;
    Ok(LocatedTask {
        path: path.to_path_buf(),
        state,
        document,
    })
}

pub fn parse_task_file(path: &Path) -> Result<LocatedTask, Diagnostic> {
    let source = fs::read_to_string(path).map_err(|error| {
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::ParseFailed,
            error.to_string(),
        )
        .path(path)
    })?;
    parse_task_str(&source, path)
}

pub fn parse_project_str(source: &str, path: &Path) -> Result<ProjectDocument, Diagnostic> {
    serde_yaml::from_str(source).map_err(|error| {
        Diagnostic::new(
            Severity::Error,
            DiagnosticCode::ParseFailed,
            error.to_string(),
        )
        .path(path)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_agent_extensions() {
        let task = parse_task_str(
            "id: TASK-001\ntitle: Test\npriority: P1\ntype: feature\ncreated: 2026-07-28\nlog:\n  - ts: 2026-07-28T10:00:00Z\n    msg: started\n    tokens_used: 42\ncustom: yes\n",
            Path::new(".aurapilot/tasks/backlog/TASK-001.yaml"),
        ).unwrap();
        assert_eq!(
            task.document.extensions.get("custom"),
            Some(&serde_json::json!("yes"))
        );
        assert_eq!(
            task.document.log[0].extensions.get("tokens_used"),
            Some(&serde_json::json!(42))
        );
    }

    #[test]
    fn rejects_non_numeric_pr() {
        let error = parse_task_str(
            "id: TASK-001\ntitle: Test\npriority: P1\ntype: feature\ncreated: 2026-07-28\npr: '#12'\n",
            Path::new(".aurapilot/tasks/in-review/TASK-001.yaml"),
        ).unwrap_err();
        assert_eq!(error.code, DiagnosticCode::ParseFailed);
    }

    #[test]
    fn rejects_a_task_outside_the_protocol_tree() {
        let error =
            parse_task_str("id: TASK-001\n", Path::new("other/backlog/TASK-001.yaml")).unwrap_err();
        assert_eq!(error.code, DiagnosticCode::InvalidLocation);
    }

    #[test]
    fn reports_protocol_arrays_that_were_omitted_from_yaml() {
        assert_eq!(
            missing_task_collection_fields("id: TASK-001\naccept: []\n").unwrap(),
            vec!["log", "blockers"]
        );
    }
}
