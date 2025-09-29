//! Dynamic schema management for extensions with basic conflict detection

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use super::Extension;

/// Modifier applied to a type reference in order of appearance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TypeModifier {
    #[serde(rename = "list-type")]
    ListType,
    #[serde(rename = "non-null")]
    NonNull,
}

/// Reference to another GraphQL type with optional wrappers.
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

/// Input value definition used for field arguments and input object fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputValueDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "ty")]
    pub ty: TypeRef,
    #[serde(rename = "default_value")]
    pub default_value: Option<String>,
}

impl InputValueDefinition {
    fn to_sdl(&self, indent: usize) -> String {
        let mut s = String::new();
        if let Some(formatted) = format_description(self.description.as_deref(), indent) {
            s.push_str(&formatted);
        }

        write!(
            s,
            "{}{}: {}",
            indent_spaces(indent),
            self.name,
            self.ty.to_sdl()
        )
        .unwrap();

        if let Some(default) = &self.default_value {
            write!(s, " = {}", default).unwrap();
        }

        s
    }
}

/// Field definition on an object or interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDefinition {
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "ty")]
    pub ty: TypeRef,
    pub args: Vec<InputValueDefinition>,
}

impl FieldDefinition {
    fn to_sdl(&self, indent: usize) -> String {
        let mut s = String::new();
        if let Some(formatted) = format_description(self.description.as_deref(), indent) {
            s.push_str(&formatted);
        }

        write!(s, "{}{}", indent_spaces(indent), self.name).unwrap();

        if !self.args.is_empty() {
            let args = self
                .args
                .iter()
                .map(|arg| arg.to_sdl(0))
                .collect::<Vec<_>>()
                .join(", ");
            write!(s, "({})", args).unwrap();
        }

        write!(s, ": {}", self.ty.to_sdl()).unwrap();
        s
    }
}

/// Object type definition. When `is_extension` is true the fields extend a root type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectType {
    pub name: String,
    pub description: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldDefinition>,
    #[serde(rename = "is_extension")]
    pub is_extension: bool,
}

/// Interface definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterfaceType {
    pub name: String,
    pub description: Option<String>,
    pub interfaces: Vec<String>,
    pub fields: Vec<FieldDefinition>,
}

/// Scalar definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScalarType {
    pub name: String,
    pub description: Option<String>,
}

/// Enum value definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumValue {
    pub name: String,
    pub description: Option<String>,
}

/// Enum definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnumType {
    pub name: String,
    pub description: Option<String>,
    pub values: Vec<EnumValue>,
}

/// Union definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnionType {
    pub name: String,
    pub description: Option<String>,
    pub members: Vec<String>,
}

/// Input object definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputObjectType {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<InputValueDefinition>,
}

/// Schema type variant.
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

/// Structured schema fragment returned by extensions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct SchemaFragment {
    pub types: Vec<SchemaType>,
}

impl SchemaFragment {
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

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
    fn name(&self) -> &str {
        match self {
            SchemaType::Scalar(s) => &s.name,
            SchemaType::Object(o) => &o.name,
            SchemaType::Interface(i) => &i.name,
            SchemaType::Union(u) => &u.name,
            SchemaType::Enum(e) => &e.name,
            SchemaType::InputObject(io) => &io.name,
        }
    }

    fn to_sdl(&self) -> String {
        match self {
            SchemaType::Scalar(scalar) => {
                let mut out = String::new();
                if let Some(desc) = format_description(scalar.description.as_deref(), 0) {
                    out.push_str(&desc);
                }
                out.push_str(&format!("scalar {}", scalar.name));
                out
            }
            SchemaType::Object(object) => render_object_like(object, object.is_extension),
            SchemaType::Interface(interface) => render_interface(interface),
            SchemaType::Union(union) => {
                let mut out = String::new();
                if let Some(desc) = format_description(union.description.as_deref(), 0) {
                    out.push_str(&desc);
                }
                if union.members.is_empty() {
                    out.push_str(&format!("union {} =", union.name));
                } else {
                    out.push_str(&format!(
                        "union {} = {}",
                        union.name,
                        union.members.join(" | ")
                    ));
                }
                out
            }
            SchemaType::Enum(enum_type) => {
                let mut out = String::new();
                if let Some(desc) = format_description(enum_type.description.as_deref(), 0) {
                    out.push_str(&desc);
                }
                out.push_str(&format!("enum {} {{\n", enum_type.name));
                for value in &enum_type.values {
                    if let Some(desc) = format_description(value.description.as_deref(), 2) {
                        out.push_str(&desc);
                    }
                    out.push_str(&indent_spaces(2));
                    out.push_str(&value.name);
                    out.push('\n');
                }
                out.push('}');
                out
            }
            SchemaType::InputObject(input) => {
                let mut out = String::new();
                if let Some(desc) = format_description(input.description.as_deref(), 0) {
                    out.push_str(&desc);
                }
                out.push_str(&format!("input {} {{\n", input.name));
                for field in &input.fields {
                    out.push_str(&field.to_sdl(2));
                    out.push('\n');
                }
                out.push('}');
                out
            }
        }
    }
}

