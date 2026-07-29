CREATE TABLE runs (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    session_binding_id TEXT REFERENCES session_bindings(id),
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE session_bindings (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    profile_id TEXT NOT NULL,
    provider TEXT NOT NULL,
    external_session_id TEXT NOT NULL,
    binding_source TEXT NOT NULL,
    verification_status TEXT NOT NULL,
    display_name TEXT,
    working_directory TEXT NOT NULL,
    state TEXT NOT NULL,
    active_turn_id TEXT,
    hidden INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_used_at TEXT NOT NULL,
    UNIQUE(project_id, provider, external_session_id)
);

CREATE TABLE push_requests (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL,
    task_id TEXT NOT NULL,
    selected_profile_id TEXT,
    target_run_id TEXT REFERENCES runs(id),
    target_session_id TEXT REFERENCES session_bindings(id),
    mode TEXT NOT NULL,
    delivery TEXT NOT NULL,
    content TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL,
    resolved_run_id TEXT REFERENCES runs(id),
    resolved_session_id TEXT REFERENCES session_bindings(id),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    delivered_at TEXT
);

CREATE TABLE push_delivery_attempts (
    id TEXT PRIMARY KEY,
    push_id TEXT NOT NULL REFERENCES push_requests(id),
    attempt_number INTEGER NOT NULL,
    started_at TEXT NOT NULL,
    finished_at TEXT,
    provider_receipt TEXT,
    error TEXT,
    retryable INTEGER,
    UNIQUE(push_id, attempt_number)
);

CREATE INDEX idx_sessions_project_recent
ON session_bindings(project_id, hidden, last_used_at DESC);

CREATE INDEX idx_push_status_created
ON push_requests(status, created_at);
