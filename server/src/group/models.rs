use async_graphql::{ID, Object};

#[derive(Clone, Debug, sqlx::FromRow)]
pub struct GroupRecord {
    pub id: String,
    pub slug: String,
    pub parent: Option<String>,
}

#[derive(Clone)]
pub struct GroupNode(pub GroupRecord);

#[derive(Clone)]
pub struct GroupSummary {
    pub id: String,
    pub slug: String,
}

#[Object]
impl GroupSummary {
    async fn id(&self) -> ID {
        ID::from(self.id.clone())
    }

    async fn slug(&self) -> &str {
        &self.slug
    }
}

impl From<GroupRecord> for GroupNode {
    fn from(record: GroupRecord) -> Self {
        GroupNode(record)
    }
}

impl From<GroupRecord> for GroupSummary {
    fn from(record: GroupRecord) -> Self {
        GroupSummary {
            id: record.id,
            slug: record.slug,
        }
    }
}