fn render_object_like(object: &ObjectType, is_extension: bool) -> String {
    let mut out = String::new();
    if let Some(desc) = format_description(object.description.as_deref(), 0) {
        out.push_str(&desc);
    }
    if is_extension {
        out.push_str("extend ");
    }
    out.push_str(&format!("type {}", object.name));
    if !object.interfaces.is_empty() {
        out.push_str(&format!(" implements {}", object.interfaces.join(" & ")));
    }
    out.push_str(" {\n");
    for field in &object.fields {
        out.push_str(&field.to_sdl(2));
        out.push('\n');
    }
    out.push('}');
    out
}

fn render_interface(interface: &InterfaceType) -> String {
    let mut out = String::new();
    if let Some(desc) = format_description(interface.description.as_deref(), 0) {
        out.push_str(&desc);
    }
    out.push_str(&format!("interface {}", interface.name));
    if !interface.interfaces.is_empty() {
        out.push_str(&format!(" implements {}", interface.interfaces.join(" & ")));
    }
    out.push_str(" {\n");
    for field in &interface.fields {
        out.push_str(&field.to_sdl(2));
        out.push('\n');
    }
    out.push('}');
    out
}

fn indent_spaces(indent: usize) -> String {
    " ".repeat(indent)
}

fn format_description(desc: Option<&str>, indent: usize) -> Option<String> {
    let description = desc?;
    let indent_str = indent_spaces(indent);
    if description.contains('\n') {
        let mut out = String::new();
        out.push_str(&indent_str);
        out.push_str("\"\"\"\n");
        for line in description.lines() {
            out.push_str(&indent_str);
            out.push_str(line);
            out.push('\n');
        }
        out.push_str(&indent_str);
        out.push_str("\"\"\"\n");
        Some(out)
    } else {
        Some(format!(
            "{}\"{}\"\n",
            indent_str,
            description.replace('"', "\\\"")
        ))
    }
}

/// Schema manager handles merging extension schemas with basic conflict detection
pub struct SchemaManager {
    extension_schemas: HashMap<String, SchemaFragment>,
    type_registry: HashMap<String, String>, // type_name -> extension_name
    field_registry: HashMap<String, HashMap<String, String>>, // root_type -> field -> extension_name
}

impl SchemaManager {
    pub fn new() -> Self {
        Self {
            extension_schemas: HashMap::new(),
            type_registry: HashMap::new(),
            field_registry: HashMap::new(),
        }
    }

    /// Parse and validate an extension's GraphQL schema with basic conflict detection
    pub fn parse_extension_schema(&mut self, extension: &Extension) -> Result<()> {
        if extension.schema.is_empty() {
            return Ok(());
        }

        self.validate_extension_schema(&extension.name, &extension.schema)?;
        self.detect_basic_conflicts(&extension.name, &extension.schema)?;

        self.extension_schemas
            .insert(extension.name.clone(), extension.schema.clone());

        tracing::info!(
            "Successfully registered schema for extension: {}",
            extension.name
        );
        Ok(())
    }

    fn validate_extension_schema(
        &self,
        extension_name: &str,
        schema: &SchemaFragment,
    ) -> Result<()> {
        if schema.types.is_empty() {
            return Ok(());
        }

        for ty in &schema.types {
            match ty {
                SchemaType::Object(object) => {
                    if is_root_type(&object.name) {
                        if !object.is_extension {
                            bail!(
                                "Extension '{}' must set 'is_extension' when extending root type '{}'",
                                extension_name,
                                object.name
                            );
                        }
                    } else if object.is_extension {
                        bail!(
                            "Extension '{}' attempted to extend non-root type '{}'",
                            extension_name,
                            object.name
                        );
                    }

                    ensure_unique_field_names(&object.fields, extension_name, &object.name)?;
                }
                SchemaType::Interface(interface) => {
                    ensure_unique_field_names(&interface.fields, extension_name, &interface.name)?;
                }
                SchemaType::Enum(enum_type) => {
                    ensure_unique_names(
                        enum_type.values.iter().map(|v| v.name.as_str()),
                        extension_name,
                        &enum_type.name,
                        "enum value",
                    )?;
                }
                SchemaType::InputObject(input_object) => {
                    ensure_unique_names(
                        input_object.fields.iter().map(|f| f.name.as_str()),
                        extension_name,
                        &input_object.name,
                        "input field",
                    )?;
                }
                SchemaType::Scalar(_) | SchemaType::Union(_) => {}
            }
        }

        Ok(())
    }

