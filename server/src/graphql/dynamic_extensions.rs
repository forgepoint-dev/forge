//! Dynamic extension schema integration
//!
//! This module provides utilities for integrating extension schemas
//! into the main GraphQL schema at runtime.

use async_graphql::dynamic::*;
use async_graphql::{Value as GqlValue};
use std::sync::Arc;

use crate::extensions::ExtensionManager;

/// Build a dynamic schema that includes extension fields
#[allow(dead_code)]
pub fn build_dynamic_schema_with_extensions(
    extension_manager: Arc<ExtensionManager>,
) -> anyhow::Result<Schema> {
    let mut schema = Schema::build("Query", Some("Mutation"), None);

    // Add core Query fields
    let mut query = Object::new("Query");

    // Add core query fields
    query = query
        .field(Field::new("getAllGroups", TypeRef::named_nn_list_nn("Group"), |_ctx| {
            FieldFuture::new(async move {
                Ok(Some(GqlValue::List(vec![])))
            })
        }))
        .field(Field::new("getAllRepositories", TypeRef::named_nn_list_nn("Repository"), |_ctx| {
            FieldFuture::new(async move {
                Ok(Some(GqlValue::List(vec![])))
            })
        }));

    // Add fields from extensions
    for (ext_name, extension) in extension_manager.get_extensions() {
        tracing::debug!("Adding fields from extension: {}", ext_name);

        // Parse the extension's schema SDL
        let schema_sdl = extension.runtime.schema();

        // For now, log that we have the schema
        // Full dynamic schema parsing would require parsing the SDL
        // and adding all types/fields dynamically
        tracing::info!("Extension {} provides schema: {} bytes", ext_name, schema_sdl.len());
    }

    schema = schema.register(query);

    Ok(schema.finish()?)
}

/// Create a schema comment block documenting available extensions
#[allow(dead_code)]
pub fn create_extension_schema_documentation(extension_manager: &ExtensionManager) -> String {
    let mut doc = String::from("# Extensions\n\n");
    doc.push_str("The following extensions are loaded and provide additional GraphQL types:\n\n");

    for (name, extension) in extension_manager.get_extensions() {
        doc.push_str(&format!("## Extension: {}\n", name));
        doc.push_str(&format!("Version: {}\n", extension.runtime.version()));
        doc.push_str(&format!("Capabilities: {:?}\n\n", extension.runtime.capabilities()));
        doc.push_str("Schema:\n```graphql\n");
        doc.push_str(extension.runtime.schema());
        doc.push_str("\n```\n\n");
    }

    doc
}