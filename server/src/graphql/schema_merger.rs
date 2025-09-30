use anyhow::{Context as _, Result};
use async_graphql_parser::types::{TypeDefinition, TypeKind, TypeSystemDefinition};
use std::collections::HashMap;
use std::sync::Arc;

use crate::extensions::ExtensionManager;

/// Merges extension schemas with the core schema
#[allow(dead_code)]
pub struct SchemaMerger {
    extension_manager: Arc<ExtensionManager>,
}

#[allow(dead_code)]
impl SchemaMerger {
    pub fn new(extension_manager: Arc<ExtensionManager>) -> Self {
        Self { extension_manager }
    }

    /// Parse all extension schemas and extract their types and fields
    pub fn parse_extensions(&self) -> Result<ExtensionSchemaData> {
        let mut query_fields = HashMap::new();
        let mut mutation_fields = HashMap::new();
        let mut types = Vec::new();

        for (ext_name, extension) in self.extension_manager.get_extensions() {
            let schema_sdl = extension.runtime.schema();
            let doc = async_graphql_parser::parse_schema(schema_sdl)
                .context("Failed to parse extension schema")?;

            for definition in &doc.definitions {
                if let TypeSystemDefinition::Type(type_def) = definition {
                    match &type_def.node.kind {
                        TypeKind::Object(obj_def) => {
                            let type_name = &type_def.node.name.node;

                            if type_def.node.extend {
                                if type_name == "Query" {
                                    for field in &obj_def.fields {
                                        query_fields.insert(
                                            field.node.name.node.to_string(),
                                            (ext_name.clone(), field.node.clone()),
                                        );
                                    }
                                } else if type_name == "Mutation" {
                                    for field in &obj_def.fields {
                                        mutation_fields.insert(
                                            field.node.name.node.to_string(),
                                            (ext_name.clone(), field.node.clone()),
                                        );
                                    }
                                }
                            } else if type_name != "Query" && type_name != "Mutation" {
                                types.push(type_def.node.clone());
                            }
                        }
                        TypeKind::Enum(_) | TypeKind::Interface(_) | TypeKind::Union(_) | TypeKind::InputObject(_) => {
                            types.push(type_def.node.clone());
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(ExtensionSchemaData {
            query_fields,
            mutation_fields,
            types,
        })
    }

    /// Generate a merged SDL containing core and extension schemas
    pub fn generate_merged_sdl(&self) -> Result<String> {
        let extension_data = self.parse_extensions()?;
        let sdl = String::new();

        // Add extension types
        // Note: TypeDefinition doesn't implement Display, so we'd need to
        // implement our own SDL generation for these types
        for _type_def in &extension_data.types {
            // TODO: Implement SDL generation for TypeDefinition
            // sdl.push_str(&format!("{}\n\n", type_def));
        }

        // We'll use the SDL to document the schema but actual resolution
        // will be handled through our resolver infrastructure

        Ok(sdl)
    }
}

#[allow(dead_code)]
pub struct ExtensionSchemaData {
    pub query_fields: HashMap<String, (String, async_graphql_parser::types::FieldDefinition)>,
    pub mutation_fields: HashMap<String, (String, async_graphql_parser::types::FieldDefinition)>,
    pub types: Vec<TypeDefinition>,
}