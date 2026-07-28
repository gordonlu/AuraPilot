use crate::config::CoreConfig;
use crate::model::{ProjectDocument, TaskState};
use crate::path_security::{
    PathSecurityError, canonical_repository_root, resolve_within_repository,
};
use crate::transaction::{FileTransaction, TransactionError};
use chrono::{SecondsFormat, Utc};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

const AGENT_PROTOCOL: &str = include_str!("../assets/protocol/AGENTS.md");
const TASK_SCHEMA: &str = include_str!("../assets/protocol/schema.json");
const GITIGNORE_ENTRY: &str = ".aurapilot/";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InitOptions {
    pub owner: Option<String>,
    pub add_to_gitignore: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitStatus {
    Created,
    Repaired,
    AlreadyInitialized,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InitReport {
    pub repository: PathBuf,
    pub status: InitStatus,
    pub created: Vec<PathBuf>,
    pub preserved: Vec<PathBuf>,
    pub gitignore_modified: bool,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("initialization target is not a directory: {0}")]
    NotDirectory(PathBuf),
    #[error("owner cannot be empty")]
    EmptyOwner,
    #[error(transparent)]
    Path(#[from] PathSecurityError),
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Serialize)]
struct InstallationDocument {
    protocol_version: u32,
    installed_at: String,
    updated_at: String,
    configured_files: Vec<String>,
    agent_detected: String,
    mode: String,
    status: String,
}

pub fn initialize_repository(
    repository: &Path,
    config: &CoreConfig,
    options: &InitOptions,
) -> Result<InitReport, InitError> {
    if !repository.is_dir() {
        return Err(InitError::NotDirectory(repository.to_path_buf()));
    }
    if options.owner.as_deref().is_some_and(str::is_empty) {
        return Err(InitError::EmptyOwner);
    }
    let repository = canonical_repository_root(repository)?;
    let aura = resolve_within_repository(&repository, Path::new(".aurapilot"))?;
    let existed_before = aura.exists();
    fs::create_dir_all(&aura)?;

    let mut created = Vec::new();
    let mut preserved = Vec::new();
    for state in TaskState::ALL {
        let relative = PathBuf::from(".aurapilot/tasks").join(state.directory());
        let path = resolve_within_repository(&repository, &relative)?;
        if path.exists() {
            if !path.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "protocol directory path is not a directory: {}",
                        path.display()
                    ),
                )
                .into());
            }
            preserved.push(relative);
        } else {
            fs::create_dir_all(&path)?;
            created.push(relative);
        }
    }

    let transaction = FileTransaction::new(&repository);
    write_if_missing(
        &transaction,
        Path::new("AGENTS.md"),
        AGENT_PROTOCOL.as_bytes(),
        &mut created,
        &mut preserved,
    )?;
    write_if_missing(
        &transaction,
        Path::new("schema.json"),
        TASK_SCHEMA.as_bytes(),
        &mut created,
        &mut preserved,
    )?;

    let project = ProjectDocument {
        name: Some(project_name(&repository)),
        owner: Some(options.owner.clone().unwrap_or_else(|| "unknown".into())),
        health: Some("green".into()),
        schema_version: Some(config.supported_schema_version),
        created: Some(Utc::now().date_naive().to_string()),
        extensions: BTreeMap::new(),
        ..ProjectDocument::default()
    };
    let project_yaml = serde_yaml::to_string(&project)?;
    write_if_missing(
        &transaction,
        Path::new("project.yaml"),
        project_yaml.as_bytes(),
        &mut created,
        &mut preserved,
    )?;

    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let installation = InstallationDocument {
        protocol_version: config.supported_schema_version,
        installed_at: now.clone(),
        updated_at: now,
        configured_files: Vec::new(),
        agent_detected: "unknown".into(),
        mode: "repository".into(),
        status: "protocol_initialized".into(),
    };
    let installation_yaml = serde_yaml::to_string(&installation)?;
    write_if_missing(
        &transaction,
        Path::new("installation.yaml"),
        installation_yaml.as_bytes(),
        &mut created,
        &mut preserved,
    )?;

    let gitignore_modified = if options.add_to_gitignore {
        ensure_gitignore_entry(&repository)?
    } else {
        false
    };
    let status = if !existed_before {
        InitStatus::Created
    } else if created.is_empty() && !gitignore_modified {
        InitStatus::AlreadyInitialized
    } else {
        InitStatus::Repaired
    };
    Ok(InitReport {
        repository,
        status,
        created,
        preserved,
        gitignore_modified,
    })
}

fn write_if_missing(
    transaction: &FileTransaction<'_>,
    relative: &Path,
    content: &[u8],
    created: &mut Vec<PathBuf>,
    preserved: &mut Vec<PathBuf>,
) -> Result<(), InitError> {
    let display = PathBuf::from(".aurapilot").join(relative);
    match transaction.write_new(relative, content) {
        Ok(_) => created.push(display),
        Err(TransactionError::DestinationExists(_)) => preserved.push(display),
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

fn ensure_gitignore_entry(repository: &Path) -> Result<bool, InitError> {
    let path = resolve_within_repository(repository, Path::new(".gitignore"))?;
    let original = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(error.into()),
    };
    if original.lines().any(|line| line.trim() == GITIGNORE_ENTRY) {
        return Ok(false);
    }
    let newline = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut next = original;
    if !next.is_empty() && !next.ends_with(['\n', '\r']) {
        next.push_str(newline);
    }
    next.push_str(GITIGNORE_ENTRY);
    next.push_str(newline);
    atomic_write(&path, next.as_bytes())?;
    Ok(true)
}

fn atomic_write(path: &Path, content: &[u8]) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("aurapilot-tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(content)?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn project_name(repository: &Path) -> String {
    repository
        .file_name()
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn initializes_idempotently_and_preserves_existing_files() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("项目 with spaces");
        fs::create_dir(&repo).unwrap();
        fs::write(repo.join(".gitignore"), "target\r\n").unwrap();
        let options = InitOptions {
            owner: Some("tester".into()),
            add_to_gitignore: true,
        };
        let first = initialize_repository(&repo, &CoreConfig::default(), &options).unwrap();
        assert_eq!(first.status, InitStatus::Created);
        assert!(first.gitignore_modified);
        assert!(repo.join(".aurapilot/schema.json").is_file());
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(repo.join(".aurapilot/schema.json")).unwrap(),
        )
        .unwrap();
        let protocol = repo.join(".aurapilot/AGENTS.md");
        fs::write(&protocol, "custom protocol").unwrap();
        let second = initialize_repository(&repo, &CoreConfig::default(), &options).unwrap();
        assert_eq!(second.status, InitStatus::AlreadyInitialized);
        assert_eq!(fs::read_to_string(protocol).unwrap(), "custom protocol");
        assert_eq!(
            fs::read_to_string(repo.join(".gitignore")).unwrap(),
            "target\r\n.aurapilot/\r\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_an_aurapilot_symlink_that_escapes_the_repository() {
        use std::os::unix::fs::symlink;
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        let outside = dir.path().join("outside");
        fs::create_dir(&repo).unwrap();
        fs::create_dir(&outside).unwrap();
        symlink(&outside, repo.join(".aurapilot")).unwrap();
        let result = initialize_repository(&repo, &CoreConfig::default(), &InitOptions::default());
        assert!(matches!(
            result,
            Err(InitError::Path(PathSecurityError::Escape(_)))
        ));
        assert!(fs::read_dir(outside).unwrap().next().is_none());
    }
}
