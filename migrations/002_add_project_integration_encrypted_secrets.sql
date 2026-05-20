ALTER TABLE project_gitlab_integrations
    ADD COLUMN IF NOT EXISTS token_encrypted TEXT;

ALTER TABLE project_gitlab_integrations
    ADD COLUMN IF NOT EXISTS webhook_secret_encrypted TEXT;
