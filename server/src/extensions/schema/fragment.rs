use super::enum_type::EnumType;
use super::interface::InterfaceType;
use super::object::ObjectType;
use super::scalar::ScalarType;
use super::sdl;
use super::types::InputValueDefinition;
use serde::{Deserialize, Serialize};
use std::fmt::Write;

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
}

impl SchemaFragment {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    #[allow(dead_code)]
    pub fn to_sdl(&self) -> String {
        let mut out = String::new();
        for ty in &self.types {
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&ty.to_sdl());
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
    pub fn to_sdl(&self) -> String {
        match self {
            SchemaType::Object(obj) => sdl::render_object_like(obj, false),
            SchemaType::Interface(iface) => sdl::render_interface(iface),
            SchemaType::Scalar(scalar) => {
                let mut output = String::new();
                if let Some(desc) = &scalar.description {
                    writeln!(&mut output, "\"\"\"{}\"\"\"", desc).unwrap();
                }
                write!(&mut output, "scalar").unwrap();
                output
            }
            SchemaType::Enum(e) => {
                let mut output = String::new();
                if let Some(desc) = &e.description {
                    writeln!(&mut output, "\"\"\"{}\"\"\"", desc).unwrap();
                }
                writeln!(&mut output, "enum {{").unwrap();
                for value in &e.values {
                    if let Some(desc) = &value.description {
                        writeln!(&mut output, "  \"\"\"{}\"\"\"", desc).unwrap();
                    }
                    writeln!(&mut output, "  {}", value.name).unwrap();
                }
                write!(&mut output, "}}").unwrap();
                output
            }
            SchemaType::Union(u) => {
                let mut output = String::new();
                if let Some(desc) = &u.description {
                    writeln!(&mut output, "\"\"\"{}\"\"\"", desc).unwrap();
                }
                write!(&mut output, "union = {}", u.types.join(" | ")).unwrap();
                output
            }
            SchemaType::InputObject(io) => {
                let mut output = String::new();
                if let Some(desc) = &io.description {
                    writeln!(&mut output, "\"\"\"{}\"\"\"", desc).unwrap();
                }
                writeln!(&mut output, "input {{").unwrap();
                for field in &io.fields {
                    if let Some(desc) = &field.description {
                        writeln!(&mut output, "  \"\"\"{}\"\"\"", desc).unwrap();
                    }
                    writeln!(&mut output, "  {}: {}", field.name, field.ty.to_sdl()).unwrap();
                }
                write!(&mut output, "}}").unwrap();
                output
            }
        }
    }
}
