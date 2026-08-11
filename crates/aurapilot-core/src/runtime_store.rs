use crate::config::CoreConfig;
use chrono::{SecondsFormat, Utc};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

const SCHEMA_VERSION: i64 = 3;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentProvider {
    Codex,
    ClaudeCode,
    OpenCode,
    Other,
}

impl AgentProvider {
    pub fn from_profile(profile_id: &str) -> Self {
        match profile_id {
            "codex" => Self::Codex,
            "claude-code" => Self::ClaudeCode,
            "opencode" => Self::OpenCode,
            _ => Self::Other,
        }
    }

    pub fn from_profile_and_executable(profile_id: &str, executable: &str) -> Self {
        let builtin = Self::from_profile(profile_id);
        if builtin != Self::Other {
            return builtin;
        }
        let executable = Path::new(executable)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(executable)
            .to_ascii_lowercase();
        match executable.as_str() {
            "codex" => Self::Codex,
            "claude" | "claude-code" => Self::ClaudeCode,
            "opencode" => Self::OpenCode,
            _ => Self::Other,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude_code",
            Self::OpenCode => "open_code",
            Self::Other => "other",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "codex" => Ok(Self::Codex),
            "claude_code" => Ok(Self::ClaudeCode),
            "open_code" => Ok(Self::OpenCode),
            "other" => Ok(Self::Other),
            _ => Err(RuntimeStoreError::InvalidEnum("provider", value.into())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionBindingSource {
    Managed,
    Discovered,
    IntegrationReported,
    Manual,
}

impl SessionBindingSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Discovered => "discovered",
            Self::IntegrationReported => "integration_reported",
            Self::Manual => "manual",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "managed" => Ok(Self::Managed),
            "discovered" => Ok(Self::Discovered),
            "integration_reported" => Ok(Self::IntegrationReported),
            "manual" => Ok(Self::Manual),
            _ => Err(RuntimeStoreError::InvalidEnum(
                "binding_source",
                value.into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionVerification {
    Verified,
    Unverified,
    Unavailable,
}

impl SessionVerification {
    fn as_str(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Unverified => "unverified",
            Self::Unavailable => "unavailable",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "verified" => Ok(Self::Verified),
            "unverified" => Ok(Self::Unverified),
            "unavailable" => Ok(Self::Unavailable),
            _ => Err(RuntimeStoreError::InvalidEnum("verification", value.into())),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionRuntimeState {
    Starting,
    Idle,
    Running,
    WaitingApproval,
    Interrupting,
    NotLoaded,
    Terminated,
    Failed,
}

impl SessionRuntimeState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Idle => "idle",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Interrupting => "interrupting",
            Self::NotLoaded => "not_loaded",
            Self::Terminated => "terminated",
            Self::Failed => "failed",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "starting" => Ok(Self::Starting),
            "idle" => Ok(Self::Idle),
            "running" => Ok(Self::Running),
            "waiting_approval" => Ok(Self::WaitingApproval),
            "interrupting" => Ok(Self::Interrupting),
            "not_loaded" => Ok(Self::NotLoaded),
            "terminated" => Ok(Self::Terminated),
            "failed" => Ok(Self::Failed),
            _ => Err(RuntimeStoreError::InvalidEnum(
                "session_state",
                value.into(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushMode {
    ExistingSession,
    Fork,
    NewSession,
}

impl PushMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExistingSession => "existing_session",
            Self::Fork => "fork",
            Self::NewSession => "new_session",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushDeliveryPolicy {
    SafeBoundary,
    SteerCurrentTurn,
    InterruptThenAppend,
}

impl PushDeliveryPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::SafeBoundary => "safe_boundary",
            Self::SteerCurrentTurn => "steer_current_turn",
            Self::InterruptThenAppend => "interrupt_then_append",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PushStatus {
    Queued,
    Delivering,
    Delivered,
    DeliveryUnknown,
    Failed,
    Cancelled,
}

impl PushStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Delivering => "delivering",
            Self::Delivered => "delivered",
            Self::DeliveryUnknown => "delivery_unknown",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    fn parse(value: &str) -> Result<Self, RuntimeStoreError> {
        match value {
            "queued" => Ok(Self::Queued),
            "delivering" => Ok(Self::Delivering),
            "delivered" => Ok(Self::Delivered),
            "delivery_unknown" => Ok(Self::DeliveryUnknown),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            _ => Err(RuntimeStoreError::InvalidEnum("push_status", value.into())),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SessionBinding {
    pub id: Uuid,
    pub project_id: Uuid,
    pub profile_id: String,
    pub provider: AgentProvider,
    pub external_session_id: String,
    pub source: SessionBindingSource,
    pub verification: SessionVerification,
    pub display_name: Option<String>,
    pub working_directory: PathBuf,
    pub state: SessionRuntimeState,
    pub active_turn_id: Option<String>,
    pub hidden: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_used_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RunRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: String,
    pub profile_id: String,
    pub provider: AgentProvider,
    pub session_binding_id: Option<Uuid>,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PushRecord {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: String,
    pub selected_profile_id: Option<String>,
    pub target_run_id: Option<Uuid>,
    pub target_session_id: Option<Uuid>,
    pub mode: PushMode,
    pub delivery: PushDeliveryPolicy,
    pub content: String,
    pub idempotency_key: String,
    pub status: PushStatus,
    pub resolved_run_id: Option<Uuid>,
    pub resolved_session_id: Option<Uuid>,
    pub attempt_count: u32,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub delivered_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ExecutionEvent {
    pub id: Uuid,
    pub project_id: Uuid,
    pub task_id: String,
    pub profile_id: String,
    pub provider: AgentProvider,
    pub session_binding_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub kind: String,
    pub level: String,
    pub phase: String,
    pub message: String,
    pub detail: Option<String>,
    pub created_at: String,
}

#[derive(Clone, Debug)]
pub struct NewExecutionEvent<'a> {
    pub project_id: Uuid,
    pub task_id: &'a str,
    pub profile_id: &'a str,
    pub provider: AgentProvider,
    pub session_binding_id: Option<Uuid>,
    pub attempt_id: Option<Uuid>,
    pub kind: &'a str,
    pub level: &'a str,
    pub phase: &'a str,
    pub message: &'a str,
    pub detail: Option<&'a str>,
}

#[derive(Clone, Debug)]
pub struct NewSessionBinding<'a> {
    pub project_id: Uuid,
    pub profile_id: &'a str,
    pub provider: AgentProvider,
    pub external_session_id: &'a str,
    pub source: SessionBindingSource,
    pub verification: SessionVerification,
    pub display_name: Option<&'a str>,
    pub working_directory: &'a Path,
    pub state: SessionRuntimeState,
}

#[derive(Clone, Debug)]
pub struct NewPush<'a> {
    pub project_id: Uuid,
    pub task_id: &'a str,
    pub selected_profile_id: Option<&'a str>,
    pub target_run_id: Option<Uuid>,
    pub target_session_id: Option<Uuid>,
    pub mode: PushMode,
    pub delivery: PushDeliveryPolicy,
    pub content: &'a str,
    pub idempotency_key: &'a str,
}

#[derive(Debug, Error)]
pub enum RuntimeStoreError {
    #[error("runtime database path has no parent: {0}")]
    MissingParent(PathBuf),
    #[error("invalid {0} value in runtime database: {1}")]
    InvalidEnum(&'static str, String),
    #[error("session binding not found: {0}")]
    SessionNotFound(Uuid),
    #[error("session belongs to project {actual}, not {expected}")]
    ProjectMismatch { expected: Uuid, actual: Uuid },
    #[error("session profile is {actual}, not {expected}; create a new session to change profile")]
    ProfileMismatch { expected: String, actual: String },
    #[error("session {0} is active and cannot be edited")]
    SessionActive(Uuid),
    #[error("existing-session push requires a target session")]
    SessionRequired,
    #[error("new-session push requires a selected profile")]
    ProfileRequired,
    #[error("push is not queued and cannot begin delivery: {0}")]
    PushNotQueued(Uuid),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Uuid(#[from] uuid::Error),
}

pub struct RuntimeStore {
    connection: Connection,
    execution_event_retention: usize,
    execution_event_message_max_bytes: usize,
    execution_event_detail_max_bytes: usize,
}

impl RuntimeStore {
    pub fn open(path: impl AsRef<Path>, config: &CoreConfig) -> Result<Self, RuntimeStoreError> {
        let path = path.as_ref();
        let parent = path
            .parent()
            .ok_or_else(|| RuntimeStoreError::MissingParent(path.to_path_buf()))?;
        fs::create_dir_all(parent)?;
        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "FULL")?;
        connection.busy_timeout(config.sqlite_busy_timeout)?;
        let mut store = Self {
            connection,
            execution_event_retention: config.execution_event_retention,
            execution_event_message_max_bytes: config.execution_event_message_max_bytes,
            execution_event_detail_max_bytes: config.execution_event_detail_max_bytes,
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&mut self) -> Result<(), RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL
            );",
        )?;
        let current = tx
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get::<_, Option<i64>>(0)
            })?
            .unwrap_or(0);
        if current < 1 {
            tx.execute_batch(include_str!("../migrations/0001_runtime.sql"))?;
            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
                [now()],
            )?;
        }
        if current < 2 {
            tx.execute_batch(include_str!("../migrations/0002_push_inbox.sql"))?;
            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                params![2, now()],
            )?;
        }
        if current < 3 {
            tx.execute_batch(include_str!("../migrations/0003_execution_events.sql"))?;
            tx.execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (?1, ?2)",
                params![SCHEMA_VERSION, now()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64, RuntimeStoreError> {
        Ok(self.connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?)
    }

    pub fn register_session(
        &mut self,
        input: NewSessionBinding<'_>,
    ) -> Result<SessionBinding, RuntimeStoreError> {
        let timestamp = now();
        let existing = self
            .connection
            .query_row(
                "SELECT id, profile_id FROM session_bindings
                 WHERE project_id = ?1 AND provider = ?2 AND external_session_id = ?3",
                params![
                    input.project_id.to_string(),
                    input.provider.as_str(),
                    input.external_session_id
                ],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let id = match existing {
            Some((value, profile_id)) => {
                if profile_id != input.profile_id {
                    return Err(RuntimeStoreError::ProfileMismatch {
                        expected: input.profile_id.into(),
                        actual: profile_id,
                    });
                }
                Uuid::parse_str(&value)?
            }
            None => Uuid::new_v4(),
        };
        self.connection.execute(
            "INSERT INTO session_bindings (
                id, project_id, profile_id, provider, external_session_id, binding_source,
                verification_status, display_name, working_directory, state, hidden,
                created_at, updated_at, last_used_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?11, ?11)
             ON CONFLICT(project_id, provider, external_session_id) DO UPDATE SET
                binding_source = excluded.binding_source,
                verification_status = excluded.verification_status,
                display_name = COALESCE(excluded.display_name, session_bindings.display_name),
                working_directory = excluded.working_directory,
                state = excluded.state,
                hidden = 0,
                updated_at = excluded.updated_at,
                last_used_at = excluded.last_used_at",
            params![
                id.to_string(),
                input.project_id.to_string(),
                input.profile_id,
                input.provider.as_str(),
                input.external_session_id,
                input.source.as_str(),
                input.verification.as_str(),
                input.display_name,
                input.working_directory.to_string_lossy(),
                input.state.as_str(),
                timestamp,
            ],
        )?;
        self.session(id)?
            .ok_or(RuntimeStoreError::SessionNotFound(id))
    }

    pub fn list_sessions(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<SessionBinding>, RuntimeStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, profile_id, provider, external_session_id, binding_source,
                    verification_status, display_name, working_directory, state, active_turn_id,
                    hidden, created_at, updated_at, last_used_at
             FROM session_bindings WHERE project_id = ?1 AND hidden = 0
             ORDER BY last_used_at DESC",
        )?;
        let rows = statement.query_map([project_id.to_string()], map_session)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn sessions_with_queued_pushes(
        &self,
        provider: AgentProvider,
    ) -> Result<Vec<SessionBinding>, RuntimeStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT DISTINCT
                    s.id, s.project_id, s.profile_id, s.provider, s.external_session_id,
                    s.binding_source, s.verification_status, s.display_name,
                    s.working_directory, s.state, s.active_turn_id, s.hidden,
                    s.created_at, s.updated_at, s.last_used_at
             FROM session_bindings s
             JOIN push_requests p
               ON COALESCE(p.resolved_session_id, p.target_session_id) = s.id
             WHERE p.status = 'queued' AND s.provider = ?1 AND s.hidden = 0
             ORDER BY s.last_used_at ASC",
        )?;
        let rows = statement.query_map([provider.as_str()], map_session)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn session(&self, id: Uuid) -> Result<Option<SessionBinding>, RuntimeStoreError> {
        self.connection
            .query_row(
                "SELECT id, project_id, profile_id, provider, external_session_id, binding_source,
                        verification_status, display_name, working_directory, state, active_turn_id,
                        hidden, created_at, updated_at, last_used_at
                 FROM session_bindings WHERE id = ?1",
                [id.to_string()],
                map_session,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn update_session_binding(
        &mut self,
        id: Uuid,
        project_id: Uuid,
        external_session_id: &str,
        display_name: Option<&str>,
        verification: SessionVerification,
    ) -> Result<SessionBinding, RuntimeStoreError> {
        let session = self
            .session(id)?
            .ok_or(RuntimeStoreError::SessionNotFound(id))?;
        if session.project_id != project_id {
            return Err(RuntimeStoreError::ProjectMismatch {
                expected: project_id,
                actual: session.project_id,
            });
        }
        if matches!(
            session.state,
            SessionRuntimeState::Starting
                | SessionRuntimeState::Running
                | SessionRuntimeState::WaitingApproval
                | SessionRuntimeState::Interrupting
        ) {
            return Err(RuntimeStoreError::SessionActive(id));
        }
        self.connection.execute(
            "UPDATE session_bindings SET external_session_id = ?2, display_name = ?3,
                verification_status = ?4, updated_at = ?5
             WHERE id = ?1",
            params![
                id.to_string(),
                external_session_id,
                display_name,
                verification.as_str(),
                now(),
            ],
        )?;
        self.session(id)?
            .ok_or(RuntimeStoreError::SessionNotFound(id))
    }

    pub fn create_run(
        &mut self,
        project_id: Uuid,
        task_id: &str,
        profile_id: &str,
        provider: AgentProvider,
        session_binding_id: Option<Uuid>,
        status: &str,
    ) -> Result<RunRecord, RuntimeStoreError> {
        if let Some(session_id) = session_binding_id {
            let session = self
                .session(session_id)?
                .ok_or(RuntimeStoreError::SessionNotFound(session_id))?;
            validate_session_target(&session, project_id, profile_id)?;
        }
        let id = Uuid::new_v4();
        let timestamp = now();
        self.connection.execute(
            "INSERT INTO runs (
                id, project_id, task_id, profile_id, provider, session_binding_id,
                status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
            params![
                id.to_string(),
                project_id.to_string(),
                task_id,
                profile_id,
                provider.as_str(),
                session_binding_id.map(|value| value.to_string()),
                status,
                timestamp,
            ],
        )?;
        Ok(RunRecord {
            id,
            project_id,
            task_id: task_id.into(),
            profile_id: profile_id.into(),
            provider,
            session_binding_id,
            status: status.into(),
            created_at: timestamp.clone(),
            updated_at: timestamp,
        })
    }

    pub fn create_run_with_session(
        &mut self,
        project_id: Uuid,
        task_id: &str,
        profile_id: &str,
        provider: AgentProvider,
        session: NewSessionBinding<'_>,
        status: &str,
    ) -> Result<(RunRecord, SessionBinding), RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        let binding = register_session_in_transaction(&tx, session)?;
        let run = create_run_in_transaction(
            &tx,
            project_id,
            task_id,
            profile_id,
            provider,
            Some(binding.id),
            status,
        )?;
        tx.commit()?;
        Ok((run, binding))
    }

    pub fn enqueue_push(&mut self, input: NewPush<'_>) -> Result<PushRecord, RuntimeStoreError> {
        match input.mode {
            PushMode::ExistingSession | PushMode::Fork => {
                let target = input
                    .target_session_id
                    .ok_or(RuntimeStoreError::SessionRequired)?;
                let session = self
                    .session(target)?
                    .ok_or(RuntimeStoreError::SessionNotFound(target))?;
                if session.project_id != input.project_id {
                    return Err(RuntimeStoreError::ProjectMismatch {
                        expected: input.project_id,
                        actual: session.project_id,
                    });
                }
            }
            PushMode::NewSession if input.selected_profile_id.is_none() => {
                return Err(RuntimeStoreError::ProfileRequired);
            }
            PushMode::NewSession => {}
        }
        if let Some(existing) = self.push_by_idempotency_key(input.idempotency_key)? {
            return Ok(existing);
        }
        let id = Uuid::new_v4();
        let timestamp = now();
        self.connection.execute(
            "INSERT INTO push_requests (
                id, project_id, task_id, selected_profile_id, target_run_id, target_session_id,
                mode, delivery, content, idempotency_key, status, attempt_count,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'queued', 0, ?11, ?11)",
            params![
                id.to_string(),
                input.project_id.to_string(),
                input.task_id,
                input.selected_profile_id,
                input.target_run_id.map(|value| value.to_string()),
                input.target_session_id.map(|value| value.to_string()),
                input.mode.as_str(),
                input.delivery.as_str(),
                input.content,
                input.idempotency_key,
                timestamp,
            ],
        )?;
        self.push(id)?
            .ok_or_else(|| RuntimeStoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn begin_delivery(&mut self, push_id: Uuid) -> Result<(), RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        let attempt: u32 = tx.query_row(
            "SELECT attempt_count + 1 FROM push_requests WHERE id = ?1",
            [push_id.to_string()],
            |row| row.get(0),
        )?;
        let changed = tx.execute(
            "UPDATE push_requests SET status = 'delivering', attempt_count = ?2, updated_at = ?3
             WHERE id = ?1 AND status = 'queued'",
            params![push_id.to_string(), attempt, now()],
        )?;
        if changed == 0 {
            return Err(RuntimeStoreError::PushNotQueued(push_id));
        }
        tx.execute(
            "INSERT INTO push_delivery_attempts (
                id, push_id, attempt_number, started_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                Uuid::new_v4().to_string(),
                push_id.to_string(),
                attempt,
                now()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn claim_next_queued_push(
        &mut self,
        session_id: Uuid,
    ) -> Result<Option<PushRecord>, RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        let next = tx
            .query_row(
                "SELECT id FROM push_requests
                 WHERE status = 'queued'
                   AND COALESCE(resolved_session_id, target_session_id) = ?1
                 ORDER BY created_at ASC, rowid ASC
                 LIMIT 1",
                [session_id.to_string()],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(next) = next else {
            tx.commit()?;
            return Ok(None);
        };
        let push_id = Uuid::parse_str(&next)?;
        let attempt: u32 = tx.query_row(
            "SELECT attempt_count + 1 FROM push_requests WHERE id = ?1",
            [next.as_str()],
            |row| row.get(0),
        )?;
        let changed = tx.execute(
            "UPDATE push_requests SET status = 'delivering', attempt_count = ?2, updated_at = ?3
             WHERE id = ?1 AND status = 'queued'",
            params![next, attempt, now()],
        )?;
        if changed == 0 {
            return Err(RuntimeStoreError::PushNotQueued(push_id));
        }
        tx.execute(
            "INSERT INTO push_delivery_attempts (
                id, push_id, attempt_number, started_at
             ) VALUES (?1, ?2, ?3, ?4)",
            params![Uuid::new_v4().to_string(), next, attempt, now()],
        )?;
        tx.commit()?;
        self.push(push_id)
    }

    pub fn resolve_push(
        &mut self,
        push_id: Uuid,
        run_id: Uuid,
        session_id: Uuid,
    ) -> Result<(), RuntimeStoreError> {
        self.connection.execute(
            "UPDATE push_requests SET resolved_run_id = ?2, resolved_session_id = ?3,
                updated_at = ?4 WHERE id = ?1",
            params![
                push_id.to_string(),
                run_id.to_string(),
                session_id.to_string(),
                now(),
            ],
        )?;
        Ok(())
    }

    pub fn update_session_runtime(
        &mut self,
        session_id: Uuid,
        state: SessionRuntimeState,
        active_turn_id: Option<&str>,
    ) -> Result<(), RuntimeStoreError> {
        let changed = self.connection.execute(
            "UPDATE session_bindings SET state = ?2, active_turn_id = ?3,
                updated_at = ?4, last_used_at = ?4 WHERE id = ?1",
            params![
                session_id.to_string(),
                state.as_str(),
                active_turn_id,
                now(),
            ],
        )?;
        if changed == 0 {
            return Err(RuntimeStoreError::SessionNotFound(session_id));
        }
        Ok(())
    }

    pub fn finish_delivery(
        &mut self,
        push_id: Uuid,
        status: PushStatus,
        provider_receipt: Option<&str>,
        error: Option<&str>,
        retryable: bool,
    ) -> Result<PushRecord, RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        let timestamp = now();
        tx.execute(
            "UPDATE push_requests SET status = ?2, last_error = ?3, updated_at = ?4,
                delivered_at = CASE WHEN ?2 = 'delivered' THEN ?4 ELSE delivered_at END
             WHERE id = ?1",
            params![push_id.to_string(), status.as_str(), error, timestamp],
        )?;
        tx.execute(
            "UPDATE push_delivery_attempts SET finished_at = ?2, provider_receipt = ?3,
                error = ?4, retryable = ?5
             WHERE id = (
                SELECT id FROM push_delivery_attempts WHERE push_id = ?1
                ORDER BY attempt_number DESC LIMIT 1
             )",
            params![
                push_id.to_string(),
                timestamp,
                provider_receipt,
                error,
                i64::from(retryable),
            ],
        )?;
        tx.commit()?;
        self.push(push_id)?
            .ok_or_else(|| RuntimeStoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn requeue_delivery(
        &mut self,
        push_id: Uuid,
        reason: &str,
    ) -> Result<PushRecord, RuntimeStoreError> {
        let tx = self.connection.transaction()?;
        let timestamp = now();
        let changed = tx.execute(
            "UPDATE push_requests SET status = 'queued', last_error = ?2, updated_at = ?3
             WHERE id = ?1 AND status = 'delivering'",
            params![push_id.to_string(), reason, timestamp],
        )?;
        if changed == 0 {
            return Err(RuntimeStoreError::PushNotQueued(push_id));
        }
        tx.execute(
            "UPDATE push_delivery_attempts SET finished_at = ?2, error = ?3, retryable = 1
             WHERE id = (
                SELECT id FROM push_delivery_attempts WHERE push_id = ?1
                ORDER BY attempt_number DESC LIMIT 1
             )",
            params![push_id.to_string(), timestamp, reason],
        )?;
        tx.commit()?;
        self.push(push_id)?
            .ok_or_else(|| RuntimeStoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn recover_interrupted_deliveries(&mut self) -> Result<usize, RuntimeStoreError> {
        Ok(self.connection.execute(
            "UPDATE push_requests SET status = 'delivery_unknown',
                last_error = 'AuraPilot exited while delivery was in progress; verify the provider session before retrying',
                updated_at = ?1
             WHERE status = 'delivering'",
            [now()],
        )?)
    }

    pub fn recover_loaded_sessions(&mut self) -> Result<usize, RuntimeStoreError> {
        Ok(self.connection.execute(
            "UPDATE session_bindings SET state = 'not_loaded', active_turn_id = NULL,
                updated_at = ?1
             WHERE state IN ('starting', 'running', 'waiting_approval', 'interrupting')",
            [now()],
        )?)
    }

    fn push_by_idempotency_key(&self, key: &str) -> Result<Option<PushRecord>, RuntimeStoreError> {
        let id = self
            .connection
            .query_row(
                "SELECT id FROM push_requests WHERE idempotency_key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|value| self.push(Uuid::parse_str(&value)?))
            .transpose()
            .map(Option::flatten)
    }

    pub fn push(&self, id: Uuid) -> Result<Option<PushRecord>, RuntimeStoreError> {
        self.connection
            .query_row(
                "SELECT id, project_id, task_id, selected_profile_id, target_run_id,
                        target_session_id, mode, delivery, content, idempotency_key, status,
                        resolved_run_id, resolved_session_id, attempt_count, last_error,
                        created_at, updated_at, delivered_at
                 FROM push_requests WHERE id = ?1",
                [id.to_string()],
                map_push,
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn append_execution_event(
        &mut self,
        input: NewExecutionEvent<'_>,
    ) -> Result<ExecutionEvent, RuntimeStoreError> {
        let event = ExecutionEvent {
            id: Uuid::new_v4(),
            project_id: input.project_id,
            task_id: input.task_id.to_owned(),
            profile_id: input.profile_id.to_owned(),
            provider: input.provider,
            session_binding_id: input.session_binding_id,
            attempt_id: input.attempt_id,
            kind: input.kind.to_owned(),
            level: input.level.to_owned(),
            phase: input.phase.to_owned(),
            message: truncate_utf8(input.message, self.execution_event_message_max_bytes),
            detail: input
                .detail
                .map(|value| truncate_utf8(value, self.execution_event_detail_max_bytes)),
            created_at: now(),
        };
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO execution_events (
                id, project_id, task_id, profile_id, provider, session_binding_id,
                attempt_id, kind, level, phase, message, detail, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                event.id.to_string(),
                event.project_id.to_string(),
                event.task_id,
                event.profile_id,
                event.provider.as_str(),
                event.session_binding_id.map(|value| value.to_string()),
                event.attempt_id.map(|value| value.to_string()),
                event.kind,
                event.level,
                event.phase,
                event.message,
                event.detail,
                event.created_at,
            ],
        )?;
        tx.execute(
            "DELETE FROM execution_events
             WHERE project_id = ?1 AND id NOT IN (
                SELECT id FROM execution_events WHERE project_id = ?1
                ORDER BY created_at DESC, rowid DESC LIMIT ?2
             )",
            params![
                input.project_id.to_string(),
                self.execution_event_retention as i64
            ],
        )?;
        tx.commit()?;
        Ok(event)
    }

    pub fn list_execution_events(
        &self,
        project_id: Option<Uuid>,
        task_id: Option<&str>,
        limit: usize,
    ) -> Result<Vec<ExecutionEvent>, RuntimeStoreError> {
        let limit = limit.min(self.execution_event_retention) as i64;
        let project = project_id.map(|value| value.to_string());
        let mut statement = self.connection.prepare(
            "SELECT id, project_id, task_id, profile_id, provider, session_binding_id,
                    attempt_id, kind, level, phase, message, detail, created_at
             FROM execution_events
             WHERE (?1 IS NULL OR project_id = ?1)
               AND (?2 IS NULL OR task_id = ?2)
             ORDER BY created_at DESC, rowid DESC LIMIT ?3",
        )?;
        let rows = statement.query_map(params![project, task_id, limit], map_execution_event)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes.min(value.len());
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &value[..end])
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn validate_session_target(
    session: &SessionBinding,
    project_id: Uuid,
    profile_id: &str,
) -> Result<(), RuntimeStoreError> {
    if session.project_id != project_id {
        return Err(RuntimeStoreError::ProjectMismatch {
            expected: project_id,
            actual: session.project_id,
        });
    }
    if session.profile_id != profile_id {
        return Err(RuntimeStoreError::ProfileMismatch {
            expected: profile_id.into(),
            actual: session.profile_id.clone(),
        });
    }
    Ok(())
}

fn register_session_in_transaction(
    tx: &Transaction<'_>,
    input: NewSessionBinding<'_>,
) -> Result<SessionBinding, RuntimeStoreError> {
    let id = Uuid::new_v4();
    let timestamp = now();
    tx.execute(
        "INSERT INTO session_bindings (
            id, project_id, profile_id, provider, external_session_id, binding_source,
            verification_status, display_name, working_directory, state, hidden,
            created_at, updated_at, last_used_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 0, ?11, ?11, ?11)",
        params![
            id.to_string(),
            input.project_id.to_string(),
            input.profile_id,
            input.provider.as_str(),
            input.external_session_id,
            input.source.as_str(),
            input.verification.as_str(),
            input.display_name,
            input.working_directory.to_string_lossy(),
            input.state.as_str(),
            timestamp,
        ],
    )?;
    Ok(SessionBinding {
        id,
        project_id: input.project_id,
        profile_id: input.profile_id.into(),
        provider: input.provider,
        external_session_id: input.external_session_id.into(),
        source: input.source,
        verification: input.verification,
        display_name: input.display_name.map(str::to_owned),
        working_directory: input.working_directory.into(),
        state: input.state,
        active_turn_id: None,
        hidden: false,
        created_at: timestamp.clone(),
        updated_at: timestamp.clone(),
        last_used_at: timestamp,
    })
}

fn create_run_in_transaction(
    tx: &Transaction<'_>,
    project_id: Uuid,
    task_id: &str,
    profile_id: &str,
    provider: AgentProvider,
    session_binding_id: Option<Uuid>,
    status: &str,
) -> Result<RunRecord, RuntimeStoreError> {
    let id = Uuid::new_v4();
    let timestamp = now();
    tx.execute(
        "INSERT INTO runs (
            id, project_id, task_id, profile_id, provider, session_binding_id,
            status, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
        params![
            id.to_string(),
            project_id.to_string(),
            task_id,
            profile_id,
            provider.as_str(),
            session_binding_id.map(|value| value.to_string()),
            status,
            timestamp,
        ],
    )?;
    Ok(RunRecord {
        id,
        project_id,
        task_id: task_id.into(),
        profile_id: profile_id.into(),
        provider,
        session_binding_id,
        status: status.into(),
        created_at: timestamp.clone(),
        updated_at: timestamp,
    })
}

fn map_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<SessionBinding> {
    let parse_error = |index, error: RuntimeStoreError| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    };
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let provider: String = row.get(3)?;
    let source: String = row.get(5)?;
    let verification: String = row.get(6)?;
    let state: String = row.get(9)?;
    Ok(SessionBinding {
        id: Uuid::parse_str(&id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        project_id: Uuid::parse_str(&project_id).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                1,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        profile_id: row.get(2)?,
        provider: AgentProvider::parse(&provider).map_err(|error| parse_error(3, error))?,
        external_session_id: row.get(4)?,
        source: SessionBindingSource::parse(&source).map_err(|error| parse_error(5, error))?,
        verification: SessionVerification::parse(&verification)
            .map_err(|error| parse_error(6, error))?,
        display_name: row.get(7)?,
        working_directory: PathBuf::from(row.get::<_, String>(8)?),
        state: SessionRuntimeState::parse(&state).map_err(|error| parse_error(9, error))?,
        active_turn_id: row.get(10)?,
        hidden: row.get::<_, i64>(11)? != 0,
        created_at: row.get(12)?,
        updated_at: row.get(13)?,
        last_used_at: row.get(14)?,
    })
}

fn map_push(row: &rusqlite::Row<'_>) -> rusqlite::Result<PushRecord> {
    let parse_uuid = |index: usize, value: String| {
        Uuid::parse_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    };
    let parse_optional_uuid = |index: usize, value: Option<String>| {
        value.map(|value| parse_uuid(index, value)).transpose()
    };
    let mode: String = row.get(6)?;
    let delivery: String = row.get(7)?;
    let status: String = row.get(10)?;
    Ok(PushRecord {
        id: parse_uuid(0, row.get(0)?)?,
        project_id: parse_uuid(1, row.get(1)?)?,
        task_id: row.get(2)?,
        selected_profile_id: row.get(3)?,
        target_run_id: parse_optional_uuid(4, row.get(4)?)?,
        target_session_id: parse_optional_uuid(5, row.get(5)?)?,
        mode: match mode.as_str() {
            "existing_session" => PushMode::ExistingSession,
            "fork" => PushMode::Fork,
            "new_session" => PushMode::NewSession,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        delivery: match delivery.as_str() {
            "safe_boundary" => PushDeliveryPolicy::SafeBoundary,
            "steer_current_turn" => PushDeliveryPolicy::SteerCurrentTurn,
            "interrupt_then_append" => PushDeliveryPolicy::InterruptThenAppend,
            _ => return Err(rusqlite::Error::InvalidQuery),
        },
        content: row.get(8)?,
        idempotency_key: row.get(9)?,
        status: PushStatus::parse(&status).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                10,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        resolved_run_id: parse_optional_uuid(11, row.get(11)?)?,
        resolved_session_id: parse_optional_uuid(12, row.get(12)?)?,
        attempt_count: row.get(13)?,
        last_error: row.get(14)?,
        created_at: row.get(15)?,
        updated_at: row.get(16)?,
        delivered_at: row.get(17)?,
    })
}

fn map_execution_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionEvent> {
    let parse_uuid = |index: usize, value: String| {
        Uuid::parse_str(&value).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                index,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
    };
    let parse_optional_uuid = |index: usize, value: Option<String>| {
        value.map(|value| parse_uuid(index, value)).transpose()
    };
    let provider: String = row.get(4)?;
    Ok(ExecutionEvent {
        id: parse_uuid(0, row.get(0)?)?,
        project_id: parse_uuid(1, row.get(1)?)?,
        task_id: row.get(2)?,
        profile_id: row.get(3)?,
        provider: AgentProvider::parse(&provider).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        session_binding_id: parse_optional_uuid(5, row.get(5)?)?,
        attempt_id: parse_optional_uuid(6, row.get(6)?)?,
        kind: row.get(7)?,
        level: row.get(8)?,
        phase: row.get(9)?,
        message: row.get(10)?,
        detail: row.get(11)?,
        created_at: row.get(12)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn store() -> RuntimeStore {
        let dir = tempdir().unwrap().keep();
        RuntimeStore::open(dir.join("runtime.sqlite3"), &CoreConfig::default()).unwrap()
    }

    #[test]
    fn migrations_and_fixed_pragmas_are_applied() {
        let store = store();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .connection
                .query_row("PRAGMA synchronous", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            2
        );
    }

    #[test]
    fn execution_events_are_persisted_filtered_truncated_and_retained() {
        let dir = tempdir().unwrap();
        let config = CoreConfig {
            execution_event_retention: 2,
            execution_event_message_max_bytes: 8,
            execution_event_detail_max_bytes: 10,
            ..CoreConfig::default()
        };
        let mut store = RuntimeStore::open(dir.path().join("runtime.sqlite3"), &config).unwrap();
        let first_project = Uuid::new_v4();
        let second_project = Uuid::new_v4();
        for (index, task) in ["TASK-001", "TASK-001", "TASK-002"].into_iter().enumerate() {
            store
                .append_execution_event(NewExecutionEvent {
                    project_id: first_project,
                    task_id: task,
                    profile_id: "codex",
                    provider: AgentProvider::Codex,
                    session_binding_id: None,
                    attempt_id: None,
                    kind: "lifecycle",
                    level: "info",
                    phase: "turn",
                    message: &format!("event-{index}-long"),
                    detail: Some("0123456789-long"),
                })
                .unwrap();
        }
        store
            .append_execution_event(NewExecutionEvent {
                project_id: second_project,
                task_id: "TASK-900",
                profile_id: "opencode",
                provider: AgentProvider::OpenCode,
                session_binding_id: None,
                attempt_id: None,
                kind: "diagnostic",
                level: "warning",
                phase: "provider",
                message: "separate project",
                detail: None,
            })
            .unwrap();

        let retained = store
            .list_execution_events(Some(first_project), None, 20)
            .unwrap();
        assert_eq!(retained.len(), 2);
        assert!(retained.iter().all(|event| event.message.len() <= 11));
        assert!(
            retained
                .iter()
                .all(|event| event.detail.as_deref() == Some("0123456789…"))
        );
        assert_eq!(
            store
                .list_execution_events(Some(first_project), Some("TASK-001"), 20)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store.list_execution_events(None, None, 20).unwrap().len(),
            2
        );
    }

    #[test]
    fn custom_profiles_infer_provider_from_their_executable_without_changing_profile_identity() {
        assert_eq!(
            AgentProvider::from_profile_and_executable("codex-review", "/usr/bin/codex"),
            AgentProvider::Codex
        );
        assert_eq!(
            AgentProvider::from_profile_and_executable("claude-fast", "claude"),
            AgentProvider::ClaudeCode
        );
        assert_eq!(
            AgentProvider::from_profile_and_executable("private-tool", "agent-wrapper"),
            AgentProvider::Other
        );
    }

    #[test]
    fn existing_version_one_database_is_upgraded_without_rewriting_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("runtime.sqlite3");
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations (
                    version INTEGER PRIMARY KEY,
                    applied_at TEXT NOT NULL
                 );",
            )
            .unwrap();
        connection
            .execute_batch(include_str!("../migrations/0001_runtime.sql"))
            .unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations(version, applied_at) VALUES (1, ?1)",
                [now()],
            )
            .unwrap();
        drop(connection);

        let store = RuntimeStore::open(&path, &CoreConfig::default()).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        let index_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'index' AND name = 'idx_push_session_inbox'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(index_count, 1);
        let event_table_count: i64 = store
            .connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master
                 WHERE type = 'table' AND name = 'execution_events'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(event_table_count, 1);
    }

    #[test]
    fn sessions_are_project_scoped_and_profile_locked() {
        let mut store = store();
        let project = Uuid::new_v4();
        let binding = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "codex-review",
                provider: AgentProvider::Codex,
                external_session_id: "thr_123",
                source: SessionBindingSource::Manual,
                verification: SessionVerification::Unverified,
                display_name: Some("Review thread"),
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::NotLoaded,
            })
            .unwrap();
        assert_eq!(
            store.list_sessions(project).unwrap(),
            std::slice::from_ref(&binding)
        );
        let error = store
            .create_run(
                project,
                "TASK-001",
                "codex-other",
                AgentProvider::Codex,
                Some(binding.id),
                "starting",
            )
            .unwrap_err();
        assert!(matches!(error, RuntimeStoreError::ProfileMismatch { .. }));

        let rebind_error = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "codex-other",
                provider: AgentProvider::Codex,
                external_session_id: "thr_123",
                source: SessionBindingSource::Manual,
                verification: SessionVerification::Unverified,
                display_name: None,
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::NotLoaded,
            })
            .unwrap_err();
        assert!(matches!(
            rebind_error,
            RuntimeStoreError::ProfileMismatch { .. }
        ));
    }

    #[test]
    fn idle_session_binding_can_be_renamed_and_repointed_without_changing_profile() {
        let mut store = store();
        let project = Uuid::new_v4();
        let binding = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "opencode-review",
                provider: AgentProvider::OpenCode,
                external_session_id: "ses_old",
                source: SessionBindingSource::Manual,
                verification: SessionVerification::Unverified,
                display_name: Some("Old"),
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::NotLoaded,
            })
            .unwrap();

        let updated = store
            .update_session_binding(
                binding.id,
                project,
                "ses_new",
                Some("New"),
                SessionVerification::Verified,
            )
            .unwrap();
        assert_eq!(updated.profile_id, "opencode-review");
        assert_eq!(updated.external_session_id, "ses_new");
        assert_eq!(updated.display_name.as_deref(), Some("New"));
        assert_eq!(updated.verification, SessionVerification::Verified);
    }

    #[test]
    fn active_session_binding_cannot_be_edited() {
        let mut store = store();
        let project = Uuid::new_v4();
        let binding = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "codex",
                provider: AgentProvider::Codex,
                external_session_id: "thr_active",
                source: SessionBindingSource::Managed,
                verification: SessionVerification::Verified,
                display_name: None,
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::Running,
            })
            .unwrap();

        assert!(matches!(
            store.update_session_binding(
                binding.id,
                project,
                "thr_changed",
                None,
                SessionVerification::Unverified,
            ),
            Err(RuntimeStoreError::SessionActive(id)) if id == binding.id
        ));
    }

    #[test]
    fn push_idempotency_and_crash_ambiguity_are_explicit() {
        let mut store = store();
        let project = Uuid::new_v4();
        let first = store
            .enqueue_push(NewPush {
                project_id: project,
                task_id: "TASK-001",
                selected_profile_id: Some("codex"),
                target_run_id: None,
                target_session_id: None,
                mode: PushMode::NewSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: "pointer",
                idempotency_key: "client-key",
            })
            .unwrap();
        let duplicate = store
            .enqueue_push(NewPush {
                project_id: project,
                task_id: "TASK-001",
                selected_profile_id: Some("codex"),
                target_run_id: None,
                target_session_id: None,
                mode: PushMode::NewSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: "pointer",
                idempotency_key: "client-key",
            })
            .unwrap();
        assert_eq!(duplicate.id, first.id);
        store.begin_delivery(first.id).unwrap();
        assert_eq!(store.recover_interrupted_deliveries().unwrap(), 1);
        assert_eq!(
            store.push(first.id).unwrap().unwrap().status,
            PushStatus::DeliveryUnknown
        );
    }

    #[test]
    fn provider_busy_requeues_a_claimed_push_without_losing_fifo_position() {
        let mut store = store();
        let project = Uuid::new_v4();
        let session = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "opencode",
                provider: AgentProvider::OpenCode,
                external_session_id: "ses_busy",
                source: SessionBindingSource::Managed,
                verification: SessionVerification::Verified,
                display_name: None,
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::NotLoaded,
            })
            .unwrap();
        let first = store
            .enqueue_push(NewPush {
                project_id: project,
                task_id: "TASK-001",
                selected_profile_id: None,
                target_run_id: None,
                target_session_id: Some(session.id),
                mode: PushMode::ExistingSession,
                delivery: PushDeliveryPolicy::SafeBoundary,
                content: "first",
                idempotency_key: "busy-1",
            })
            .unwrap();
        let claimed = store.claim_next_queued_push(session.id).unwrap().unwrap();
        assert_eq!(claimed.id, first.id);
        let queued = store.requeue_delivery(first.id, "provider busy").unwrap();
        assert_eq!(queued.status, PushStatus::Queued);
        assert_eq!(queued.last_error.as_deref(), Some("provider busy"));
        assert_eq!(
            store
                .claim_next_queued_push(session.id)
                .unwrap()
                .unwrap()
                .id,
            first.id
        );
    }

    #[test]
    fn session_inbox_claims_queued_pushes_in_fifo_order_once() {
        let mut store = store();
        let project = Uuid::new_v4();
        let session = store
            .register_session(NewSessionBinding {
                project_id: project,
                profile_id: "codex",
                provider: AgentProvider::Codex,
                external_session_id: "thr_fifo",
                source: SessionBindingSource::Managed,
                verification: SessionVerification::Verified,
                display_name: None,
                working_directory: Path::new("/repo"),
                state: SessionRuntimeState::Running,
            })
            .unwrap();
        let mut enqueue = |task_id: &str, key: &str| {
            store
                .enqueue_push(NewPush {
                    project_id: project,
                    task_id,
                    selected_profile_id: None,
                    target_run_id: None,
                    target_session_id: Some(session.id),
                    mode: PushMode::ExistingSession,
                    delivery: PushDeliveryPolicy::SafeBoundary,
                    content: task_id,
                    idempotency_key: key,
                })
                .unwrap()
        };
        let first = enqueue("TASK-001", "fifo-1");
        let second = enqueue("TASK-002", "fifo-2");
        assert_eq!(
            store
                .sessions_with_queued_pushes(AgentProvider::Codex)
                .unwrap(),
            std::slice::from_ref(&session)
        );

        assert_eq!(
            store
                .claim_next_queued_push(session.id)
                .unwrap()
                .unwrap()
                .id,
            first.id
        );
        store
            .finish_delivery(first.id, PushStatus::Delivered, None, None, false)
            .unwrap();
        assert_eq!(
            store
                .claim_next_queued_push(session.id)
                .unwrap()
                .unwrap()
                .id,
            second.id
        );
        store
            .finish_delivery(second.id, PushStatus::Delivered, None, None, false)
            .unwrap();
        assert!(store.claim_next_queued_push(session.id).unwrap().is_none());
        assert!(
            store
                .sessions_with_queued_pushes(AgentProvider::Codex)
                .unwrap()
                .is_empty()
        );
        assert!(matches!(
            store.begin_delivery(first.id),
            Err(RuntimeStoreError::PushNotQueued(id)) if id == first.id
        ));

        store
            .update_session_runtime(session.id, SessionRuntimeState::Running, Some("turn_stale"))
            .unwrap();
        assert_eq!(store.recover_loaded_sessions().unwrap(), 1);
        let recovered = store.session(session.id).unwrap().unwrap();
        assert_eq!(recovered.state, SessionRuntimeState::NotLoaded);
        assert_eq!(recovered.active_turn_id, None);
    }
}
