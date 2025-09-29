-- Create groups table storing hierarchical metadata.
CREATE TABLE IF NOT EXISTS groups (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    parent TEXT NULL REFERENCES groups(id),
    UNIQUE(parent, slug)
);

-- Create repositories table keyed by cuid2 identifiers.
CREATE TABLE IF NOT EXISTS repositories (
    id TEXT PRIMARY KEY,
    slug TEXT NOT NULL,
    "group" TEXT NULL REFERENCES groups(id),
    UNIQUE("group", slug)
);
