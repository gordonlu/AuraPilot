use aurapilot_core::app_paths::registry_path;
use aurapilot_core::config::CoreConfig;
use aurapilot_core::initializer::{InitOptions, InitStatus, initialize_repository};
use aurapilot_core::model::TaskState;
use aurapilot_core::project_registry::{ProjectRegistry, RegistryError};
use aurapilot_core::project_scanner::scan_project;
use aurapilot_core::validation::SeverityProfile;
use std::collections::VecDeque;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "AuraPilot local task control plane

Usage:
  aurapilot [--config PATH] init [PATH] [--owner NAME] [--ignore]
  aurapilot [--config PATH] add [PATH]
  aurapilot [--config PATH] status
  aurapilot --help

Commands:
  init      Initialize the .aurapilot protocol in a repository
  add       Register an initialized repository for desktop and CLI use
  status    List registered projects and task counts

Options:
  --config PATH  Override the local registry path
  --owner NAME   Project owner written by init (default: unknown)
  --ignore       Add .aurapilot/ to the repository .gitignore
  -h, --help     Show this help
  -V, --version  Show the version";

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let mut arguments = VecDeque::from(arguments);
    if matches!(
        arguments.front().and_then(|value| value.to_str()),
        Some("-h" | "--help")
    ) {
        println!("{HELP}");
        return Ok(());
    }
    if matches!(
        arguments.front().and_then(|value| value.to_str()),
        Some("-V" | "--version")
    ) {
        println!("aurapilot {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let config_override = take_global_config(&mut arguments)?;
    let command = arguments
        .pop_front()
        .and_then(|value| value.into_string().ok())
        .ok_or_else(|| format!("a command is required\n\n{HELP}"))?;
    let config = CoreConfig::default();
    let registry = config_override
        .or_else(|| env::var_os("AURAPILOT_CONFIG").map(PathBuf::from))
        .map(Ok)
        .unwrap_or_else(registry_path)
        .map_err(|error| error.to_string())?;
    match command.as_str() {
        "init" => command_init(arguments, &config),
        "add" => command_add(arguments, registry, &config),
        "status" => command_status(arguments, registry, &config),
        _ => Err(format!("unknown command `{command}`\n\n{HELP}")),
    }
}

fn take_global_config(arguments: &mut VecDeque<OsString>) -> Result<Option<PathBuf>, String> {
    if arguments.front().and_then(|value| value.to_str()) != Some("--config") {
        return Ok(None);
    }
    arguments.pop_front();
    arguments
        .pop_front()
        .map(PathBuf::from)
        .map(Some)
        .ok_or_else(|| "--config requires a path".into())
}

fn command_init(mut arguments: VecDeque<OsString>, config: &CoreConfig) -> Result<(), String> {
    let mut path = None;
    let mut owner = None;
    let mut add_to_gitignore = false;
    while let Some(argument) = arguments.pop_front() {
        match argument.to_str() {
            Some("--owner") => {
                owner = Some(
                    arguments
                        .pop_front()
                        .and_then(|value| value.into_string().ok())
                        .ok_or_else(|| "--owner requires UTF-8 text".to_string())?,
                );
            }
            Some("--ignore") => add_to_gitignore = true,
            Some(value) if value.starts_with('-') => {
                return Err(format!("unknown init option `{value}`"));
            }
            _ if path.is_none() => path = Some(PathBuf::from(argument)),
            _ => return Err("init accepts only one repository path".into()),
        }
    }
    let repository = path.unwrap_or_else(|| PathBuf::from("."));
    let report = initialize_repository(
        &repository,
        config,
        &InitOptions {
            owner,
            add_to_gitignore,
        },
    )
    .map_err(|error| error.to_string())?;
    let status = match report.status {
        InitStatus::Created => "initialized",
        InitStatus::Repaired => "repaired",
        InitStatus::AlreadyInitialized => "already initialized",
    };
    println!("AuraPilot {status}: {}", report.repository.display());
    println!("created: {}", report.created.len());
    println!("preserved: {}", report.preserved.len());
    println!(
        "git tracking: {}",
        if add_to_gitignore {
            "disabled by .gitignore"
        } else {
            "enabled (default)"
        }
    );
    println!(
        "next: configure one repository-level Agent instruction using the AuraPilot Bootstrap guide"
    );
    Ok(())
}

fn command_add(
    mut arguments: VecDeque<OsString>,
    registry_path: PathBuf,
    config: &CoreConfig,
) -> Result<(), String> {
    let path = match arguments.pop_front() {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from("."),
    };
    if arguments.pop_front().is_some() {
        return Err("add accepts only one repository path".into());
    }
    let mut registry =
        ProjectRegistry::load(registry_path, config.clone()).map_err(|error| error.to_string())?;
    match registry.add(&path) {
        Ok(project) => println!("registered {} ({})", project.path.display(), project.id),
        Err(RegistryError::Duplicate(path)) => {
            println!("already registered {}", path.display());
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok(())
}

fn command_status(
    arguments: VecDeque<OsString>,
    registry_path: PathBuf,
    config: &CoreConfig,
) -> Result<(), String> {
    if !arguments.is_empty() {
        return Err("status does not accept positional arguments".into());
    }
    let registry =
        ProjectRegistry::load(registry_path, config.clone()).map_err(|error| error.to_string())?;
    if registry.projects().is_empty() {
        println!("No projects registered. Run `aurapilot add [path]`.");
        return Ok(());
    }
    println!("PROJECT\tBACKLOG\tIN PROGRESS\tIN REVIEW\tDONE\tDIAGNOSTICS");
    for project in registry.projects() {
        let snapshot = scan_project(project, config, SeverityProfile::lenient());
        let count = |state| {
            snapshot
                .tasks
                .iter()
                .filter(|task| task.state == state)
                .count()
        };
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}",
            project.path.display(),
            count(TaskState::Backlog),
            count(TaskState::InProgress),
            count(TaskState::InReview),
            count(TaskState::Done),
            snapshot.diagnostics.len()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn accepts_paths_with_spaces_without_shell_or_string_joining() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo with spaces");
        fs::create_dir(&repo).unwrap();
        run(vec![
            "--config".into(),
            dir.path().join("config.json").into_os_string(),
            "init".into(),
            repo.clone().into_os_string(),
        ])
        .unwrap();
        assert!(repo.join(".aurapilot/AGENTS.md").is_file());
    }
}
