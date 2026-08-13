CREATE TABLE approval_requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT,
    profile_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    session_binding_id TEXT NOT NULL REFERENCES session_bindings(id),
    attempt_id TEXT,
    turn_id TEXT NOT NULL,
    item_id TEXT NOT NULL,
    provider_request_key TEXT NOT NULL,
    kind TEXT NOT NULL,
    command TEXT,
    cwd TEXT,
    reason TEXT,
    status TEXT NOT NULL,
    decision TEXT,
    error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    resolved_at TEXT
);

CREATE INDEX idx_approval_requests_status_recent
ON approval_requests(status, created_at DESC);

CREATE INDEX idx_approval_requests_session_status
ON approval_requests(session_binding_id, status, created_at DESC);

CREATE UNIQUE INDEX idx_approval_requests_provider_request
ON approval_requests(session_binding_id, provider_request_key);
