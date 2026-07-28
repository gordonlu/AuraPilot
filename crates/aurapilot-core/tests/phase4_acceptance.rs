use aurapilot_core::agent_profile::{
    BUILTIN_CLAUDE_ID, BUILTIN_CODEX_ID, BUILTIN_OPENCODE_ID, built_in_profiles,
};
use aurapilot_core::config::CoreConfig;
use aurapilot_core::parser::parse_task_file;
use aurapilot_core::pointer_prompt::build_pointer_prompt;
use aurapilot_core::push_attempt::{PushAttemptStatus, PushAttemptStore};
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

#[test]
fn phase_four_acceptance_prepares_three_agents_and_failures_never_modify_task_state() {
    let dir = tempdir().unwrap();
    let repo = dir.path().join("repo with spaces");
    let backlog = repo.join(".aurapilot/tasks/backlog");
    fs::create_dir_all(&backlog).unwrap();
    fs::write(repo.join(".aurapilot/AGENTS.md"), "# Agent protocol").unwrap();
    let task_path = backlog.join("TASK-025.yaml");
    fs::write(
        &task_path,
        "id: TASK-025\ntitle: Push me\npriority: P1\ntype: feature\ncreated: 2026-07-28\n",
    )
    .unwrap();
    let before = fs::read(&task_path).unwrap();
    let task = parse_task_file(&task_path).unwrap();
    let pointer = build_pointer_prompt(&repo, &task).unwrap();
    let profiles = built_in_profiles();
    let selected = [BUILTIN_CODEX_ID, BUILTIN_CLAUDE_ID, BUILTIN_OPENCODE_ID];
    let prepared = selected
        .iter()
        .map(|id| {
            profiles
                .iter()
                .find(|profile| profile.id == *id)
                .unwrap()
                .prepare(&pointer, &CoreConfig::default())
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(prepared.len(), 3);
    assert!(
        prepared
            .iter()
            .all(|launch| launch.args.iter().any(|arg| arg.contains("TASK-025")))
    );
    assert_eq!(
        prepared
            .iter()
            .find(|launch| launch.profile_id == "opencode")
            .unwrap()
            .args[0],
        "--prompt"
    );

    let project_id = Uuid::new_v4();
    let attempts_path = dir.path().join("local/push-attempts.json");
    let mut attempts = PushAttemptStore::load(&attempts_path, CoreConfig::default()).unwrap();
    for launch in prepared {
        let requested = attempts
            .requested(project_id, "TASK-025", launch.profile_id)
            .unwrap();
        attempts
            .update(
                requested.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some("fake launcher failure".into()),
                aurapilot_core::push_attempt::PushDelivery::Process,
            )
            .unwrap();
    }

    assert_eq!(attempts.attempts().len(), 3);
    assert!(
        attempts
            .attempts()
            .iter()
            .all(|attempt| attempt.status == PushAttemptStatus::FailedToStart)
    );
    assert_eq!(fs::read(&task_path).unwrap(), before);
    assert!(
        !repo
            .join(".aurapilot/tasks/in-progress/TASK-025.yaml")
            .exists()
    );
}
