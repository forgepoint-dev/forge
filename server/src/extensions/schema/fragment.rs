use super::enum_type::EnumType;
use super::interface::InterfaceType;
use super::object::ObjectType;
use super::scalar::ScalarType;
use super::types::InputValueDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnionType {
    pub description: Option<String>,
    pub types: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputObjectType {
    pub description: Option<String>,
    pub fields: Vec<InputValueDefinition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SchemaType {
    #[serde(rename = "scalar-type")]
    Scalar(ScalarType),
    #[serde(rename = "object-type")]
    Object(ObjectType),
    #[serde(rename = "interface-type")]
    Interface(InterfaceType),
    #[serde(rename = "union-type")]
    Union(UnionType),
    #[serde(rename = "enum-type")]
    Enum(EnumType),
    #[serde(rename = "input-object-type")]
    InputObject(InputObjectType),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SchemaFragment {
    pub types: Vec<SchemaType>,
    pub federation_sdl: Option<String>,
}

impl SchemaFragment {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// Generate federation-compatible SDL
    #[allow(dead_code)]
    pub fn to_sdl(&self) -> String {
        // If we have pre-generated federation SDL, use that
        if let Some(ref federation_sdl) = self.federation_sdl {
            return federation_sdl.clone();
        }

        // Otherwise fallback to generating from types (if needed)
        if self.types.is_empty() {
            return String::new();
        }

        // Generate SDL from types (placeholder implementation)
        let mut out = String::new();
        for ty in &self.types {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&ty.to_federation_sdl());
        }
        out
    }
}

impl SchemaType {
    #[allow(dead_code)]
    fn name(&self) -> &str {
        match self {
            SchemaType::Scalar(_) => "scalar",
            SchemaType::Object(_) => "object",
            SchemaType::Interface(_) => "interface",
            SchemaType::Union(_) => "union",
            SchemaType::Enum(_) => "enum",
            SchemaType::InputObject(_) => "input object",
        }
    }

    #[allow(dead_code)]
    pub fn to_federation_sdl(&self) -> String {
        // For now, we're using pre-generated federation SDL from extensions
        // This method is not used when federation_sdl is provided in SchemaFragment
        String::new()
    }
}
