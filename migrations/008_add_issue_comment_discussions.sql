ALTER TABLE issue_comments
    ADD COLUMN IF NOT EXISTS discussion_id TEXT;

ALTER TABLE issue_comments
    ADD COLUMN IF NOT EXISTS individual_note BOOLEAN NOT NULL DEFAULT FALSE;

ALTER TABLE issue_comments
    ADD COLUMN IF NOT EXISTS reply_to_gitlab_note_id BIGINT;
