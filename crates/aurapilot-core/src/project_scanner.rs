use crate::config::CoreConfig;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::model::{LocatedTask, ProjectDocument, TaskState};
use crate::parser::{parse_project_str, parse_task_file};
use crate::project_registry::RegisteredProject;
use crate::validation::{SchemaValidator, SeverityProfile, StatePolicyValidator};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct ProjectSnapshot {
    pub registration: RegisteredProject,
    pub project: Option<ProjectDocument>,
    pub tasks: Vec<LocatedTask>,
    pub diagnostics: Vec<Diagnostic>,
}

impl ProjectSnapshot {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity >= Severity::Error)
    }
}

pub fn scan_project(
    registration: &RegisteredProject,
    config: &CoreConfig,
    profile: SeverityProfile,
) -> ProjectSnapshot {
    let mut snapshot = ProjectSnapshot {
        registration: registration.clone(),
        project: None,
        tasks: Vec::new(),
        diagnostics: Vec::new(),
    };
    let repo = &registration.path;
    if !repo.is_dir() {
        snapshot.diagnostics.push(
            Diagnostic::new(
                Severity::Blocked,
                DiagnosticCode::ProjectUnavailable,
                "registered project directory is unavailable",
            )
            .path(repo),
        );
        return snapshot;
    }

    let aura = repo.join(".aurapilot");
    for required in ["AGENTS.md", "project.yaml", "schema.json"] {
        let path = aura.join(required);
        if !path.is_file() {
            snapshot.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    DiagnosticCode::MissingProtocolFile,
                    format!("missing protocol file `{required}`"),
                )
                .path(path),
            );
        }
    }

    let schema = SchemaValidator::new(profile);
    let project_path = aura.join("project.yaml");
    match fs::read_to_string(&project_path) {
        Ok(source) => match parse_project_str(&source, &project_path) {
            Ok(project) => {
                snapshot.diagnostics.extend(
                    schema
                        .validate_project(&project, config)
                        .into_iter()
                        .map(|diagnostic| diagnostic.path(project_path.clone())),
                );
                snapshot.project = Some(project);
            }
            Err(diagnostic) => snapshot.diagnostics.push(diagnostic),
        },
        Err(error) if project_path.exists() => snapshot.diagnostics.push(
            Diagnostic::new(
                Severity::Error,
                DiagnosticCode::ParseFailed,
                format!("cannot read project metadata: {error}"),
            )
            .path(project_path.clone()),
        ),
        Err(_) => {}
    }

    for state in TaskState::ALL {
        scan_state_directory(&mut snapshot, &schema, profile, &aura, state);
    }
    detect_duplicate_document_ids(&mut snapshot);
    snapshot
        .tasks
        .sort_by(|left, right| left.path.cmp(&right.path));
    snapshot
}

pub fn scan_projects(
    projects: &[RegisteredProject],
    config: &CoreConfig,
    profile: SeverityProfile,
) -> Vec<ProjectSnapshot> {
    projects
        .iter()
        .map(|project| scan_project(project, config, profile))
        .collect()
}

fn scan_state_directory(
    snapshot: &mut ProjectSnapshot,
    schema: &SchemaValidator,
    profile: SeverityProfile,
    aura: &Path,
    state: TaskState,
) {
    let directory = aura.join("tasks").join(state.directory());
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) => {
            snapshot.diagnostics.push(
                Diagnostic::new(
                    Severity::Error,
                    DiagnosticCode::MissingProtocolFile,
                    format!("cannot read state directory: {error}"),
                )
                .path(directory),
            );
            return;
        }
    };
    for entry in entries {
        let path = match entry {
            Ok(entry) => entry.path(),
            Err(error) => {
                snapshot.diagnostics.push(Diagnostic::new(
                    Severity::Error,
                    DiagnosticCode::ParseFailed,
                    format!("cannot read task directory entry: {error}"),
                ));
                continue;
            }
        };
        if path.extension().and_then(|extension| extension.to_str()) != Some("yaml") {
            continue;
        }
        match parse_task_file(&path) {
            Ok(task) => {
                snapshot.diagnostics.extend(
                    schema
                        .validate_task(&task.document)
                        .into_iter()
                        .map(|diagnostic| diagnostic.path(path.clone())),
                );
                snapshot
                    .diagnostics
                    .extend(StatePolicyValidator::validate(&task, profile));
                if let (Some(id), Some(stem)) = (
                    task.document.id.as_deref(),
                    path.file_stem().and_then(|stem| stem.to_str()),
                ) && id != stem
                {
                    snapshot.diagnostics.push(
                        Diagnostic::new(
                            Severity::Error,
                            DiagnosticCode::TaskIdMismatch,
                            format!("task field id `{id}` does not match filename `{stem}`"),
                        )
                        .field("id")
                        .path(path.clone()),
                    );
                }
                snapshot.tasks.push(task);
            }
            Err(diagnostic) => snapshot.diagnostics.push(diagnostic),
        }
    }
}