    fn detect_basic_conflicts(
        &mut self,
        extension_name: &str,
        schema: &SchemaFragment,
    ) -> Result<()> {
        for ty in &schema.types {
            match ty {
                SchemaType::Object(object) => {
                    if is_root_type(&object.name) {
                        let field_registry = self
                            .field_registry
                            .entry(object.name.clone())
                            .or_insert_with(HashMap::new);

                        for field in &object.fields {
                            if let Some(existing) = field_registry.get(&field.name) {
                                bail!(
                                    "Root field conflict: '{}' on '{}' is already provided by extension '{}'",
                                    field.name,
                                    object.name,
                                    existing
                                );
                            }
                            field_registry.insert(field.name.clone(), extension_name.to_string());
                        }
                    } else {
                        self.register_type(&object.name, extension_name)?;
                    }
                }
                SchemaType::Interface(interface) => {
                    self.register_type(&interface.name, extension_name)?;
                }
                SchemaType::Enum(enum_type) => {
                    self.register_type(&enum_type.name, extension_name)?;
                }
                SchemaType::InputObject(input_object) => {
                    self.register_type(&input_object.name, extension_name)?;
                }
                SchemaType::Scalar(scalar) => {
                    self.register_type(&scalar.name, extension_name)?;
                }
                SchemaType::Union(union) => {
                    self.register_type(&union.name, extension_name)?;
                }
            }
        }

        Ok(())
    }

    fn register_type(&mut self, type_name: &str, extension_name: &str) -> Result<()> {
        if let Some(existing) = self.type_registry.get(type_name) {
            if existing != extension_name {
                bail!(
                    "Type name conflict: '{}' is already defined by extension '{}'",
                    type_name,
                    existing
                );
            }
        } else {
            self.type_registry
                .insert(type_name.to_string(), extension_name.to_string());
        }
        Ok(())
    }

    /// Get all extension schema fragments
    pub fn get_extension_schemas(&self) -> &HashMap<String, SchemaFragment> {
        &self.extension_schemas
    }

    /// Create a merged schema SDL string for all extensions (for debugging)
    pub fn create_merged_schema_sdl(&self) -> String {
        let mut merged = String::new();
        merged.push_str("# Merged GraphQL schema from extensions\n");
        merged.push_str("# Generated automatically - do not edit\n\n");

        for (extension_name, schema) in &self.extension_schemas {
            merged.push_str(&format!("# Schema from {} extension\n", extension_name));
            merged.push_str(&schema.to_sdl());
            merged.push_str("\n\n");
        }

        merged
    }

    /// Get schema for a specific extension
    pub fn get_extension_schema(&self, extension_name: &str) -> Option<&SchemaFragment> {
        self.extension_schemas.get(extension_name)
    }

    /// Check if any extensions are loaded
    pub fn has_extensions(&self) -> bool {
        !self.extension_schemas.is_empty()
    }

    /// Get conflict summary for debugging
    pub fn get_conflict_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str("Schema Registry Summary:\n");
        summary.push_str(&format!(
            "Total extensions: {}\n",
            self.extension_schemas.len()
        ));
        summary.push_str(&format!(
            "Total registered types: {}\n",
            self.type_registry.len()
        ));

        for (type_name, extension) in &self.type_registry {
            summary.push_str(&format!("  type {} -> {}\n", type_name, extension));
        }

        for (root_type, fields) in &self.field_registry {
            summary.push_str(&format!("  {} fields: {}\n", root_type, fields.len()));
            for (field, owner) in fields {
                summary.push_str(&format!("    {} -> {}\n", field, owner));
            }
        }

        summary
    }
}

fn ensure_unique_field_names(fields: &[FieldDefinition], extension: &str, ty: &str) -> Result<()> {
    ensure_unique_names(
        fields.iter().map(|f| f.name.as_str()),
        extension,
        ty,
        "field",
    )
}

fn ensure_unique_names<'a, I>(names: I, extension: &str, ty: &str, scope: &str) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut seen = HashSet::new();
    for name in names {
        if !seen.insert(name.to_string()) {
            bail!(
                "Duplicate {} '{}' found in '{}' provided by extension '{}'",
                scope,
                name,
                ty,
                extension
            );
        }
    }
    Ok(())
}

fn is_root_type(name: &str) -> bool {
    matches!(name, "Query" | "Mutation" | "Subscription")
}
