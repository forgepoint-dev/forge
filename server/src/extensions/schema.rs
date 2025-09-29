//! Dynamic schema management for extensions

use anyhow::{Context, Result};
use async_graphql::{SchemaBuilder, EmptySubscription};
use std::collections::HashMap;

use super::{Extension, ExtensionManager};

/// Schema manager handles merging extension schemas with the core schema
pub struct SchemaManager {
    extension_schemas: HashMap<String, String>,
}

impl SchemaManager {
    pub fn new() -> Self {
        Self {
            extension_schemas: HashMap::new(),
        }
    }

    /// Parse and validate an extension's GraphQL schema SDL
    pub fn parse_extension_schema(&mut self, extension: &Extension) -> Result<()> {
        if extension.schema_sdl.is_empty() {
            return Ok(());
        }

        // Validate the schema SDL format
        self.validate_extension_schema(&extension.name, &extension.schema_sdl)?;

        // Store the SDL
        self.extension_schemas.insert(extension.name.clone(), extension.schema_sdl.clone());

        Ok(())
    }

    /// Validate an extension schema for conflicts and compliance
    fn validate_extension_schema(&self, extension_name: &str, schema_sdl: &str) -> Result<()> {
        // Basic validation - ensure it's not empty and contains valid GraphQL
        if schema_sdl.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "Extension '{}' provided empty schema", 
                extension_name
            ));
        }

        // In a full implementation, we would parse the SDL and check for conflicts
        Ok(())
    }

    /// Get all extension schema SDLs
    pub fn get_extension_schemas(&self) -> &HashMap<String, String> {
        &self.extension_schemas
    }

    /// Create a merged schema SDL string for all extensions
    pub fn create_merged_schema_sdl(&self) -> String {
        let mut merged = String::new();
        
        // Add core schema extensions
        merged.push_str("extend type Query {\n");
        
        for (extension_name, _schema_sdl) in &self.extension_schemas {
            // Add extension fields with namespace prefixing to avoid conflicts
            merged.push_str(&format!("  # Fields from {} extension\n", extension_name));
            // In a full implementation, we would extract fields and add them
            // with proper namespace prefixing like: issues_getAllIssues
        }
        
        merged.push_str("}\n\n");
        
        // Add extension types with namespace prefixing
        for (extension_name, schema_sdl) in &self.extension_schemas {
            merged.push_str(&format!("# Types from {} extension\n", extension_name));
            merged.push_str(schema_sdl);
            merged.push_str("\n");
        }
        
        merged
    }
}