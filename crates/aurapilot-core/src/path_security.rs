use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PathSecurityError {
    #[error("path escapes the repository: {0}")]
    Escape(PathBuf),
    #[error("invalid path component in: {0}")]
    Invalid(PathBuf),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub fn canonical_repository_root(repo: &Path) -> Result<PathBuf, PathSecurityError> {
    Ok(fs::canonicalize(repo)?)
}

pub fn resolve_within_repository(
    repo: &Path,
    candidate: &Path,
) -> Result<PathBuf, PathSecurityError> {
    let canonical_root = canonical_repository_root(repo)?;
    let absolute = if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        repo.join(candidate)
    };
    if absolute
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(PathSecurityError::Invalid(absolute));
    }

    let mut existing = absolute.as_path();
    let mut suffix = Vec::new();
    while !existing.exists() {
        let name = existing
            .file_name()
            .ok_or_else(|| PathSecurityError::Invalid(absolute.clone()))?;
        suffix.push(name.to_os_string());
        existing = existing
            .parent()
            .ok_or_else(|| PathSecurityError::Invalid(absolute.clone()))?;
    }
    let mut resolved = fs::canonicalize(existing)?;
    for part in suffix.iter().rev() {
        resolved.push(part);
    }
    if !resolved.starts_with(&canonical_root) {
        return Err(PathSecurityError::Escape(resolved));
    }
    Ok(resolved)
}

pub fn resolve_aurapilot_path(repo: &Path, relative: &Path) -> Result<PathBuf, PathSecurityError> {
    let requested = repo.join(".aurapilot").join(relative);
    resolve_within_repository(repo, &requested)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_parent_traversal() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join(".aurapilot")).unwrap();
        assert!(matches!(
            resolve_aurapilot_path(dir.path(), Path::new("../secret")),
            Err(PathSecurityError::Invalid(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn allows_symlink_repository_root_but_rejects_inner_escape() {
        use std::os::unix::fs::symlink;
        let parent = tempdir().unwrap();
        let real = parent.path().join("real-repo");
        let link = parent.path().join("repo-link");
        let outside = parent.path().join("outside");
        fs::create_dir_all(real.join(".aurapilot/tasks/backlog")).unwrap();
        fs::create_dir(&outside).unwrap();
        symlink(&real, &link).unwrap();
        symlink(&outside, real.join(".aurapilot/escape")).unwrap();

        let valid =
            resolve_aurapilot_path(&link, Path::new("tasks/backlog/TASK-001.yaml")).unwrap();
        assert!(valid.starts_with(fs::canonicalize(&real).unwrap()));
        assert!(matches!(
            resolve_aurapilot_path(&link, Path::new("escape/file.yaml")),
            Err(PathSecurityError::Escape(_))
        ));
    }
}
