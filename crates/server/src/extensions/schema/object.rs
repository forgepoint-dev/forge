use super::field::FieldDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    pub description: Option<String>,
    pub fields: Vec<FieldDefinition>,
    pub interfaces: Vec<String>,
}
