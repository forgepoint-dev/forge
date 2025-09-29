use super::field::FieldDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub description: Option<String>,
    pub fields: Vec<FieldDefinition>,
}
