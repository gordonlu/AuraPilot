use aurapilot_core::config::CoreConfig;
use aurapilot_core::model::TaskState;
use aurapilot_core::task_store::{
    CreateTaskInput, TransitionTaskInput, UpdateTaskInput, create_task, delete_task, locate_task,
    transition_task, update_task,
};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn phase_three_acceptance_manages_a_task_without_touching_git_history_or_source_files() {
    let sandbox = tempdir().unwrap();
    let repo = sandbox.path().join("真实 项目 with spaces");
    create_repository(&repo);
    let initial_head = git(&repo, &["rev-parse", "HEAD"]);

    let created = create_task(
        &repo,
        &CoreConfig::default(),
        CreateTaskInput {
            title: "生产看板".into(),
            priority: "P1".into(),
            task_type: "feature".into(),
            desc: Some("用户无需进入终端即可管理任务".into()),
            accept: vec!["创建、编辑、流转和删除均安全".into()],
        },
    )
    .unwrap();
    assert_eq!(created.document.id.as_deref(), Some("TASK-001"));

    transition_task(
        &repo,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::InProgress,
            assigned: Some("codex".into()),
            branch: Some("task/TASK-001".into()),
            ..TransitionTaskInput::default()
        },
    )
    .unwrap();
    let updated = update_task(
        &repo,
        "TASK-001",
        UpdateTaskInput {
            title: "生产级跨项目看板".into(),
            priority: "P0".into(),
            task_type: "feature".into(),
            desc: Some("保留 Agent 写入的运行字段".into()),
            accept: vec!["核心工作流通过".into()],
        },
    )
    .unwrap();
    assert_eq!(updated.document.assigned.as_deref(), Some("codex"));
    assert_eq!(updated.document.branch.as_deref(), Some("task/TASK-001"));

    transition_task(
        &repo,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::InReview,
            pr: Some(42),
            waiting: Some("human-review".into()),
            ..TransitionTaskInput::default()
        },
    )
    .unwrap();
    let done = transition_task(
        &repo,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::Done,
            commit: Some("abc1234".into()),
            ..TransitionTaskInput::default()
        },
    )
    .unwrap();
    assert_eq!(done.state, TaskState::Done);
    assert!(done.document.completed.is_some());

    delete_task(&repo, "TASK-001").unwrap();
    assert!(locate_task(&repo, "TASK-001").is_err());
    assert_eq!(
        fs::read_to_string(repo.join("src/main.txt")).unwrap(),
        "user source\n"
    );
    assert_eq!(git(&repo, &["rev-parse", "HEAD"]), initial_head);
    let changed = git(&repo, &["status", "--short"]);
    assert!(changed.lines().all(|line| line.contains(".aurapilot/")));
}

fn create_repository(repo: &Path) {
    fs::create_dir_all(repo.join("src")).unwrap();
    for state in TaskState::ALL {
        fs::create_dir_all(repo.join(".aurapilot/tasks").join(state.directory())).unwrap();
    }
    fs::write(repo.join("src/main.txt"), "user source\n").unwrap();
    fs::write(repo.join(".aurapilot/AGENTS.md"), "# Protocol\n").unwrap();
    fs::write(repo.join(".aurapilot/schema.json"), "{}\n").unwrap();
    fs::write(
        repo.join(".aurapilot/project.yaml"),
        "name: phase-three\nowner: tester\nhealth: green\nschema_version: 1\ncreated: 2026-07-28\n",
    )
    .unwrap();
    git(repo, &["init", "--quiet"]);
    git(repo, &["config", "user.email", "acceptance@example.com"]);
    git(repo, &["config", "user.name", "AuraPilot Acceptance"]);
    git(repo, &["add", "."]);
    git(repo, &["commit", "--quiet", "-m", "baseline"]);
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap();
    assert!(output.status.success(), "git command failed: {args:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}
