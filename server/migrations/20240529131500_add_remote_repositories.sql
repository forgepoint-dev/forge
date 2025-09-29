ALTER TABLE repositories ADD COLUMN remote_url TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_repositories_remote_url
    ON repositories(remote_url)
    WHERE remote_url IS NOT NULL;
