use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct ExtensionConfig {
    pub name: String,
    pub db_path: String,
    pub config: Option<String>,
}