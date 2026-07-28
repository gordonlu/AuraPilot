use aurapilot_core::config::CoreConfig;
use aurapilot_core::model::TaskState;
use aurapilot_core::parser::{parse_project_str, parse_task_file};
use aurapilot_core::task_id::create_backlog_task;
use aurapilot_core::transaction::FileTransaction;
use aurapilot_core::validation::{SchemaValidator, SeverityProfile, StatePolicyValidator};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn phase_one_acceptance_reads_and_safely_modifies_a_real_git_repository() {
    let sandbox = tempdir().unwrap();
    let repo = sandbox.path().join("真实 repo with spaces");
    fs::create_dir(&repo).unwrap();
    let git = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(&repo)
        .status()
        .expect("git must be installed for the Phase 1 acceptance test");
    assert!(git.success());

    for state in TaskState::ALL {
        fs::create_dir_all(repo.join(".aurapilot/tasks").join(state.directory())).unwrap();
    }
    fs::write(
        repo.join(".aurapilot/project.yaml"),
        "name: acceptance-repo\nowner: tester\nhealth: green\nschema_version: 1\ncreated: 2026-07-28\n",
    )
    .unwrap();
    fs::write(
        repo.join(".aurapilot/tasks/backlog/TASK-001.yaml"),
        "id: TASK-001\ntitle: Verify protocol core\npriority: P0\ntype: test\ncreated: 2026-07-28\naccept:\n  - repository remains safe\nblockers: []\n",
    )
    .unwrap();

    let config = CoreConfig::default();
    let schema = SchemaValidator::new(SeverityProfile::strict());
    let project_source = fs::read_to_string(repo.join(".aurapilot/project.yaml")).unwrap();
    let project = parse_project_str(&project_source, &repo.join(".aurapilot/project.yaml"))
        .expect("project YAML must parse");
    assert!(schema.validate_project(&project, &config).is_empty());

    let backlog_path = repo.join(".aurapilot/tasks/backlog/TASK-001.yaml");
    let mut task = parse_task_file(&backlog_path).expect("task YAML must parse");
    assert!(schema.validate_task(&task.document).is_empty());
    assert!(StatePolicyValidator::validate(&task, SeverityProfile::strict()).is_empty());

    let (created_id, created_path) = create_backlog_task(&repo, &config, |id| {
        format!(
            "id: {id}\ntitle: Concurrent-safe creation\npriority: P1\ntype: test\ncreated: 2026-07-28\n"
        )
        .into_bytes()
    })
    .expect("task creation must hold the project lock");
    assert_eq!(created_id, "TASK-002");
    assert!(created_path.exists());

    task.document.assigned = Some("Acceptance Agent".into());
    task.document.branch = Some("test/phase-one".into());
    task.document.started = Some("2026-07-28T10:00:00+08:00".into());
    let updated = serde_yaml::to_string(&task.document).unwrap();
    FileTransaction::new(&repo)
        .move_with_content(
            Path::new("tasks/backlog/TASK-001.yaml"),
            Path::new("tasks/in-progress/TASK-001.yaml"),
            updated.as_bytes(),
        )
        .expect("state transition must be transactional");

    assert!(!backlog_path.exists());
    let moved_path = repo.join(".aurapilot/tasks/in-progress/TASK-001.yaml");
    let moved = parse_task_file(&moved_path).expect("moved task must remain readable");
    assert_eq!(moved.state, TaskState::InProgress);
    assert!(schema.validate_task(&moved.document).is_empty());
    assert!(StatePolicyValidator::validate(&moved, SeverityProfile::strict()).is_empty());

    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(status.status.success());
    let changed = String::from_utf8(status.stdout).unwrap();
    assert!(changed.lines().all(|line| line.contains(".aurapilot/")));

    let head = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        !head.status.success(),
        "AuraPilot must not create a Git commit"
    );
}
