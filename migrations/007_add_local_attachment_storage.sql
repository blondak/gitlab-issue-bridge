ALTER TABLE issue_attachments
    ADD COLUMN IF NOT EXISTS storage_backend TEXT NOT NULL DEFAULT 'gitlab';

ALTER TABLE issue_attachments
    ADD COLUMN IF NOT EXISTS storage_path TEXT;

CREATE TABLE IF NOT EXISTS issue_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    uploaded_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL DEFAULT 0,
    storage_path TEXT NOT NULL,
    proxy_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS issue_uploads_issue_id_idx
    ON issue_uploads (issue_id);

CREATE INDEX IF NOT EXISTS issue_uploads_uploaded_by_user_id_idx
    ON issue_uploads (uploaded_by_user_id);
