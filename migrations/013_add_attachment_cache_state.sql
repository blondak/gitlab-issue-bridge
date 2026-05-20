ALTER TABLE issue_attachments
    ADD COLUMN IF NOT EXISTS cache_state TEXT NOT NULL DEFAULT 'not_cached';

ALTER TABLE issue_attachments
    ADD COLUMN IF NOT EXISTS cached_at TIMESTAMPTZ;

ALTER TABLE issue_attachments
    ADD COLUMN IF NOT EXISTS last_cache_error TEXT;

UPDATE issue_attachments
SET cache_state = CASE
    WHEN storage_backend = 'local' THEN 'local_authoritative'
    WHEN storage_backend = 'gitlab' AND storage_path IS NOT NULL THEN 'cached'
    ELSE 'not_cached'
END
WHERE cache_state = 'not_cached';

UPDATE issue_attachments
SET cached_at = created_at
WHERE cached_at IS NULL
  AND (
      storage_backend = 'local'
      OR (storage_backend = 'gitlab' AND storage_path IS NOT NULL)
  );

CREATE INDEX IF NOT EXISTS issue_attachments_storage_backend_cache_state_idx
    ON issue_attachments (storage_backend, cache_state);
