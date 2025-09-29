use anyhow::{Result, bail};
use std::collections::HashMap;

use super::fragment::{SchemaFragment, SchemaType};
use super::validator::{ensure_unique_field_names, ensure_unique_names};
use crate::extensions::Extension;

#[allow(dead_code)] // Will be used for dynamic schema management
pub struct SchemaManager {
    extension_schemas: HashMap<String, SchemaFragment>,
    type_registry: HashMap<String, String>,
    field_registry: HashMap<String, HashMap<String, String>>,
}

#[allow(dead_code)] // Implementation will be used for schema operations
impl SchemaManager {
    pub fn new() -> Self {
        Self {
            extension_schemas: HashMap::new(),
            type_registry: HashMap::new(),
            field_registry: HashMap::new(),
        }
    }

    #[allow(dead_code)]
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
                    ensure_unique_field_names(&object.fields, extension_name, "object")?;
                }
                SchemaType::Interface(interface) => {
                    ensure_unique_field_names(&interface.fields, extension_name, "interface")?;
                }
                SchemaType::Enum(enum_type) => {
                    ensure_unique_names(
                        enum_type.values.iter().map(|v| v.name.as_str()),
                        extension_name,
                        "enum",
                        "enum value",
                    )?;
                }
                SchemaType::InputObject(input_object) => {
                    ensure_unique_names(
                        input_object.fields.iter().map(|f| f.name.as_str()),
                        extension_name,
                        "input object",
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
                SchemaType::Object(_) => {
                    self.register_type("object", extension_name)?;
                }
                SchemaType::Interface(_) => {
                    self.register_type("interface", extension_name)?;
                }
                SchemaType::Enum(_) => {
                    self.register_type("enum", extension_name)?;
                }
                SchemaType::InputObject(_) => {
                    self.register_type("input", extension_name)?;
                }
                SchemaType::Scalar(_) => {
                    self.register_type("scalar", extension_name)?;
                }
                SchemaType::Union(_) => {
                    self.register_type("union", extension_name)?;
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

    pub fn get_extension_schemas(&self) -> &HashMap<String, SchemaFragment> {
        &self.extension_schemas
    }

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

    #[allow(dead_code)]
    pub fn get_extension_schema(&self, extension_name: &str) -> Option<&SchemaFragment> {
        self.extension_schemas.get(extension_name)
    }

    pub fn has_extensions(&self) -> bool {
        !self.extension_schemas.is_empty()
    }

    #[allow(dead_code)]
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
            summary.push_str(&format!(
                "  Type '{}' from extension '{}'\n",
                type_name, extension
            ));
        }

        summary
    }
}
