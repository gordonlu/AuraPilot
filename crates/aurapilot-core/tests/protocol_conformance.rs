mod support;

use aurapilot_core::agent_profile::{BUILTIN_CODEX_ID, BUILTIN_OPENCODE_ID, built_in_profiles};
use aurapilot_core::config::CoreConfig;
use aurapilot_core::model::TaskState;
use aurapilot_core::parser::parse_task_file;
use aurapilot_core::pointer_prompt::build_pointer_prompt;
use aurapilot_core::push_attempt::{PushAttemptStatus, PushAttemptStore, PushDelivery};
use aurapilot_core::task_store::{
    TaskStoreError, TransitionTaskInput, locate_task, transition_task,
};
use std::fs;
use std::path::PathBuf;
use tempfile::{TempDir, tempdir};
use uuid::Uuid;

use support::{CompatibilityCase, EvidenceLevel, assert_compatible};

const PROVIDERS: [&str; 2] = [BUILTIN_CODEX_ID, BUILTIN_OPENCODE_ID];

struct ProtocolFixture {
    _sandbox: TempDir,
    repository: PathBuf,
    task_path: PathBuf,
    attempts_path: PathBuf,
}

impl ProtocolFixture {
    fn new() -> Self {
        let sandbox = tempdir().unwrap();
        let repository = sandbox.path().join("repository with spaces");
        let aura = repository.join(".aurapilot");
        for state in TaskState::ALL {
            fs::create_dir_all(aura.join("tasks").join(state.directory())).unwrap();
        }
        fs::write(aura.join("AGENTS.md"), "# Test protocol\n").unwrap();
        fs::write(
            aura.join("project.yaml"),
            "name: compatibility-fixture\nowner: test\nhealth: green\nschema_version: 1\ncreated: 2026-08-11\n",
        )
        .unwrap();
        let task_path = aura.join("tasks/backlog/TASK-001.yaml");
        fs::write(
            &task_path,
            concat!(
                "id: TASK-001\n",
                "title: Verify protocol conformance\n",
                "priority: P0\n",
                "type: test\n",
                "created: 2026-08-11\n",
                "accept:\n  - protocol invariants remain true\n",
                "blockers: []\n",
            ),
        )
        .unwrap();
        let attempts_path = sandbox.path().join("local/push-attempts.json");
        Self {
            _sandbox: sandbox,
            repository,
            task_path,
            attempts_path,
        }
    }

    fn task_bytes(&self) -> Vec<u8> {
        fs::read(&self.task_path).unwrap()
    }

    fn state_path(&self, state: TaskState) -> PathBuf {
        self.repository
            .join(".aurapilot/tasks")
            .join(state.directory())
            .join("TASK-001.yaml")
    }
}

#[test]
fn provider_matrix_prepares_push_without_claiming_the_task() {
    for provider in PROVIDERS {
        let fixture = ProtocolFixture::new();
        let before = fixture.task_bytes();
        let task = parse_task_file(&fixture.task_path).unwrap();
        let pointer = build_pointer_prompt(&fixture.repository, &task).unwrap();
        let profile = built_in_profiles()
            .into_iter()
            .find(|profile| profile.id == provider)
            .unwrap();
        let launch = profile.prepare(&pointer, &CoreConfig::default()).unwrap();

        let mut attempts =
            PushAttemptStore::load(&fixture.attempts_path, CoreConfig::default()).unwrap();
        let attempt = attempts
            .requested(Uuid::new_v4(), "TASK-001", provider)
            .unwrap();

        let mut case = CompatibilityCase::new(
            "push-prepared-without-claim",
            provider,
            EvidenceLevel::DeterministicProtocol,
        );
        case.required(
            attempt.status == PushAttemptStatus::Requested,
            "PushAttempt created",
        );
        case.required(
            launch.profile_id == provider,
            "selected Provider Profile prepared",
        );
        case.required(
            launch.prompt.contains("TASK-001")
                && launch.args.iter().any(|arg| arg.contains("TASK-001")),
            "Pointer Prompt transported to the prepared launch",
        );
        case.required(
            launch.working_directory == fixture.repository,
            "repository selected as working directory",
        );
        case.forbidden(
            fixture.task_bytes() != before,
            "Push preparation changed task YAML",
        );
        case.forbidden(
            fixture.state_path(TaskState::InProgress).exists(),
            "Push preparation claimed the task",
        );
        case.forbidden(
            locate_task(&fixture.repository, "TASK-001")
                .unwrap()
                .document
                .assigned
                .is_some(),
            "AuraPilot assigned the task during Push",
        );
        assert_compatible(case.finish());
    }
}

#[test]
fn missing_executable_is_visible_and_does_not_change_task_truth() {
    for provider in PROVIDERS {
        let fixture = ProtocolFixture::new();
        let before = fixture.task_bytes();
        let missing = fixture
            .repository
            .join("definitely-not-an-agent-executable");
        let launch_error = std::process::Command::new(&missing)
            .current_dir(&fixture.repository)
            .status()
            .unwrap_err();
        let mut attempts =
            PushAttemptStore::load(&fixture.attempts_path, CoreConfig::default()).unwrap();
        let requested = attempts
            .requested(Uuid::new_v4(), "TASK-001", provider)
            .unwrap();
        let failed = attempts
            .update(
                requested.id,
                PushAttemptStatus::FailedToStart,
                None,
                Some(format!("failed to launch {provider}: {launch_error}")),
                PushDelivery::Process,
            )
            .unwrap();

        let mut case = CompatibilityCase::new(
            "missing-executable",
            provider,
            EvidenceLevel::SimulatedAdapter,
        );
        case.required(
            failed.status == PushAttemptStatus::FailedToStart,
            "failed-to-start lifecycle recorded",
        );
        case.required(
            failed
                .error
                .as_deref()
                .is_some_and(|error| error.contains(provider) && !error.trim().is_empty()),
            "diagnostic preserves Provider and launch error context",
        );
        case.forbidden(
            fixture.task_bytes() != before,
            "launch failure changed task YAML",
        );
        case.forbidden(
            fixture.state_path(TaskState::InProgress).exists(),
            "launch failure claimed the task",
        );
        assert_compatible(case.finish());
    }
}

