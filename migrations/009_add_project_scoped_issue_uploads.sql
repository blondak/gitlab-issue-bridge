ALTER TABLE issue_uploads
    ADD COLUMN IF NOT EXISTS project_id UUID REFERENCES projects(id) ON DELETE CASCADE;

UPDATE issue_uploads
SET project_id = issues.project_id
FROM issues
WHERE issues.id = issue_uploads.issue_id
  AND issue_uploads.project_id IS NULL;

ALTER TABLE issue_uploads
    ALTER COLUMN issue_id DROP NOT NULL;

CREATE INDEX IF NOT EXISTS issue_uploads_project_id_idx
    ON issue_uploads (project_id);
