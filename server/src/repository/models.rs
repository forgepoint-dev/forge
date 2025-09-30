use async_graphql::{Enum, ID, Object};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct RepositoryRecord {
    pub id: String,
    pub slug: String,
    pub group_id: Option<String>,
    pub remote_url: Option<String>,
}

#[derive(Clone)]
pub struct RepositoryNode(pub RepositoryRecord);

#[derive(Clone)]
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

#[derive(Clone, PartialEq, Eq)]
pub struct RepositoryEntryNode {
    pub name: String,
    pub path: String,
    pub kind: RepositoryEntryKind,
    pub size: Option<i64>,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq)]
pub enum RepositoryEntryKind {
    #[graphql(name = "FILE")]
    File,
    #[graphql(name = "DIRECTORY")]
    Directory,
}

#[derive(Clone)]
pub struct RepositoryEntriesPayload {
    pub tree_path: String,
    pub entries: Vec<RepositoryEntryNode>,
}

#[Object]
impl RepositoryEntriesPayload {
    async fn tree_path(&self) -> &str {
        &self.tree_path
    }

    async fn entries(&self) -> &[RepositoryEntryNode] {
        &self.entries
    }
}

#[Object]
impl RepositoryEntryNode {
    async fn name(&self) -> &str {
        &self.name
    }

    async fn path(&self) -> &str {
        &self.path
    }

    async fn kind(&self) -> RepositoryEntryKind {
        self.kind
    }

    async fn size(&self) -> Option<i64> {
        self.size
    }
}

#[Object]
impl RepositorySummary {
    async fn id(&self) -> ID {
        ID::from(self.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.slug
    }

    #[graphql(name = "isRemote")]
    async fn is_remote(&self) -> bool {
        self.remote_url.is_some()
    }

    #[graphql(name = "remoteUrl")]
    async fn remote_url(&self) -> Option<&str> {
        self.remote_url.as_deref()
    }
}

impl From<RepositoryRecord> for RepositoryNode {
    fn from(record: RepositoryRecord) -> Self {
        RepositoryNode(record)
    }
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
