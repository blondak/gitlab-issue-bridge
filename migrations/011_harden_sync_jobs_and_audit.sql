CREATE INDEX IF NOT EXISTS jobs_processing_locked_idx
    ON jobs (locked_at)
    WHERE status = 'processing';

CREATE INDEX IF NOT EXISTS audit_log_entity_created_idx
    ON audit_log (entity_type, entity_id, created_at DESC);

CREATE INDEX IF NOT EXISTS audit_log_action_created_idx
    ON audit_log (action, created_at DESC);
