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

#[derive(Clone, Serialize)]
pub struct RepositoryBranch {
    pub name: String,
    pub reference: String,
    pub target: Option<String>,
    #[serde(rename = "isDefault")]
    pub is_default: bool,
}

#[derive(Clone, Serialize)]
pub struct RepositoryFilePayload {
    pub path: String,
    pub name: String,
    pub size: i64,
    pub is_binary: bool,
    pub text: Option<String>,
    pub truncated: bool,
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
