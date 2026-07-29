CREATE INDEX idx_push_session_inbox
ON push_requests(resolved_session_id, target_session_id, status, created_at);
