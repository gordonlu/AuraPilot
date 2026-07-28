use crate::config::CoreConfig;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushAttemptStatus {
    Requested,
    Started,
    FailedToStart,
    Exited,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushDelivery {
    Process,
    Clipboard,
    ClipboardFallback,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PushAttempt {
    pub id: Uuid,
    pub task_id: String,
    pub project_id: Uuid,
    pub agent_profile_id: String,
    pub created_at: String,
    pub status: PushAttemptStatus,
    pub process_id: Option<u32>,
    pub error: Option<String>,
    pub delivery: PushDelivery,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PushAttemptDocument {
    pub version: u32,
    #[serde(default)]
    pub attempts: Vec<PushAttempt>,
}

#[derive(Debug, Error)]
pub enum PushAttemptError {
    #[error("push attempt not found: {0}")]
    NotFound(Uuid),
    #[error("unsupported push attempt format version: {0}")]
    UnsupportedVersion(u32),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

pub struct PushAttemptStore {
    path: PathBuf,
    config: CoreConfig,
    document: PushAttemptDocument,
}

impl PushAttemptStore {
    pub fn load(path: impl Into<PathBuf>, config: CoreConfig) -> Result<Self, PushAttemptError> {
        let path = path.into();
        let document = match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice::<PushAttemptDocument>(&bytes)?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => PushAttemptDocument {
                version: config.push_attempt_format_version,
                attempts: Vec::new(),
            },
            Err(error) => return Err(error.into()),
        };
        if document.version != config.push_attempt_format_version {
            return Err(PushAttemptError::UnsupportedVersion(document.version));
        }
        Ok(Self {
            path,
            config,
            document,
        })
    }

    pub fn attempts(&self) -> &[PushAttempt] {
        &self.document.attempts
    }

    pub fn requested(
        &mut self,
        project_id: Uuid,
        task_id: impl Into<String>,
        profile_id: impl Into<String>,
    ) -> Result<PushAttempt, PushAttemptError> {
        let attempt = PushAttempt {
            id: Uuid::new_v4(),
            task_id: task_id.into(),
            project_id,
            agent_profile_id: profile_id.into(),
            created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            status: PushAttemptStatus::Requested,
            process_id: None,
            error: None,
            delivery: PushDelivery::Process,
        };
        let mut next = self.document.clone();
        next.attempts.push(attempt.clone());
        trim_to_retention(&mut next.attempts, self.config.push_attempt_retention);
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(attempt)
    }

    pub fn update(
        &mut self,
        id: Uuid,
        status: PushAttemptStatus,
        process_id: Option<u32>,
        error: Option<String>,
        delivery: PushDelivery,
    ) -> Result<PushAttempt, PushAttemptError> {
        let mut next = self.document.clone();
        let attempt = next
            .attempts
            .iter_mut()
            .find(|attempt| attempt.id == id)
            .ok_or(PushAttemptError::NotFound(id))?;
        attempt.status = status;
        attempt.process_id = process_id;
        attempt.error = error;
        attempt.delivery = delivery;
        let updated = attempt.clone();
        persist_document(&self.path, &next)?;
        self.document = next;
        Ok(updated)
    }
}

fn trim_to_retention(attempts: &mut Vec<PushAttempt>, retention: usize) {
    if attempts.len() > retention {
        attempts.drain(..attempts.len() - retention);
    }
}

fn persist_document(path: &Path, document: &PushAttemptDocument) -> Result<(), PushAttemptError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "push attempt path has no parent",
        )
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
        Ok::<_, PushAttemptError>(())
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
    fn persists_status_without_storing_prompt_and_applies_retention() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("attempts.json");
        let config = CoreConfig {
            push_attempt_retention: 2,
            ..CoreConfig::default()
        };
        let mut store = PushAttemptStore::load(&path, config.clone()).unwrap();
        let project = Uuid::new_v4();
        let first = store.requested(project, "TASK-001", "codex").unwrap();
        store.requested(project, "TASK-002", "opencode").unwrap();
        let last = store.requested(project, "TASK-003", "claude-code").unwrap();
        assert_eq!(store.attempts().len(), 2);
        assert!(store.attempts().iter().all(|item| item.id != first.id));
        store
            .update(
                last.id,
                PushAttemptStatus::Started,
                Some(42),
                None,
                PushDelivery::Process,
            )
            .unwrap();
        let source = fs::read_to_string(&path).unwrap();
        assert!(!source.contains("pointer prompt"));
        let loaded = PushAttemptStore::load(&path, config).unwrap();
        assert_eq!(loaded.attempts().last().unwrap().process_id, Some(42));
    }
}