#[test]
fn successful_process_exit_or_agent_done_text_is_not_task_completion() {
    for provider in PROVIDERS {
        let fixture = ProtocolFixture::new();
        let before = fixture.task_bytes();
        let transcript = fixture
            .repository
            .join(format!("{provider}-execution-output.txt"));
        fs::write(&transcript, "Agent says: done\n").unwrap();
        let mut attempts =
            PushAttemptStore::load(&fixture.attempts_path, CoreConfig::default()).unwrap();
        let requested = attempts
            .requested(Uuid::new_v4(), "TASK-001", provider)
            .unwrap();
        attempts
            .update(
                requested.id,
                PushAttemptStatus::Started,
                Some(42),
                None,
                PushDelivery::Process,
            )
            .unwrap();
        let exited = attempts
            .update(
                requested.id,
                PushAttemptStatus::Exited,
                None,
                None,
                PushDelivery::Process,
            )
            .unwrap();

        let located = locate_task(&fixture.repository, "TASK-001").unwrap();
        let mut case = CompatibilityCase::new(
            "exit-and-self-report-are-not-completion",
            provider,
            EvidenceLevel::SimulatedAdapter,
        );
        case.required(
            exited.status == PushAttemptStatus::Exited && exited.error.is_none(),
            "successful process exit recorded independently",
        );
        case.required(
            fs::read_to_string(&transcript).unwrap().contains("done"),
            "Agent completion claim present in execution evidence",
        );
        case.forbidden(
            fixture.task_bytes() != before,
            "execution evidence changed task YAML",
        );
        case.forbidden(
            located.state == TaskState::Done || located.document.completed.is_some(),
            "process exit or Agent text completed the task",
        );
        assert_compatible(case.finish());
    }
}

#[test]
fn legal_task_update_is_accepted_and_invalid_direct_completion_is_rejected() {
    let legal = ProtocolFixture::new();
    let claimed = transition_task(
        &legal.repository,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::InProgress,
            assigned: Some("Compatibility Agent".into()),
            branch: Some("task/TASK-001".into()),
            ..TransitionTaskInput::default()
        },
    )
    .unwrap();
    let mut accepted = CompatibilityCase::new(
        "legal-task-update",
        "protocol",
        EvidenceLevel::DeterministicProtocol,
    );
    accepted.required(
        claimed.state == TaskState::InProgress,
        "legal claim persisted",
    );
    accepted.required(
        claimed.document.assigned.as_deref() == Some("Compatibility Agent"),
        "required claim metadata persisted",
    );
    accepted.forbidden(
        legal.state_path(TaskState::Backlog).exists(),
        "transaction left a duplicate backlog task",
    );
    assert_compatible(accepted.finish());

    let invalid = ProtocolFixture::new();
    let before = invalid.task_bytes();
    let result = transition_task(
        &invalid.repository,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::Done,
            ..TransitionTaskInput::default()
        },
    );
    let mut rejected = CompatibilityCase::new(
        "illegal-state-transition",
        "protocol",
        EvidenceLevel::DeterministicProtocol,
    );
    rejected.required(
        matches!(result, Err(TaskStoreError::Invalid(_))),
        "invalid direct completion rejected by policy validation",
    );
    rejected.forbidden(
        invalid.task_bytes() != before,
        "rejected transition changed task YAML",
    );
    rejected.forbidden(
        invalid.state_path(TaskState::Done).exists(),
        "rejected transition created a done task",
    );
    assert_compatible(rejected.finish());
}

#[test]
fn malformed_and_duplicate_tasks_fail_deterministically_without_mutation() {
    let malformed = ProtocolFixture::new();
    fs::write(&malformed.task_path, "id: [not valid yaml\n").unwrap();
    let malformed_before = malformed.task_bytes();
    let parse_result = parse_task_file(&malformed.task_path);
    let mut malformed_case = CompatibilityCase::new(
        "malformed-task-yaml",
        "protocol",
        EvidenceLevel::DeterministicProtocol,
    );
    malformed_case.required(parse_result.is_err(), "malformed YAML rejected");
    malformed_case.forbidden(
        malformed.task_bytes() != malformed_before,
        "parser rewrote malformed task YAML",
    );
    assert_compatible(malformed_case.finish());

    let duplicate = ProtocolFixture::new();
    let duplicate_path = duplicate.state_path(TaskState::InProgress);
    fs::copy(&duplicate.task_path, &duplicate_path).unwrap();
    let backlog_before = duplicate.task_bytes();
    let in_progress_before = fs::read(&duplicate_path).unwrap();
    let locate_result = locate_task(&duplicate.repository, "TASK-001");
    let mut duplicate_case = CompatibilityCase::new(
        "duplicate-task-state",
        "protocol",
        EvidenceLevel::DeterministicProtocol,
    );
    duplicate_case.required(
        matches!(locate_result, Err(TaskStoreError::Duplicate(id)) if id == "TASK-001"),
        "duplicate task state rejected",
    );
    duplicate_case.forbidden(
        duplicate.task_bytes() != backlog_before
            || fs::read(&duplicate_path).unwrap() != in_progress_before,
        "duplicate detection modified either task file",
    );
    assert_compatible(duplicate_case.finish());
}