fn detect_duplicate_document_ids(snapshot: &mut ProjectSnapshot) {
    let mut locations = BTreeMap::<String, Vec<_>>::new();
    for task in &snapshot.tasks {
        if let Some(id) = &task.document.id {
            locations
                .entry(id.clone())
                .or_default()
                .push(task.path.clone());
        }
    }
    for (id, paths) in locations.into_iter().filter(|(_, paths)| paths.len() > 1) {
        for path in &paths {
            snapshot.diagnostics.push(
                Diagnostic::new(
                    Severity::Blocked,
                    DiagnosticCode::DuplicateTaskId,
                    format!("task id `{id}` exists in multiple state directories"),
                )
                .field("id")
                .path(path),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::fs;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn registration(path: &Path) -> RegisteredProject {
        RegisteredProject {
            id: Uuid::new_v4(),
            path: path.to_path_buf(),
            registered_at: Utc::now().to_rfc3339(),
            last_profile_id: None,
        }
    }

    fn protocol_repo(path: &Path) {
        for state in TaskState::ALL {
            fs::create_dir_all(path.join(".aurapilot/tasks").join(state.directory())).unwrap();
        }
        fs::write(path.join(".aurapilot/AGENTS.md"), "# Protocol").unwrap();
        fs::write(path.join(".aurapilot/schema.json"), "{}").unwrap();
        fs::write(
            path.join(".aurapilot/project.yaml"),
            "name: demo\nowner: test\nhealth: green\nschema_version: 1\ncreated: 2026-07-28\n",
        )
        .unwrap();
    }

    #[test]
    fn scans_valid_project_and_tasks() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir(&repo).unwrap();
        protocol_repo(&repo);
        fs::write(
            repo.join(".aurapilot/tasks/backlog/TASK-001.yaml"),
            "id: TASK-001\ntitle: Test\npriority: P1\ntype: test\ncreated: 2026-07-28\n",
        )
        .unwrap();
        let snapshot = scan_project(
            &registration(&repo),
            &CoreConfig::default(),
            SeverityProfile::lenient(),
        );
        assert_eq!(snapshot.tasks.len(), 1);
        assert!(!snapshot.has_errors());
    }

    #[test]
    fn reports_duplicate_ids_and_unavailable_projects_without_panicking() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir(&repo).unwrap();
        protocol_repo(&repo);
        let task = "id: TASK-001\ntitle: Test\npriority: P1\ntype: test\ncreated: 2026-07-28\n";
        fs::write(repo.join(".aurapilot/tasks/backlog/TASK-001.yaml"), task).unwrap();
        fs::write(repo.join(".aurapilot/tasks/done/TASK-002.yaml"), format!("{task}assigned: Agent\nbranch: task/one\nstarted: 2026-07-28T10:00:00Z\ncompleted: 2026-07-28T11:00:00Z\ncommit: abcdef0\n")).unwrap();
        let snapshot = scan_project(
            &registration(&repo),
            &CoreConfig::default(),
            SeverityProfile::lenient(),
        );
        assert!(
            snapshot
                .diagnostics
                .iter()
                .any(|item| item.code == DiagnosticCode::DuplicateTaskId)
        );

        let missing = scan_project(
            &registration(&dir.path().join("moved")),
            &CoreConfig::default(),
            SeverityProfile::lenient(),
        );
        assert_eq!(
            missing.diagnostics[0].code,
            DiagnosticCode::ProjectUnavailable
        );
    }
}
