CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE IF NOT EXISTS projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS project_gitlab_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL UNIQUE REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_base_url TEXT NOT NULL,
    gitlab_api_base_url TEXT NOT NULL,
    gitlab_project_id BIGINT NOT NULL,
    token TEXT NOT NULL,
    webhook_secret TEXT NOT NULL,
    token_encrypted TEXT,
    webhook_secret_encrypted TEXT,
    verify_tls BOOLEAN NOT NULL DEFAULT TRUE,
    sync_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS users (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL UNIQUE,
    full_name TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    active BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS user_sessions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token TEXT NOT NULL UNIQUE,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS user_invitations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email TEXT NOT NULL,
    invite_token TEXT NOT NULL UNIQUE,
    invited_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    is_admin BOOLEAN NOT NULL DEFAULT FALSE,
    status TEXT NOT NULL DEFAULT 'pending',
    expires_at TIMESTAMPTZ NOT NULL,
    last_sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    accepted_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS user_invitations_email_idx
    ON user_invitations (email);

CREATE TABLE IF NOT EXISTS issues (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    gitlab_issue_iid BIGINT,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'open',
    sync_state TEXT NOT NULL DEFAULT 'idle',
    last_source TEXT NOT NULL DEFAULT 'seed',
    version BIGINT NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, gitlab_issue_iid)
);

CREATE TABLE IF NOT EXISTS issue_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    subject_type TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    permission TEXT NOT NULL,
    effect TEXT NOT NULL DEFAULT 'allow',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS project_permissions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    subject_type TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    permission TEXT NOT NULL,
    effect TEXT NOT NULL DEFAULT 'allow',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, subject_type, subject_id)
);

CREATE TABLE IF NOT EXISTS issue_comments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    gitlab_note_id BIGINT NOT NULL,
    discussion_id TEXT,
    individual_note BOOLEAN NOT NULL DEFAULT FALSE,
    reply_to_gitlab_note_id BIGINT,
    author_external_id TEXT NOT NULL,
    author_name TEXT NOT NULL,
    body_raw TEXT NOT NULL,
    system_note BOOLEAN NOT NULL DEFAULT FALSE,
    sync_state TEXT NOT NULL DEFAULT 'idle',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (issue_id, gitlab_note_id)
);

