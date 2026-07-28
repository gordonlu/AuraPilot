use aurapilot_core::config::CoreConfig;
use aurapilot_core::model::TaskState;
use aurapilot_core::project_registry::ProjectRegistry;
use aurapilot_core::project_scanner::scan_projects;
use aurapilot_core::validation::SeverityProfile;
use aurapilot_core::watcher::ProjectWatchService;
use std::collections::BTreeSet;
use std::fs;
use std::process::Command;
use std::sync::mpsc;
use std::time::Instant;
use tempfile::tempdir;

#[test]
fn phase_two_acceptance_registers_scans_and_watches_five_repositories() {
    let sandbox = tempdir().unwrap();
    let config = CoreConfig::default();
    let mut registry = ProjectRegistry::load(
        sandbox.path().join("local-config/config.json"),
        config.clone(),
    )
    .unwrap();

    for index in 1..=5 {
        let repo = sandbox.path().join(format!("项目 {index} with spaces"));
        create_protocol_repository(&repo, index);
        registry.add(&repo).unwrap();
    }
    assert_eq!(registry.projects().len(), 5);

    let snapshots = scan_projects(registry.projects(), &config, SeverityProfile::lenient());
    assert_eq!(snapshots.len(), 5);
    assert!(snapshots.iter().all(|snapshot| snapshot.tasks.len() == 1));
    assert!(snapshots.iter().all(|snapshot| !snapshot.has_errors()));

    let (tx, rx) = mpsc::channel();
    let mut watchers = ProjectWatchService::new(&config, move |change| {
        tx.send(change).unwrap();
    })
    .unwrap();
    for project in registry.projects() {
        watchers.watch_project(project).unwrap();
    }

    for (index, project) in registry.projects().iter().enumerate() {
        fs::write(
            project
                .path
                .join(format!(".aurapilot/tasks/backlog/TASK-{:03}.yaml", index + 1)),
            format!(
                "id: TASK-{:03}\ntitle: Changed externally\npriority: P1\ntype: test\ncreated: 2026-07-28\n",
                index + 1
            ),
        )
        .unwrap();
    }

    let started = Instant::now();
    let mut changed_projects = BTreeSet::new();
    while changed_projects.len() < 5 {
        let remaining = config
            .watcher_delivery_timeout
            .checked_sub(started.elapsed())
            .expect("all watcher events must arrive within two seconds");
        let change = rx.recv_timeout(remaining).unwrap();
        changed_projects.insert(change.project_id);
    }
    assert_eq!(changed_projects.len(), 5);
}

fn create_protocol_repository(repo: &std::path::Path, index: usize) {
    fs::create_dir_all(repo).unwrap();
    let status = Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo)
        .status()
        .unwrap();
    assert!(status.success());
    for state in TaskState::ALL {
        fs::create_dir_all(repo.join(".aurapilot/tasks").join(state.directory())).unwrap();
    }
    fs::write(repo.join(".aurapilot/AGENTS.md"), "# Protocol\n").unwrap();
    fs::write(repo.join(".aurapilot/schema.json"), "{}\n").unwrap();
    fs::write(
        repo.join(".aurapilot/project.yaml"),
        format!("name: project-{index}\nowner: tester\nhealth: green\nschema_version: 1\ncreated: 2026-07-28\n"),
    )
    .unwrap();
    fs::write(
        repo.join(format!(
            ".aurapilot/tasks/backlog/TASK-{index:03}.yaml"
        )),
        format!("id: TASK-{index:03}\ntitle: Initial task\npriority: P1\ntype: test\ncreated: 2026-07-28\n"),
    )
    .unwrap();
}
