use crate::config::CoreConfig;
use serde::Serialize;
use std::fs;
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::Instant;
use thiserror::Error;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct GitWorkspaceStatus {
    pub is_repository: bool,
    pub current_branch: Option<String>,
    pub dirty: bool,
    pub detail: String,
}

#[derive(Debug, Error)]
pub enum GitWorkspaceError {
    #[error("Git executable was not found")]
    ExecutableNotFound,
    #[error("Git operation timed out after {seconds} seconds; the child process was stopped")]
    Timeout { seconds: u64 },
    #[error("cannot create Git branch `{branch}`: {detail}")]
    CreateBranch { branch: String, detail: String },
    #[error("cannot inspect Git repository: {0}")]
    Inspection(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

struct GitOutput {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

pub fn inspect_repository(
    repository: &Path,
    config: &CoreConfig,
) -> Result<GitWorkspaceStatus, GitWorkspaceError> {
    let repository = fs::canonicalize(repository)?;
    let probe = run_git(&repository, &["rev-parse", "--is-inside-work-tree"], config)?;
    if !probe.status.success() || probe.stdout.trim() != "true" {
        return Ok(GitWorkspaceStatus {
            is_repository: false,
            current_branch: None,
            dirty: false,
            detail: clean_detail(&probe.stderr, "目录不是 Git 仓库"),
        });
    }
    let branch = run_git(
        &repository,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        config,
    )?;
    let current_branch = branch
        .status
        .success()
        .then(|| branch.stdout.trim().to_owned())
        .filter(|value| !value.is_empty());
    let status = run_git(
        &repository,
        &["status", "--porcelain", "--untracked-files=normal"],
        config,
    )?;
    if !status.status.success() {
        return Err(GitWorkspaceError::Inspection(clean_detail(
            &status.stderr,
            "git status failed",
        )));
    }
    let dirty = !status.stdout.trim().is_empty();
    Ok(GitWorkspaceStatus {
        is_repository: true,
        current_branch,
        dirty,
        detail: if dirty {
            "工作区有未提交变更；创建分支时这些变更会保留".into()
        } else {
            "Git 工作区干净".into()
        },
    })
}

pub fn create_and_checkout_branch(
    repository: &Path,
    branch: &str,
    config: &CoreConfig,
) -> Result<GitWorkspaceStatus, GitWorkspaceError> {
    let repository = fs::canonicalize(repository)?;
    let branch = branch.trim();
    if branch.is_empty() {
        return Err(GitWorkspaceError::CreateBranch {
            branch: branch.into(),
            detail: "branch name cannot be empty".into(),
        });
    }
    let validation = run_git(
        &repository,
        &["check-ref-format", "--branch", branch],
        config,
    )?;
    if !validation.status.success() || validation.stdout.trim() != branch {
        return Err(GitWorkspaceError::CreateBranch {
            branch: branch.into(),
            detail: clean_detail(&validation.stderr, "invalid Git branch name"),
        });
    }
    let current = run_git(
        &repository,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        config,
    )?;
    let full_ref = format!("refs/heads/{branch}");
    let existing = run_git(
        &repository,
        &["show-ref", "--verify", "--quiet", &full_ref],
        config,
    )?;
    if current.stdout.trim() == branch || existing.status.success() {
        return Err(GitWorkspaceError::CreateBranch {
            branch: branch.into(),
            detail: "a branch with that name already exists".into(),
        });
    }
    let checkout = run_git(&repository, &["checkout", "-b", branch], config)?;
    if !checkout.status.success() {
        return Err(GitWorkspaceError::CreateBranch {
            branch: branch.into(),
            detail: clean_detail(&checkout.stderr, "git checkout failed"),
        });
    }
    inspect_repository(&repository, config)
}

fn run_git(
    repository: &Path,
    arguments: &[&str],
    config: &CoreConfig,
) -> Result<GitOutput, GitWorkspaceError> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                GitWorkspaceError::ExecutableNotFound
            } else {
                GitWorkspaceError::Io(error)
            }
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("Git stdout pipe was unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| io::Error::other("Git stderr pipe was unavailable"))?;
    let output_limit = config.git_error_output_limit_bytes;
    let stdout_reader = thread::spawn(move || read_limited(stdout, output_limit));
    let stderr_reader = thread::spawn(move || read_limited(stderr, output_limit));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() >= config.git_command_timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitWorkspaceError::Timeout {
                seconds: config.git_command_timeout.as_secs(),
            });
        }
        thread::sleep(config.git_poll_interval);
    };
    let stdout = stdout_reader
        .join()
        .map_err(|_| io::Error::other("Git stdout reader panicked"))??;
    let stderr = stderr_reader
        .join()
        .map_err(|_| io::Error::other("Git stderr reader panicked"))??;
    Ok(GitOutput {
        status,
        stdout,
        stderr,
    })
}

fn read_limited(mut reader: impl Read, limit: usize) -> io::Result<String> {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 4_096];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let remaining = limit.saturating_sub(captured.len());
        captured.extend_from_slice(&buffer[..read.min(remaining)]);
    }
    Ok(String::from_utf8_lossy(&captured).into_owned())
}

fn clean_detail(stderr: &str, fallback: &str) -> String {
    let detail = stderr.trim();
    if detail.is_empty() {
        fallback.into()
    } else {
        detail.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn git(repository: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(arguments)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn inspects_and_creates_a_branch_without_discarding_changes() {
        let temp = tempdir().unwrap();
        git(temp.path(), &["init", "-q"]);
        git(temp.path(), &["config", "user.name", "AuraPilot Test"]);
        git(
            temp.path(),
            &["config", "user.email", "test@example.invalid"],
        );
        fs::write(temp.path().join("tracked.txt"), "initial").unwrap();
        git(temp.path(), &["add", "tracked.txt"]);
        git(temp.path(), &["commit", "-qm", "initial"]);
        fs::write(temp.path().join("tracked.txt"), "changed").unwrap();

        let config = CoreConfig::default();
        let before = inspect_repository(temp.path(), &config).unwrap();
        assert!(before.is_repository);
        assert!(before.dirty);
        let after = create_and_checkout_branch(temp.path(), "task/TASK-001", &config).unwrap();
        assert_eq!(after.current_branch.as_deref(), Some("task/TASK-001"));
        assert!(after.dirty);
        assert_eq!(
            fs::read_to_string(temp.path().join("tracked.txt")).unwrap(),
            "changed"
        );
    }

    #[test]
    fn rejects_invalid_and_existing_branch_names_without_switching() {
        let temp = tempdir().unwrap();
        git(temp.path(), &["init", "-q"]);
        let config = CoreConfig::default();
        assert!(matches!(
            create_and_checkout_branch(temp.path(), "bad name", &config),
            Err(GitWorkspaceError::CreateBranch { .. })
        ));
        create_and_checkout_branch(temp.path(), "task/first", &config).unwrap();
        assert!(matches!(
            create_and_checkout_branch(temp.path(), "task/first", &config),
            Err(GitWorkspaceError::CreateBranch { .. })
        ));
        assert_eq!(
            inspect_repository(temp.path(), &config)
                .unwrap()
                .current_branch
                .as_deref(),
            Some("task/first")
        );
    }
}
