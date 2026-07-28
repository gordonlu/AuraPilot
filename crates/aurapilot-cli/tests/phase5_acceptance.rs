use aurapilot_core::agent_profile::{BUILTIN_OPENCODE_ID, built_in_profiles};
use aurapilot_core::config::CoreConfig;
use aurapilot_core::model::{TaskLogEntry, TaskState};
use aurapilot_core::parser::parse_task_file;
use aurapilot_core::pointer_prompt::build_pointer_prompt;
use aurapilot_core::task_store::{
    CreateTaskInput, TransitionTaskInput, create_task, transition_task,
};
use aurapilot_core::transaction::FileTransaction;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

const BOOTSTRAP_REFERENCE: &str = "<!-- aurapilot:start -->
## AuraPilot

本项目使用 AuraPilot 管理 AI Coding 任务。

处理项目任务前，必须读取：

- `.aurapilot/AGENTS.md`
- 用户指定的 `.aurapilot/tasks/` 任务文件

任务领取、执行、进度、阻塞、审核和完成流程，以 `.aurapilot/AGENTS.md` 为准。
<!-- aurapilot:end -->";

#[test]
fn phase_five_acceptance_runs_from_init_to_agent_claim_in_an_isolated_repository() {
    let sandbox = tempdir().unwrap();
    let repo = sandbox.path().join("真实 项目 with spaces");
    let registry = sandbox.path().join("local/config.json");
    fs::create_dir(&repo).unwrap();
    fs::create_dir(repo.join("src")).unwrap();
    fs::write(repo.join("src/lib.rs"), "pub fn untouched() {}\n").unwrap();
    fs::write(
        repo.join("AGENTS.md"),
        "# Existing instructions\n\nKeep this text.\n",
    )
    .unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );

    let cli = env!("CARGO_BIN_EXE_aurapilot");
    let init = Command::new(cli)
        .arg("--config")
        .arg(&registry)
        .arg("init")
        .arg(&repo)
        .arg("--owner")
        .arg("acceptance")
        .output()
        .unwrap();
    assert!(
        init.status.success(),
        "{}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(repo.join(".aurapilot/AGENTS.md").is_file());
    assert_eq!(
        fs::read_to_string(repo.join("src/lib.rs")).unwrap(),
        "pub fn untouched() {}\n"
    );

    install_bootstrap_reference(&repo.join("AGENTS.md"));
    install_bootstrap_reference(&repo.join("AGENTS.md"));
    let configured = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    assert!(configured.starts_with("# Existing instructions\n\nKeep this text.\n"));
    assert_eq!(configured.matches("<!-- aurapilot:start -->").count(), 1);
    assert_eq!(configured.matches("<!-- aurapilot:end -->").count(), 1);

    let add = Command::new(cli)
        .arg("--config")
        .arg(&registry)
        .arg("add")
        .arg(&repo)
        .output()
        .unwrap();
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );

    let config = CoreConfig::default();
    let created = create_task(
        &repo,
        &config,
        CreateTaskInput {
            title: "Validate the first user journey".into(),
            priority: "P0".into(),
            task_type: "test".into(),
            desc: Some("isolated acceptance task".into()),
            accept: vec!["agent claims through the protocol".into()],
        },
    )
    .unwrap();
    let pointer = build_pointer_prompt(&repo, &created).unwrap();
    let opencode = built_in_profiles()
        .into_iter()
        .find(|profile| profile.id == BUILTIN_OPENCODE_ID)
        .unwrap();
    let launch = opencode.prepare(&pointer, &config).unwrap();
    assert_eq!(launch.args[0], "--prompt");
    assert!(launch.args[1].contains("TASK-001"));

    let claimed = transition_task(
        &repo,
        "TASK-001",
        TransitionTaskInput {
            target: TaskState::InProgress,
            assigned: Some("OpenCode acceptance profile".into()),
            branch: Some("task/TASK-001".into()),
            ..TransitionTaskInput::default()
        },
    )
    .unwrap();
    let mut document = claimed.document;
    document.log.push(TaskLogEntry {
        ts: Some("2026-07-28T12:00:00Z".into()),
        msg: Some("Agent read the protocol and claimed the task".into()),
        extensions: BTreeMap::new(),
    });
    FileTransaction::new(&repo)
        .write(
            Path::new("tasks/in-progress/TASK-001.yaml"),
            serde_yaml::to_string(&document).unwrap().as_bytes(),
        )
        .unwrap();
    let claimed =
        parse_task_file(&repo.join(".aurapilot/tasks/in-progress/TASK-001.yaml")).unwrap();
    assert_eq!(claimed.state, TaskState::InProgress);
    assert_eq!(claimed.document.log.len(), 1);

    let status = Command::new(cli)
        .arg("--config")
        .arg(&registry)
        .arg("status")
        .output()
        .unwrap();
    assert!(status.status.success());
    let output = String::from_utf8(status.stdout).unwrap();
    assert!(output.contains("IN PROGRESS"));
    assert!(output.contains("\t0\t1\t0\t0\t0"), "{output}");

    let head = Command::new("git")
        .args(["rev-parse", "--verify", "HEAD"])
        .current_dir(&repo)
        .output()
        .unwrap();
    assert!(
        !head.status.success(),
        "AuraPilot and Bootstrap must not commit"
    );
}

fn install_bootstrap_reference(path: &Path) {
    let source = fs::read_to_string(path).unwrap_or_default();
    let start = source.find("<!-- aurapilot:start -->");
    let end = source.find("<!-- aurapilot:end -->");
    let next = match (start, end) {
        (Some(start), Some(end)) if end >= start => {
            let end = end + "<!-- aurapilot:end -->".len();
            format!(
                "{}{}{}",
                &source[..start],
                BOOTSTRAP_REFERENCE,
                &source[end..]
            )
        }
        _ => format!("{}\n{}\n", source.trim_end(), BOOTSTRAP_REFERENCE),
    };
    fs::write(path, next).unwrap();
}
