CREATE TABLE execution_events (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    session_binding_id TEXT REFERENCES session_bindings(id),
    attempt_id TEXT,
    kind TEXT NOT NULL,
    level TEXT NOT NULL,
    phase TEXT NOT NULL,
    message TEXT NOT NULL,
    detail TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_execution_events_project_recent
ON execution_events(project_id, created_at DESC);

CREATE INDEX idx_execution_events_task_recent
ON execution_events(project_id, task_id, created_at DESC);

CREATE INDEX idx_execution_events_session_recent
ON execution_events(session_binding_id, created_at DESC);