CREATE TABLE IF NOT EXISTS issue_attachments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    comment_id UUID REFERENCES issue_comments(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL DEFAULT 0,
    external_url TEXT NOT NULL,
    proxy_path TEXT NOT NULL,
    storage_backend TEXT NOT NULL DEFAULT 'gitlab',
    storage_path TEXT,
    cache_state TEXT NOT NULL DEFAULT 'not_cached',
    cached_at TIMESTAMPTZ,
    last_cache_error TEXT,
    inline BOOLEAN NOT NULL DEFAULT FALSE,
    created_by_external_id TEXT NOT NULL,
    sync_state TEXT NOT NULL DEFAULT 'idle',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS issue_uploads (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    issue_id UUID REFERENCES issues(id) ON DELETE CASCADE,
    uploaded_by_user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    filename TEXT NOT NULL,
    content_type TEXT NOT NULL,
    byte_size BIGINT NOT NULL DEFAULT 0,
    storage_path TEXT NOT NULL,
    proxy_path TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    consumed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS issue_attachments_storage_backend_cache_state_idx
    ON issue_attachments (storage_backend, cache_state);

CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    topic TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'pending',
    available_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    attempt_count INTEGER NOT NULL DEFAULT 0,
    locked_at TIMESTAMPTZ,
    locked_by TEXT,
    dedupe_key TEXT,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS jobs_dedupe_key_idx
    ON jobs (dedupe_key);

CREATE INDEX IF NOT EXISTS jobs_status_available_idx
    ON jobs (status, available_at);

CREATE INDEX IF NOT EXISTS jobs_processing_locked_idx
    ON jobs (locked_at)
    WHERE status = 'processing';

CREATE TABLE IF NOT EXISTS worker_heartbeats (
    worker_id TEXT PRIMARY KEY,
    status TEXT NOT NULL DEFAULT 'starting',
    started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    heartbeat_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_job_id UUID,
    last_job_topic TEXT,
    last_error TEXT,
    processed_jobs BIGINT NOT NULL DEFAULT 0,
    failed_jobs BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS worker_heartbeats_heartbeat_idx
    ON worker_heartbeats (heartbeat_at DESC);

CREATE TABLE IF NOT EXISTS audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    entity_type TEXT NOT NULL,
    entity_id UUID,
    action TEXT NOT NULL,
    actor TEXT NOT NULL,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS audit_log_entity_created_idx
    ON audit_log (entity_type, entity_id, created_at DESC);

CREATE INDEX IF NOT EXISTS audit_log_action_created_idx
    ON audit_log (action, created_at DESC);

INSERT INTO projects (slug, name, description)
VALUES ('demo-platform', 'Demo Platform', 'Seeded internal project for local development')
ON CONFLICT (slug) DO NOTHING;

INSERT INTO project_gitlab_integrations (
    project_id,
    gitlab_base_url,
    gitlab_api_base_url,
    gitlab_project_id,
    token,
    webhook_secret,
    verify_tls,
    sync_enabled
)
SELECT
    id,
    'https://gitlab.example.local',
    'https://gitlab.example.local/api/v4',
    1001,
    'seeded-demo-token',
    'seeded-demo-webhook-secret',
    TRUE,
    TRUE
FROM projects
WHERE slug = 'demo-platform'
ON CONFLICT (project_id) DO NOTHING;

INSERT INTO issues (project_id, gitlab_issue_iid, title, description, sync_state, last_source)
SELECT id, 1, 'Bootstrap IssueHub', 'Initial seeded issue for local development', 'idle', 'seed'
FROM projects
WHERE slug = 'demo-platform'
ON CONFLICT (project_id, gitlab_issue_iid) DO NOTHING;

INSERT INTO issues (project_id, gitlab_issue_iid, title, description, sync_state, last_source)
SELECT id, 2, 'Design per-issue ACL', 'Seeded item for ACL workflow', 'idle', 'seed'
FROM projects
WHERE slug = 'demo-platform'
ON CONFLICT (project_id, gitlab_issue_iid) DO NOTHING;

INSERT INTO issue_permissions (issue_id, subject_type, subject_id, permission, effect)
SELECT id, 'role', 'manager', 'admin', 'allow'
FROM issues
ON CONFLICT DO NOTHING;

WITH bootstrap_issue AS (
    SELECT issues.id
    FROM issues
    JOIN projects ON projects.id = issues.project_id
    WHERE projects.slug = 'demo-platform' AND issues.gitlab_issue_iid = 1
),
bootstrap_comment AS (
    INSERT INTO issue_comments (issue_id, gitlab_note_id, author_external_id, author_name, body_raw, system_note, sync_state)
    SELECT id, 9001, 'gitlab:user:42', 'Integration Bot', 'Initial sync completed. Inline assets are exposed through IssueHub proxy endpoints.', FALSE, 'idle'
    FROM bootstrap_issue
    ON CONFLICT (issue_id, gitlab_note_id) DO UPDATE
    SET body_raw = EXCLUDED.body_raw,
        updated_at = NOW()
    RETURNING id, issue_id
)
INSERT INTO issue_attachments (issue_id, comment_id, filename, content_type, byte_size, external_url, proxy_path, inline, created_by_external_id, sync_state)
SELECT
    bootstrap_comment.issue_id,
    bootstrap_comment.id,
    'architecture-overview.png',
    'image/png',
    245760,
    'https://gitlab.example.local/uploads/architecture-overview.png',
    '/api/v1/attachments/bootstrap-architecture-overview/download',
    TRUE,
    'gitlab:user:42',
    'idle'
FROM bootstrap_comment
WHERE NOT EXISTS (
    SELECT 1
    FROM issue_attachments
    WHERE proxy_path = '/api/v1/attachments/bootstrap-architecture-overview/download'
);
