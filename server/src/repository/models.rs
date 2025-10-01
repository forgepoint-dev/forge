use serde::Serialize;

#[derive(Clone, Debug, sqlx::FromRow, Serialize)]
pub struct RepositoryRecord {
    pub id: String,
    pub slug: String,
    pub group_id: Option<String>,
    pub remote_url: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RepositorySummary {
    pub id: String,
    pub slug: String,
    pub remote_url: Option<String>,
}

#[derive(sqlx::FromRow)]
pub struct RepositorySummaryRow {
    pub id: String,
    pub slug: String,
    pub remote_url: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
pub struct RepositoryEntryNode {
    pub name: String,
    pub path: String,
    pub kind: RepositoryEntryKind,
    pub size: Option<i64>,
}

#[derive(Copy, Clone, Eq, PartialEq, Serialize)]
pub enum RepositoryEntryKind {
    File,
    Directory,
}

#[derive(Clone, Serialize)]
pub struct RepositoryEntriesPayload {
    pub tree_path: String,
    pub entries: Vec<RepositoryEntryNode>,
}

impl From<RepositorySummaryRow> for RepositorySummary {
    fn from(row: RepositorySummaryRow) -> Self {
        RepositorySummary {
            id: row.id,
            slug: row.slug,
            remote_url: row.remote_url,
        }
    }
}
