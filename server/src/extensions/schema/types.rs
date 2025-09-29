use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeModifier {
    #[serde(rename = "list-type")]
    ListType,
    #[serde(rename = "non-null")]
    NonNull,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TypeRef {
    pub root: String,
    pub modifiers: Vec<TypeModifier>,
}

impl TypeRef {
    pub fn to_sdl(&self) -> String {
        let mut rendered = self.root.clone();
        for modifier in &self.modifiers {
            match modifier {
                TypeModifier::ListType => rendered = format!("[{}]", rendered),
                TypeModifier::NonNull => rendered.push('!'),
            }
        }
        rendered
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputValueDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "ty")]
    pub ty: TypeRef,
    #[serde(rename = "default_value")]
    pub default_value: Option<String>,
}
