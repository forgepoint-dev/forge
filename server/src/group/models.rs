use serde::Serialize;

#[derive(Clone, Debug, sqlx::FromRow, Serialize)]
pub struct GroupRecord {
    pub id: String,
    pub slug: String,
    pub parent: Option<String>,
}
