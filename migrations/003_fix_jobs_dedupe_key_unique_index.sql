DROP INDEX IF EXISTS jobs_dedupe_key_idx;

CREATE UNIQUE INDEX IF NOT EXISTS jobs_dedupe_key_idx
    ON jobs (dedupe_key);
