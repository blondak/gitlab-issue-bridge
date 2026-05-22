CREATE TABLE IF NOT EXISTS project_integrations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    base_url TEXT NOT NULL,
    api_base_url TEXT NOT NULL,
    external_project_id TEXT NOT NULL,
    token_encrypted TEXT,
    webhook_secret_encrypted TEXT,
    verify_tls BOOLEAN NOT NULL DEFAULT TRUE,
    sync_enabled BOOLEAN NOT NULL DEFAULT TRUE,
    settings JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (project_id, provider)
);

INSERT INTO project_integrations (
    project_id,
    provider,
    base_url,
    api_base_url,
    external_project_id,
    token_encrypted,
    webhook_secret_encrypted,
    verify_tls,
    sync_enabled,
    settings,
    created_at,
    updated_at
)
SELECT
    project_id,
    'gitlab',
    gitlab_base_url,
    gitlab_api_base_url,
    gitlab_project_id::TEXT,
    token_encrypted,
    webhook_secret_encrypted,
    verify_tls,
    sync_enabled,
    jsonb_build_object('legacy_table', 'project_gitlab_integrations'),
    created_at,
    updated_at
FROM project_gitlab_integrations
ON CONFLICT (project_id, provider)
DO UPDATE SET
    base_url = EXCLUDED.base_url,
    api_base_url = EXCLUDED.api_base_url,
    external_project_id = EXCLUDED.external_project_id,
    token_encrypted = EXCLUDED.token_encrypted,
    webhook_secret_encrypted = EXCLUDED.webhook_secret_encrypted,
    verify_tls = EXCLUDED.verify_tls,
    sync_enabled = EXCLUDED.sync_enabled,
    settings = EXCLUDED.settings,
    updated_at = NOW();

CREATE TABLE IF NOT EXISTS issue_external_refs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    issue_id UUID NOT NULL REFERENCES issues(id) ON DELETE CASCADE,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    provider TEXT NOT NULL,
    external_issue_id TEXT NOT NULL,
    external_issue_key TEXT,
    external_url TEXT,
    sync_state TEXT NOT NULL DEFAULT 'idle',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (issue_id, provider),
    UNIQUE (project_id, provider, external_issue_id)
);

INSERT INTO issue_external_refs (
    issue_id,
    project_id,
    provider,
    external_issue_id,
    external_issue_key,
    sync_state,
    created_at,
    updated_at
)
SELECT
    id,
    project_id,
    'gitlab',
    gitlab_issue_iid::TEXT,
    '#' || gitlab_issue_iid::TEXT,
    sync_state,
    created_at,
    updated_at
FROM issues
WHERE gitlab_issue_iid IS NOT NULL
ON CONFLICT (issue_id, provider)
DO UPDATE SET
    external_issue_id = EXCLUDED.external_issue_id,
    external_issue_key = EXCLUDED.external_issue_key,
    sync_state = EXCLUDED.sync_state,
    updated_at = NOW();
