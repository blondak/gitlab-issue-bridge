ALTER TABLE issues
    ALTER COLUMN gitlab_issue_iid DROP NOT NULL;

UPDATE issues
SET gitlab_issue_iid = NULL
WHERE gitlab_issue_iid = 0;
