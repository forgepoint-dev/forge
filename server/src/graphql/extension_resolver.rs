use anyhow::{Context as _, Result};
use async_graphql::Value;
use async_graphql_parser::types::TypeKind;
use std::collections::HashMap;
use std::sync::Arc;

use crate::extensions::ExtensionManager;

/// Registry of extension fields and their handlers
#[derive(Clone)]
pub struct ExtensionFieldRegistry {
    /// Maps field names to extension names for Query type
    query_fields: HashMap<String, String>,
    /// Maps field names to extension names for Mutation type
    mutation_fields: HashMap<String, String>,
    /// Extension manager reference
    extension_manager: Arc<ExtensionManager>,
}

impl ExtensionFieldRegistry {
    pub fn new(extension_manager: Arc<ExtensionManager>) -> Self {
        Self {
            query_fields: HashMap::new(),
            mutation_fields: HashMap::new(),
            extension_manager,
        }
    }

    /// Parse extension schemas and register fields
    pub fn register_extensions(&mut self) -> Result<()> {
        // Collect extension data first to avoid borrowing issues
        let extension_schemas: Vec<(String, String)> = self
            .extension_manager
            .get_extensions()
            .iter()
            .map(|(name, ext)| (name.clone(), ext.runtime.schema().to_string()))
            .collect();

        // Now parse and register each extension's fields
        for (ext_name, schema_sdl) in extension_schemas {
            self.parse_and_register_fields(&ext_name, &schema_sdl)?;
        }
        Ok(())
    }

    /// Parse SDL and register extension fields
    fn parse_and_register_fields(&mut self, extension_name: &str, sdl: &str) -> Result<()> {
        let doc = async_graphql_parser::parse_schema(sdl)
            .context("Failed to parse extension schema")?;

        use async_graphql_parser::types::TypeSystemDefinition;

        for definition in &doc.definitions {
            if let TypeSystemDefinition::Type(type_def) = definition {
                match &type_def.node.kind {
                    TypeKind::Object(obj_def) => {
                        let type_name = &type_def.node.name.node;

                        // Check if this is an extension of Query or Mutation
                        if type_def.node.extend && (type_name == "Query" || type_name == "Mutation") {
                            for field in &obj_def.fields {
                                if type_name == "Query" {
                                    self.query_fields.insert(
                                        field.node.name.node.to_string(),
                                        extension_name.to_string(),
                                    );
                                    tracing::info!(
                                        "Registered Query field '{}' from extension '{}'",
                                        field.node.name.node,
                                        extension_name
                                    );
                                } else if type_name == "Mutation" {
                                    self.mutation_fields.insert(
                                        field.node.name.node.to_string(),
                                        extension_name.to_string(),
                                    );
                                    tracing::info!(
                                        "Registered Mutation field '{}' from extension '{}'",
                                        field.node.name.node,
                                        extension_name
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Resolve a field through the appropriate extension
    #[allow(dead_code)]
    pub async fn resolve_field(
        &self,
        parent_type: &str,
        field_name: &str,
        arguments: Value,
        context: Value,
    ) -> Result<Value> {
        // Determine which extension handles this field
        let extension_name = match parent_type {
            "Query" => self.query_fields.get(field_name),
            "Mutation" => self.mutation_fields.get(field_name),
            _ => None,
        };

        let extension_name = extension_name
            .ok_or_else(|| anyhow::anyhow!("No extension registered for field: {}", field_name))?;

        // Get the extension
        let extensions = self.extension_manager.get_extensions();
        let extension = extensions
            .get(extension_name)
            .ok_or_else(|| anyhow::anyhow!("Extension not found: {}", extension_name))?;

        // Convert async_graphql::Value to serde_json::Value
        let args_json = serde_json::to_value(&arguments)?;
        let _ctx_json = serde_json::to_value(&context)?;

        // Call the extension's resolve_field method
        let result = extension
            .runtime
            .resolve_field(
                field_name,
                &serde_json::to_string(&args_json)?,
            )?;

        // Convert JSON string back to async_graphql::Value
        let json_value: serde_json::Value = serde_json::from_str(&result)?;
        let gql_value: Value = serde_json::from_value(json_value)?;
        Ok(gql_value)
    }

    /// Get registered Query fields
    #[allow(dead_code)]
    pub fn get_query_fields(&self) -> &HashMap<String, String> {
        &self.query_fields
    }

    /// Get registered Mutation fields
    #[allow(dead_code)]
    pub fn get_mutation_fields(&self) -> &HashMap<String, String> {
        &self.mutation_fields
    }
}