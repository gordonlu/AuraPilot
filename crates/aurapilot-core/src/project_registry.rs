use crate::config::CoreConfig;
use crate::path_security::canonical_repository_root;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegisteredProject {
    pub id: Uuid,
    pub path: PathBuf,
    pub registered_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_profile_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegistryDocument {
    pub version: u32,
    #[serde(default)]
    pub projects: Vec<RegisteredProject>,
}

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("project does not contain a .aurapilot directory: {0}")]
    ProtocolMissing(PathBuf),
    #[error("project is already registered: {0}")]
    Duplicate(PathBuf),
    #[error("registered project not found: {0}")]
    NotFound(Uuid),
    #[error("unsupported registry format version: {0}")]
    UnsupportedVersion(u32),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct ProjectRegistry {
    path: PathBuf,
    config: CoreConfig,
    document: RegistryDocument,
}

impl ProjectRegistry {
    pub fn load(path: impl Into<PathBuf>, config: CoreConfig) -> Result<Self, RegistryError> {
        let path = path.into();
        let document = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<RegistryDocument>(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => RegistryDocument {
                version: config.registry_format_version,
                projects: Vec::new(),
            },
            Err(error) => return Err(error.into()),
        };
        if document.version != config.registry_format_version {
            return Err(RegistryError::UnsupportedVersion(document.version));
        }
        Ok(Self {
            path,
            config,
            document,
        })
    }

    pub fn projects(&self) -> &[RegisteredProject] {
        &self.document.projects
    }

    pub fn add(&mut self, repo: &Path) -> Result<RegisteredProject, RegistryError> {
        let canonical =
            canonical_repository_root(repo).map_err(|error| io::Error::other(error.to_string()))?;
        if !canonical.join(".aurapilot").is_dir() {
            return Err(RegistryError::ProtocolMissing(canonical));
        }
        if self
            .document
            .projects
            .iter()
            .any(|project| project.path == canonical)
        {
            return Err(RegistryError::Duplicate(canonical));
        }
        let project = RegisteredProject {
            id: Uuid::new_v4(),
            path: canonical,
            registered_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
            last_profile_id: None,
        };
        let mut next = self.document.clone();
        next.projects.push(project.clone());
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(project)
    }

    pub fn remove(&mut self, id: Uuid) -> Result<RegisteredProject, RegistryError> {
        let index = self
            .document
            .projects
            .iter()
            .position(|project| project.id == id)
            .ok_or(RegistryError::NotFound(id))?;
        let mut next = self.document.clone();
        let removed = next.projects.remove(index);
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(removed)
    }

    pub fn reload(&mut self) -> Result<(), RegistryError> {
        *self = Self::load(self.path.clone(), self.config.clone())?;
        Ok(())
    }

    pub fn set_last_profile(
        &mut self,
        id: Uuid,
        profile_id: Option<String>,
    ) -> Result<RegisteredProject, RegistryError> {
        let mut next = self.document.clone();
        let project = next
            .projects
            .iter_mut()
            .find(|project| project.id == id)
            .ok_or(RegistryError::NotFound(id))?;
        project.last_profile_id = profile_id;
        let updated = project.clone();
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(updated)
    }
}

fn persist_document(path: &Path, document: &RegistryDocument) -> Result<(), RegistryError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "registry path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = path.with_extension(format!("tmp-{}-{nonce}", std::process::id()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        serde_json::to_writer_pretty(&mut file, document)?;
        file.write_all(b"\n")?;
        file.flush()?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        #[cfg(unix)]
        fs::File::open(parent)?.sync_all()?;
        Ok::<_, RegistryError>(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn persists_canonical_projects_and_prevents_duplicates() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir_all(repo.join(".aurapilot")).unwrap();
        let registry_path = dir.path().join("config/config.json");
        let mut registry = ProjectRegistry::load(&registry_path, CoreConfig::default()).unwrap();
        let added = registry.add(&repo).unwrap();
        assert!(matches!(
            registry.add(&repo),
            Err(RegistryError::Duplicate(_))
        ));

        let mut loaded = ProjectRegistry::load(&registry_path, CoreConfig::default()).unwrap();
        assert_eq!(loaded.projects(), std::slice::from_ref(&added));
        let updated = loaded
            .set_last_profile(added.id, Some("opencode-review".into()))
            .unwrap();
        assert_eq!(updated.last_profile_id.as_deref(), Some("opencode-review"));
        let reloaded = ProjectRegistry::load(&registry_path, CoreConfig::default()).unwrap();
        assert_eq!(
            reloaded.projects()[0].last_profile_id.as_deref(),
            Some("opencode-review")
        );
        assert_eq!(loaded.remove(added.id).unwrap(), updated);
        assert!(loaded.projects().is_empty());
    }

    #[test]
    fn rejects_directories_without_the_protocol() {
        let dir = tempdir().unwrap();
        let repo = dir.path().join("repo");
        fs::create_dir(&repo).unwrap();
        let mut registry =
            ProjectRegistry::load(dir.path().join("config.json"), CoreConfig::default()).unwrap();
        assert!(matches!(
            registry.add(&repo),
            Err(RegistryError::ProtocolMissing(_))
        ));
    }
}
