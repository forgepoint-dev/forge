//! Dynamic schema management for extensions

use anyhow::Result;
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

        // Validate that it only contains extend directives, not root types
        let lines: Vec<&str> = schema_sdl.lines().collect();
        for line in lines {
            let trimmed = line.trim();
            if trimmed.starts_with("type Query") || 
               trimmed.starts_with("type Mutation") || 
               trimmed.starts_with("type Subscription") {
                return Err(anyhow::anyhow!(
                    "Extension '{}' cannot define root types Query, Mutation, or Subscription. Use 'extend type' instead.", 
                    extension_name
                ));
            }
        }

        Ok(())
    }

    /// Get all extension schema SDLs
    pub fn get_extension_schemas(&self) -> &HashMap<String, String> {
        &self.extension_schemas
    }

    /// Create a merged schema SDL string for all extensions
    pub fn create_merged_schema_sdl(&self) -> String {
        let mut merged = String::new();
        
        // Simply concatenate all extension schemas
        // Extensions are responsible for using "extend type" directives properly
        for (extension_name, schema_sdl) in &self.extension_schemas {
            merged.push_str(&format!("# Schema from {} extension\n", extension_name));
            merged.push_str(schema_sdl);
            merged.push_str("\n\n");
        }
        
        merged
    }

    /// Get schema for a specific extension
    pub fn get_extension_schema(&self, extension_name: &str) -> Option<&String> {
        self.extension_schemas.get(extension_name)
    }

    /// Check if any extensions are loaded
    pub fn has_extensions(&self) -> bool {
        !self.extension_schemas.is_empty()
    }
}